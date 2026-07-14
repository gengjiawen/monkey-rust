//! Byte-range annotations for `.mbc` snapshots (the playground inspector).
//!
//! [`describe_bytecode`] funnels the buffer through [`read_bytecode`] first —
//! the full L1 validation — and only then re-walks it, recording one region
//! per format field. Regions are emitted in file order, are never empty, and
//! tile the buffer exactly: the first starts at offset 0, each next region
//! starts where the previous one ends, and the last ends at `byte_length`.
//! Consumers can therefore render a complete annotated hexdump without gap
//! or overlap handling.

use std::convert::TryInto;

use serde::Serialize;

use crate::op_code::{read_operands, Opcode, DEFINITIONS};
use crate::snapshot::{
    read_bytecode, Reader, SnapshotError, FLAG_HAS_DEBUG_INFO, TAG_FUNCTION, TAG_INTEGER,
    TAG_STRING,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotSection {
    Header,
    Main,
    Constants,
    Debug,
}

/// One annotated byte range. `label` names the format field ("magic",
/// "const[0] tag", "OpConst 1"); `detail` explains the decoded value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRegion {
    pub offset: usize,
    pub length: usize,
    pub section: SnapshotSection,
    pub label: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLayout {
    pub byte_length: usize,
    pub format_version: u8,
    /// Rendered as `0x{:08x}`: a JSON string keeps the u32 exact in JS.
    pub abi_fingerprint: String,
    pub has_debug_info: bool,
    pub regions: Vec<SnapshotRegion>,
}

/// Annotate every byte of a validated `.mbc` buffer.
///
/// The buffer is untrusted input; it is validated with [`read_bytecode`]
/// before the annotating walk, so any `Err` here carries the same
/// [`SnapshotError`] the loader would report.
pub fn describe_bytecode(buf: &[u8]) -> Result<SnapshotLayout, SnapshotError> {
    read_bytecode(buf)?;
    Walker {
        buf,
        reader: Reader::new(buf),
        regions: Vec::new(),
    }
    .walk()
}

struct Walker<'a> {
    buf: &'a [u8],
    reader: Reader<'a>,
    regions: Vec<SnapshotRegion>,
}

impl<'a> Walker<'a> {
    fn walk(mut self) -> Result<SnapshotLayout, SnapshotError> {
        use SnapshotSection::{Constants, Debug, Header, Main};

        self.record(
            Header,
            "magic",
            |r| r.read_exact(4),
            |_| "file signature \"MBC\\0\"".to_string(),
        )?;
        let format_version = self.record(Header, "version", Reader::read_u8, |version| {
            format!("container format version {}", version)
        })?;
        let fingerprint_bytes = self.record(
            Header,
            "abi fingerprint",
            |r| r.read_exact(4),
            |bytes| {
                let value = u32::from_le_bytes(bytes[..4].try_into().expect("4-byte slice"));
                format!(
                    "0x{:08x} — FNV-1a over the opcode and builtin tables (little-endian)",
                    value
                )
            },
        )?;
        let abi_fingerprint = format!(
            "0x{:08x}",
            u32::from_le_bytes(fingerprint_bytes[..4].try_into().expect("4-byte slice"))
        );
        let flags = self.record(Header, "flags", Reader::read_u8, |flags| {
            if flags & FLAG_HAS_DEBUG_INFO != 0 {
                format!("0b{:08b} — debug info present", flags)
            } else {
                format!("0b{:08b} — debug info stripped", flags)
            }
        })?;
        let has_debug_info = flags & FLAG_HAS_DEBUG_INFO != 0;

        let main_len = self.record(Main, "main length", Reader::read_usize, |len| {
            format!("{} bytes of main instructions follow (ULEB128)", len)
        })?;
        self.record_instructions(Main, "main", main_len)?;

        let constant_count =
            self.record(Constants, "constant count", Reader::read_usize, |count| {
                format!("{} constants (ULEB128)", count)
            })?;
        // Stream names for function constants, reused by the debug section.
        let mut streams: Vec<String> = Vec::with_capacity(constant_count);
        for index in 0..constant_count {
            let tag_label = format!("const[{}] tag", index);
            let tag = self.record(Constants, &tag_label, Reader::read_u8, |tag| match *tag {
                TAG_INTEGER => "TAG_INTEGER (1) — SLEB128 value".to_string(),
                TAG_STRING => "TAG_STRING (2) — length-prefixed UTF-8".to_string(),
                TAG_FUNCTION => "TAG_FUNCTION (3) — name, locals, params, body".to_string(),
                other => format!("unknown tag {}", other),
            })?;
            streams.push(format!("const[{}]", index));
            match tag {
                TAG_INTEGER => {
                    self.record(
                        Constants,
                        format!("const[{}] value", index),
                        Reader::read_sleb128,
                        |value| format!("{} (SLEB128)", value),
                    )?;
                }
                TAG_STRING => {
                    self.record_str(Constants, &format!("const[{}] text", index))?;
                }
                TAG_FUNCTION => {
                    let name = self.record_str(Constants, &format!("const[{}] name", index))?;
                    self.record(
                        Constants,
                        format!("const[{}] locals", index),
                        Reader::read_usize,
                        |count| format!("{} local slots", count),
                    )?;
                    self.record(
                        Constants,
                        format!("const[{}] params", index),
                        Reader::read_usize,
                        |count| format!("{} parameters", count),
                    )?;
                    let body_len = self.record(
                        Constants,
                        format!("const[{}] body length", index),
                        Reader::read_usize,
                        |len| format!("{} bytes of function instructions follow (ULEB128)", len),
                    )?;
                    let stream = if name.is_empty() {
                        format!("const[{}] fn", index)
                    } else {
                        format!("fn {}", name)
                    };
                    self.record_instructions(Constants, &stream, body_len)?;
                    streams[index] = stream;
                }
                other => return Err(SnapshotError::BadTag(other)),
            }
        }

        if has_debug_info {
            self.record_debug_info("main")?;
            let entry_count =
                self.record(Debug, "debug fn count", Reader::read_usize, |count| {
                    format!("{} function debug entries", count)
                })?;
            for _ in 0..entry_count {
                let start = self.reader.position();
                let constant_index = self.reader.read_usize()?;
                let stream = streams
                    .get(constant_index)
                    .cloned()
                    .unwrap_or_else(|| format!("const[{}]", constant_index));
                self.push(
                    start,
                    Debug,
                    "debug fn index".to_string(),
                    format!("pc→span entries for {}", stream),
                );
                self.record_debug_info(&stream)?;
            }
        }

        // read_bytecode already rejected trailing bytes; this guards the
        // walker itself against drifting out of sync with the codec.
        if self.reader.position() != self.buf.len() {
            return Err(SnapshotError::TrailingBytes);
        }
        Ok(SnapshotLayout {
            byte_length: self.buf.len(),
            format_version,
            abi_fingerprint,
            has_debug_info,
            regions: self.regions,
        })
    }

