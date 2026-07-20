//! Binary serialization for compiled [`Bytecode`] (`.mbc` files).
//!
//! Format and safety model: docs/bytecode-snapshot-design.md. This module is
//! defense layer L1: structural validation plus a linear scan over every
//! instruction stream, so bytecode that reaches the VM never indexes out of
//! range and never jumps into the middle of an instruction. Stack discipline
//! and runtime types are deliberately left to the VM's own checks (L3).

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::rc::Rc;

use object::builtins::BuiltIns;
use object::{CompiledFunction, Object};
use parser::lexer::token::Span;
use strum::IntoEnumIterator;

use crate::compiler::{Bytecode, DebugInfo, PcSpan};
use crate::op_code::{read_operands, Instructions, Opcode, DEFINITIONS};

/// Bump when the container layout changes (header, sections, tags, varint
/// rules). Bytecode ABI changes are covered by the fingerprint instead.
pub const FORMAT_VERSION: u8 = 1;

pub(crate) const MAGIC: [u8; 4] = *b"MBC\0";
pub(crate) const FLAG_HAS_DEBUG_INFO: u8 = 0b0000_0001;

pub(crate) const TAG_INTEGER: u8 = 1;
pub(crate) const TAG_STRING: u8 = 2;
pub(crate) const TAG_FUNCTION: u8 = 3;

#[derive(Debug, PartialEq)]
pub enum SnapshotWriteError {
    /// `Bytecode.constants` is a public field, so the writer cannot assume it
    /// only holds the three variants the compiler emits.
    UnsupportedConstant { index: usize, kind: String },
}

#[derive(Debug, PartialEq)]
pub enum SnapshotError {
    BadMagic,
    UnsupportedVersion {
        found: u8,
        expected: u8,
    },
    AbiFingerprintMismatch {
        found: u32,
        expected: u32,
    },
    UnexpectedEof,
    InvalidLeb128,
    IntegerOverflow,
    /// A declared size exceeds the remaining input bytes.
    LimitExceeded,
    BadTag(u8),
    BadUtf8,
    BadFlags(u8),
    TrailingBytes,
    /// Instruction-stream validation failure, with stream and offset.
    InvalidInstruction(String),
    DuplicateDebugEntry(usize),
    DebugPcNotIncreasing {
        pc: usize,
    },
    /// The debug entry's constant index does not name a function constant.
    DebugIndexNotFunction(usize),
    DebugPcOutOfRange {
        pc: usize,
        len: usize,
    },
}

lazy_static! {
    static ref ABI_FINGERPRINT: u32 = compute_abi_fingerprint();
}

/// Fingerprint of the bytecode ABI: every opcode (discriminant, name,
/// operand widths, in enum order) and every builtin (index, name, in table
/// order — `OpGetBuiltin` operands are indexes into that table). This is a
/// compatibility sentinel, not integrity protection: safety against forged
/// headers rests on the L1/L2/L3 checks, not on this value.
pub fn bytecode_abi_fingerprint() -> u32 {
    *ABI_FINGERPRINT
}

fn compute_abi_fingerprint() -> u32 {
    let mut hash = Fnv1a::new();
    for opcode in Opcode::iter() {
        let definition = DEFINITIONS
            .get(&opcode)
            .unwrap_or_else(|| panic!("opcode {:?} missing from DEFINITIONS", opcode));
        hash.absorb_u64(opcode as u64);
        hash.absorb_bytes(definition.name().as_bytes());
        for &width in definition.operand_widths() {
            hash.absorb_u64(width as u64);
        }
    }
    for (index, builtin) in BuiltIns.iter().enumerate() {
        hash.absorb_u64(index as u64);
        hash.absorb_bytes(builtin.name.as_bytes());
    }
    hash.finish()
}

/// FNV-1a, 32-bit.
struct Fnv1a(u32);

impl Fnv1a {
    fn new() -> Self {
        Fnv1a(0x811c_9dc5)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u32::from(byte);
            self.0 = self.0.wrapping_mul(0x0100_0193);
        }
    }

    /// Absorb one field as (ULEB length, content) so adjacent fields cannot
    /// be reinterpreted across their boundary.
    fn absorb_bytes(&mut self, bytes: &[u8]) {
        let mut length = Vec::new();
        write_uleb128(&mut length, bytes.len() as u64);
        self.write(&length);
        self.write(bytes);
    }

    fn absorb_u64(&mut self, value: u64) {
        let mut encoded = Vec::new();
        write_uleb128(&mut encoded, value);
        self.absorb_bytes(&encoded);
    }

    fn finish(&self) -> u32 {
        self.0
    }
}

/// Serialize `bytecode` into the `.mbc` container. With `strip_debug` the
/// debug section is omitted entirely (flags bit 0 cleared).
///
/// Output is deterministic: `function_debug_info` entries are written in
/// ascending constant-index order.
pub fn write_bytecode(
    bytecode: &Bytecode,
    strip_debug: bool,
) -> Result<Vec<u8>, SnapshotWriteError> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&bytecode_abi_fingerprint().to_le_bytes());
    out.push(if strip_debug { 0 } else { FLAG_HAS_DEBUG_INFO });

    write_bytes(&mut out, &bytecode.instructions.data);
    write_uleb128(&mut out, bytecode.constants.len() as u64);
    for (index, constant) in bytecode.constants.iter().enumerate() {
        write_constant(&mut out, index, constant)?;
    }

    if !strip_debug {
        write_debug_info(&mut out, &bytecode.debug_info);
        let mut entries: Vec<_> = bytecode.function_debug_info.iter().collect();
        entries.sort_by_key(|(index, _)| **index);
        write_uleb128(&mut out, entries.len() as u64);
        for (index, debug_info) in entries {
            write_uleb128(&mut out, *index as u64);
            write_debug_info(&mut out, debug_info);
        }
    }
    Ok(out)
}

fn write_constant(
    out: &mut Vec<u8>,
    index: usize,
    constant: &Object,
) -> Result<(), SnapshotWriteError> {
    match constant {
        Object::Integer(value) => {
            out.push(TAG_INTEGER);
            write_sleb128(out, *value);
        }
        Object::String(value) => {
            out.push(TAG_STRING);
            write_string(out, value);
        }
        Object::CompiledFunction(function) => {
            out.push(TAG_FUNCTION);
            write_string(out, &function.name);
            write_uleb128(out, function.num_locals as u64);
            write_uleb128(out, function.num_parameters as u64);
            write_bytes(out, &function.instructions);
        }
        other => {
            return Err(SnapshotWriteError::UnsupportedConstant {
                index,
                kind: object_kind(other).to_string(),
            })
        }
    }
    Ok(())
}