    /// Read one field and record the byte range it occupied. Zero-length
    /// fields (e.g. the bytes of an empty string) produce no region.
    fn record<T>(
        &mut self,
        section: SnapshotSection,
        label: impl Into<String>,
        read: impl FnOnce(&mut Reader<'a>) -> Result<T, SnapshotError>,
        detail: impl FnOnce(&T) -> String,
    ) -> Result<T, SnapshotError> {
        let start = self.reader.position();
        let value = read(&mut self.reader)?;
        let detail = detail(&value);
        self.push(start, section, label.into(), detail);
        Ok(value)
    }

    fn push(&mut self, start: usize, section: SnapshotSection, label: String, detail: String) {
        let end = self.reader.position();
        if end == start {
            return;
        }
        self.regions.push(SnapshotRegion {
            offset: start,
            length: end - start,
            section,
            label,
            detail,
        });
    }

    /// Length prefix and UTF-8 bytes as two regions; returns the text.
    fn record_str(
        &mut self,
        section: SnapshotSection,
        label: &str,
    ) -> Result<String, SnapshotError> {
        let len = self.record(section, format!("{} length", label), Reader::read_usize, |len| {
            format!("{} bytes (ULEB128)", len)
        })?;
        let start = self.reader.position();
        let bytes = self.reader.read_exact(len)?;
        let text = String::from_utf8(bytes.to_vec()).map_err(|_| SnapshotError::BadUtf8)?;
        self.push(start, section, label.to_string(), format!("{:?}", text));
        Ok(text)
    }

    /// One region per instruction, labelled with its disassembly. The stream
    /// was already validated, but decode defensively anyway: the walker must
    /// never panic (it runs behind the wasm boundary).
    fn record_instructions(
        &mut self,
        section: SnapshotSection,
        stream: &str,
        len: usize,
    ) -> Result<(), SnapshotError> {
        let start = self.reader.position();
        let bytes = self.reader.read_exact(len)?;
        let mut pc = 0;
        while pc < len {
            let opcode = Opcode::from_repr(bytes[pc]).ok_or_else(|| {
                SnapshotError::InvalidInstruction(format!(
                    "unknown opcode 0x{:02x} (stream {}, offset {})",
                    bytes[pc], stream, pc
                ))
            })?;
            let definition = DEFINITIONS.get(&opcode).ok_or_else(|| {
                SnapshotError::InvalidInstruction(format!(
                    "missing definition for {:?} (stream {}, offset {})",
                    opcode, stream, pc
                ))
            })?;
            let operand_len: usize = definition
                .operand_widths()
                .iter()
                .map(|w| *w as usize)
                .sum();
            if pc + 1 + operand_len > len {
                return Err(SnapshotError::InvalidInstruction(format!(
                    "truncated operands for {} (stream {}, offset {})",
                    definition.name(),
                    stream,
                    pc
                )));
            }
            let (operands, _) = read_operands(definition, &bytes[pc + 1..]);
            let mut label = definition.name().to_string();
            for operand in &operands {
                label.push_str(&format!(" {}", operand));
            }
            let end = start + pc + 1 + operand_len;
            self.regions.push(SnapshotRegion {
                offset: start + pc,
                length: end - (start + pc),
                section,
                label,
                detail: format!("{} pc {:04}", stream, pc),
            });
            pc += 1 + operand_len;
        }
        Ok(())
    }

    /// Span count plus one region per `{pc, start, end}` triple.
    fn record_debug_info(&mut self, stream: &str) -> Result<(), SnapshotError> {
        let count = self.record(
            SnapshotSection::Debug,
            format!("{} span count", stream),
            Reader::read_usize,
            |count| format!("{} pc→span entries (ULEB128)", count),
        )?;
        for _ in 0..count {
            let start = self.reader.position();
            let pc = self.reader.read_usize()?;
            let span_start = self.reader.read_usize()?;
            let span_end = self.reader.read_usize()?;
            self.push(
                start,
                SnapshotSection::Debug,
                format!("{} pc {:04}", stream, pc),
                format!("source {}..{}", span_start, span_end),
            );
        }
        Ok(())
    }
}