fn write_debug_info(out: &mut Vec<u8>, debug_info: &DebugInfo) {
    write_uleb128(out, debug_info.pc_spans.len() as u64);
    for pc_span in &debug_info.pc_spans {
        write_uleb128(out, pc_span.pc as u64);
        write_uleb128(out, pc_span.span.start as u64);
        write_uleb128(out, pc_span.span.end as u64);
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_bytes(out, value.as_bytes());
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    write_uleb128(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

pub(crate) fn write_uleb128(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub(crate) fn write_sleb128(out: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        let sign_bit_clear = byte & 0x40 == 0;
        if (value == 0 && sign_bit_clear) || (value == -1 && !sign_bit_clear) {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn object_kind(object: &Object) -> &'static str {
    match object {
        Object::Integer(_) => "Integer",
        Object::Boolean(_) => "Boolean",
        Object::String(_) => "String",
        Object::Array(_) => "Array",
        Object::Hash(_) => "Hash",
        Object::Null => "Null",
        Object::ReturnValue(_) => "ReturnValue",
        Object::Function(..) => "Function",
        Object::Builtin(_) => "Builtin",
        Object::Error(_) => "Error",
        Object::CompiledFunction(_) => "CompiledFunction",
        Object::ClosureObj(_) => "Closure",
        Object::Class(_) => "Class",
        Object::Instance(_) => "Instance",
        Object::BoundMethod(_) => "BoundMethod",
    }
}

/// Deserialize and validate an `.mbc` buffer. The input is untrusted: every
/// malformed input returns `Err`, and anything returned `Ok` has passed the
/// L1 checks (§6 of the design doc).
pub fn read_bytecode(buf: &[u8]) -> Result<Bytecode, SnapshotError> {
    let mut reader = Reader::new(buf);

    let magic = reader.read_exact(MAGIC.len())?;
    if magic != MAGIC {
        return Err(SnapshotError::BadMagic);
    }
    let version = reader.read_u8()?;
    if version != FORMAT_VERSION {
        return Err(SnapshotError::UnsupportedVersion {
            found: version,
            expected: FORMAT_VERSION,
        });
    }
    let found = u32::from_le_bytes(reader.read_exact(4)?.try_into().unwrap());
    let expected = bytecode_abi_fingerprint();
    if found != expected {
        return Err(SnapshotError::AbiFingerprintMismatch {
            found,
            expected,
        });
    }
    let flags = reader.read_u8()?;
    if flags & !FLAG_HAS_DEBUG_INFO != 0 {
        return Err(SnapshotError::BadFlags(flags));
    }
    let has_debug = flags & FLAG_HAS_DEBUG_INFO != 0;

    let main_instructions = reader.read_length_prefixed_bytes()?.to_vec();
    let constant_count = reader.read_count()?;
    let mut constants: Vec<Rc<Object>> = Vec::with_capacity(constant_count);
    for _ in 0..constant_count {
        constants.push(Rc::new(read_constant(&mut reader)?));
    }

    let (debug_info, function_debug_info) = if has_debug {
        read_debug_section(&mut reader, &constants, main_instructions.len())?
    } else {
        (DebugInfo::default(), HashMap::new())
    };

    if reader.remaining() != 0 {
        return Err(SnapshotError::TrailingBytes);
    }

    validate_instruction_stream("main", &main_instructions, &constants)?;
    for (index, constant) in constants.iter().enumerate() {
        if let Object::CompiledFunction(function) = constant.as_ref() {
            validate_instruction_stream(
                &format!("constant {}", index),
                &function.instructions,
                &constants,
            )?;
        }
    }

    Ok(Bytecode {
        instructions: Instructions {
            data: main_instructions,
        },
        constants,
        debug_info,
        function_debug_info,
    })
}

fn read_constant(reader: &mut Reader) -> Result<Object, SnapshotError> {
    let tag = reader.read_u8()?;
    match tag {
        TAG_INTEGER => Ok(Object::Integer(reader.read_sleb128()?)),
        TAG_STRING => Ok(Object::String(reader.read_string()?)),
        TAG_FUNCTION => {
            let name = reader.read_string()?;
            let num_locals = reader.read_usize()?;
            let num_parameters = reader.read_usize()?;
            let instructions = reader.read_length_prefixed_bytes()?.to_vec();
            Ok(Object::CompiledFunction(Rc::new(CompiledFunction {
                name,
                instructions,
                num_locals,
                num_parameters,
            })))
        }
        other => Err(SnapshotError::BadTag(other)),
    }
}

fn read_debug_section(
    reader: &mut Reader,
    constants: &[Rc<Object>],
    main_len: usize,
) -> Result<(DebugInfo, HashMap<usize, DebugInfo>), SnapshotError> {
    let main_debug = read_debug_info(reader, main_len)?;
    let entry_count = reader.read_count()?;
    let mut function_debug_info = HashMap::with_capacity(entry_count);
    for _ in 0..entry_count {
        let constant_index = reader.read_usize()?;
        let function_len = match constants.get(constant_index).map(Rc::as_ref) {
            Some(Object::CompiledFunction(function)) => function.instructions.len(),
            _ => return Err(SnapshotError::DebugIndexNotFunction(constant_index)),
        };
        let debug_info = read_debug_info(reader, function_len)?;
        if function_debug_info
            .insert(constant_index, debug_info)
            .is_some()
        {
            return Err(SnapshotError::DuplicateDebugEntry(constant_index));
        }
    }
    Ok((main_debug, function_debug_info))
}

fn read_debug_info(
    reader: &mut Reader,
    instruction_len: usize,
) -> Result<DebugInfo, SnapshotError> {
    let count = reader.read_count()?;
    let mut pc_spans = Vec::with_capacity(count);
    let mut previous: Option<usize> = None;
    for _ in 0..count {
        let pc = reader.read_usize()?;
        if let Some(previous) = previous {
            if pc <= previous {
                return Err(SnapshotError::DebugPcNotIncreasing {
                    pc,
                });
            }
        }
        if pc > instruction_len {
            return Err(SnapshotError::DebugPcOutOfRange {
                pc,
                len: instruction_len,
            });
        }
        let start = reader.read_usize()?;
        let end = reader.read_usize()?;
        pc_spans.push(PcSpan {
            pc,
            span: Span {
                start,
                end,
            },
        });
        previous = Some(pc);
    }
    Ok(DebugInfo {
        pc_spans,
    })
}

/// L1 linear scan of one instruction stream (§6 of the design doc): every
/// opcode is defined, operands are complete, jumps land on instruction
/// boundaries (or one past the end), and index operands stay inside the
/// constant pool / builtin table with the constant kind each opcode needs.
///
/// Deliberately not checked here: stack depth, operand runtime types,
/// local/free index validity. Those depend on execution state and are the
/// VM's defensive checks (L3).
fn validate_instruction_stream(
    stream: &str,
    instructions: &[u8],
    constants: &[Rc<Object>],
) -> Result<(), SnapshotError> {
    let len = instructions.len();
    let mut is_boundary = vec![false; len + 1];
    let mut jumps: Vec<(usize, usize)> = Vec::new();
    let mut offset = 0;
    while offset < len {
        is_boundary[offset] = true;
        let byte = instructions[offset];
        let opcode = Opcode::from_repr(byte)
            .ok_or_else(|| invalid(stream, offset, format!("unknown opcode 0x{:02x}", byte)))?;
        let definition = DEFINITIONS.get(&opcode).expect("missing opcode definition");
        let operand_len: usize = definition
            .operand_widths()
            .iter()
            .map(|w| *w as usize)
            .sum();
        if offset + 1 + operand_len > len {
            return Err(invalid(
                stream,
                offset,
                format!("truncated operands for {}", definition.name()),
            ));
        }
        let (operands, _) = read_operands(definition, &instructions[offset + 1..]);
        match opcode {
            Opcode::OpJump | Opcode::OpJumpNotTruthy => jumps.push((offset, operands[0])),
            Opcode::OpConst => {
                if operands[0] >= constants.len() {
                    return Err(invalid(
                        stream,
                        offset,
                        format!("constant index {} out of range", operands[0]),
                    ));
                }
            }
            Opcode::OpClosure => {
                let index = operands[0];
                if !matches!(
                    constants.get(index).map(Rc::as_ref),
                    Some(Object::CompiledFunction(_))
                ) {
                    return Err(invalid(
                        stream,
                        offset,
                        format!("OpClosure needs a function constant at index {}", index),
                    ));
                }
            }
            Opcode::OpClass | Opcode::OpMethod | Opcode::OpGetProperty | Opcode::OpSetProperty => {
                let index = operands[0];
                if !matches!(constants.get(index).map(Rc::as_ref), Some(Object::String(_))) {
                    return Err(invalid(
                        stream,
                        offset,
                        format!("{} needs a string constant at index {}", definition.name(), index),
                    ));
                }
            }
            Opcode::OpGetBuiltin => {
                if operands[0] >= BuiltIns.len() {
                    return Err(invalid(
                        stream,
                        offset,
                        format!("builtin index {} out of range", operands[0]),
                    ));
                }
            }
            Opcode::OpHash if operands[0] % 2 != 0 => {
                return Err(invalid(
                    stream,
                    offset,
                    format!("OpHash needs an even element count, got {}", operands[0]),
                ));
            }
            _ => {}
        }
        offset += 1 + operand_len;
    }
    is_boundary[len] = true;
    for (offset, target) in jumps {
        if target > len || !is_boundary[target] {
            return Err(invalid(
                stream,
                offset,
                format!("jump target {} is not an instruction boundary", target),
            ));
        }
    }
    Ok(())
}

fn invalid(stream: &str, offset: usize, message: String) -> SnapshotError {
    SnapshotError::InvalidInstruction(format!("{} (stream {}, offset {})", message, stream, offset))
}

pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Reader {
            buf,
            pos: 0,
        }
    }

    /// Cursor offset from the start of the buffer, for byte-range annotation
    /// (see `snapshot_layout`).
    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, SnapshotError> {
        let byte = *self.buf.get(self.pos).ok_or(SnapshotError::UnexpectedEof)?;
        self.pos += 1;
        Ok(byte)
    }

    pub(crate) fn read_exact(&mut self, len: usize) -> Result<&'a [u8], SnapshotError> {
        if len > self.remaining() {
            return Err(SnapshotError::UnexpectedEof);
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    /// Non-canonical encodings are accepted; only length and 64-bit range
    /// are enforced (§4.1 hard rules).
    pub(crate) fn read_uleb128(&mut self) -> Result<u64, SnapshotError> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        for _ in 0..10 {
            let byte = self.read_u8()?;
            let bits = u64::from(byte & 0x7f);
            if shift == 63 && bits > 1 {
                return Err(SnapshotError::InvalidLeb128);
            }
            result |= bits << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
        Err(SnapshotError::InvalidLeb128)
    }

    pub(crate) fn read_sleb128(&mut self) -> Result<i64, SnapshotError> {
        let mut result: i64 = 0;
        let mut shift = 0u32;
        for _ in 0..10 {
            let byte = self.read_u8()?;
            let bits = i64::from(byte & 0x7f);
            if shift == 63 {
                // Tenth byte: only one value bit is left in an i64, so the
                // payload must be all sign bits and end the encoding.
                if byte & 0x80 != 0 || (bits != 0 && bits != 0x7f) {
                    return Err(SnapshotError::InvalidLeb128);
                }
                return Ok(result | bits.wrapping_shl(63));
            }
            result |= bits << shift;
            if byte & 0x80 == 0 {
                if byte & 0x40 != 0 {
                    result |= -1i64 << (shift + 7);
                }
                return Ok(result);
            }
            shift += 7;
        }
        Err(SnapshotError::InvalidLeb128)
    }

    /// ULEB128 checked into `usize` (they differ on wasm32).
    pub(crate) fn read_usize(&mut self) -> Result<usize, SnapshotError> {
        let value = self.read_uleb128()?;
        usize::try_from(value).map_err(|_| SnapshotError::IntegerOverflow)
    }

    /// Entry count under the resource rule: every entry occupies at least
    /// one input byte, so a count above the remaining input is rejected and
    /// `Vec::with_capacity(count)` stays O(input size).
    fn read_count(&mut self) -> Result<usize, SnapshotError> {
        let count = self.read_usize()?;
        if count > self.remaining() {
            return Err(SnapshotError::LimitExceeded);
        }
        Ok(count)
    }

    fn read_length_prefixed_bytes(&mut self) -> Result<&'a [u8], SnapshotError> {
        let len = self.read_usize()?;
        if len > self.remaining() {
            return Err(SnapshotError::LimitExceeded);
        }
        self.read_exact(len)
    }

    fn read_string(&mut self) -> Result<String, SnapshotError> {
        let bytes = self.read_length_prefixed_bytes()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| SnapshotError::BadUtf8)
    }
}
