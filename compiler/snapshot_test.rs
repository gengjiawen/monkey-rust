#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::rc::Rc;

    use object::Object;
    use parser::parse;

    use crate::compiler::{Bytecode, Compiler, DebugInfo};
    use crate::op_code::{Instructions, Opcode};
    use crate::snapshot::{
        bytecode_abi_fingerprint, read_bytecode, write_bytecode, write_sleb128, write_uleb128,
        Reader, SnapshotError, SnapshotWriteError, FLAG_HAS_DEBUG_INFO, FORMAT_VERSION, MAGIC,
        TAG_FUNCTION, TAG_INTEGER, TAG_STRING,
    };

    fn compile(source: &str) -> Bytecode {
        let program = parse(source).unwrap();
        let mut compiler = Compiler::new();
        compiler.compile(&program).unwrap()
    }

    fn header(flags: u8) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.push(FORMAT_VERSION);
        out.extend_from_slice(&bytecode_abi_fingerprint().to_le_bytes());
        out.push(flags);
        out
    }

    /// Hand-assemble a file. ULEB128 lengths are written as single bytes,
    /// which is only correct because every length in these tests is < 128.
    /// `constants` and `debug` are raw section bytes including their counts.
    fn raw_file(flags: u8, main: &[u8], constants: &[u8], debug: &[u8]) -> Vec<u8> {
        let mut out = header(flags);
        out.push(main.len() as u8);
        out.extend_from_slice(main);
        out.extend_from_slice(constants);
        out.extend_from_slice(debug);
        out
    }

    fn hexdump(bytes: &[u8]) -> String {
        bytes
            .chunks(16)
            .enumerate()
            .map(|(line, chunk)| {
                let hex = chunk
                    .iter()
                    .map(|byte| format!("{:02x}", byte))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{:08x}  {}", line * 16, hex)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    const ROUNDTRIP_SOURCE: &str = r#"
        let greeting = "hello";
        class Point {
          constructor(x, y) { this.x = x; this.y = y; }
          sum() { return this.x + this.y; }
        }
        let make = fn(a) { fn(b) { a + b + new Point(1, 2).sum() } };
        make(1)(len(greeting))
    "#;

    #[test]
    fn roundtrip_preserves_every_field() {
        let original = compile(ROUNDTRIP_SOURCE);
        assert!(!original.function_debug_info.is_empty());
        let blob = write_bytecode(&original, false).unwrap();
        let restored = read_bytecode(&blob).unwrap();
        assert_eq!(original.instructions, restored.instructions);
        assert_eq!(original.constants, restored.constants);
        assert_eq!(original.debug_info, restored.debug_info);
        assert_eq!(original.function_debug_info, restored.function_debug_info);
    }

    #[test]
    fn strip_debug_drops_debug_sections() {
        let original = compile(ROUNDTRIP_SOURCE);
        let blob = write_bytecode(&original, true).unwrap();
        let restored = read_bytecode(&blob).unwrap();
        assert_eq!(original.instructions, restored.instructions);
        assert_eq!(original.constants, restored.constants);
        assert!(restored.debug_info.pc_spans.is_empty());
        assert!(restored.function_debug_info.is_empty());
        let with_debug = write_bytecode(&original, false).unwrap();
        assert!(blob.len() < with_debug.len());
    }

    #[test]
    fn writer_rejects_unsupported_constants() {
        let bytecode = Bytecode {
            instructions: Instructions {
                data: vec![],
            },
            constants: vec![Rc::new(Object::Integer(1)), Rc::new(Object::Null)],
            debug_info: DebugInfo::default(),
            function_debug_info: HashMap::new(),
        };
        assert_eq!(
            write_bytecode(&bytecode, false),
            Err(SnapshotWriteError::UnsupportedConstant {
                index: 1,
                kind: "Null".to_string(),
            })
        );
    }

    #[test]
    fn serialization_is_deterministic() {
        let source = "let a = fn() { 1 }; let b = fn() { 2 }; a() + b()";
        let first = write_bytecode(&compile(source), false).unwrap();
        let second = write_bytecode(&compile(source), false).unwrap();
        assert_eq!(first, second);
    }

    // Golden layout snapshot. Update rule (design doc §8): if a diff touches
    // only the 4 fingerprint bytes (offsets 5..9), accept the new snapshot;
    // if it touches anything else the container format changed, so bump
    // FORMAT_VERSION first and then accept.
    #[test]
    fn golden_mbc_layout() {
        let blob =
            write_bytecode(&compile("let add = fn(a, b) { a + b }; add(1, 2)"), false).unwrap();
        insta::assert_snapshot!(hexdump(&blob));
    }

    #[test]
    fn uleb128_roundtrip() {
        for value in [
            0u64,
            1,
            127,
            128,
            300,
            16383,
            16384,
            u64::from(u32::MAX),
            u64::MAX,
        ] {
            let mut out = Vec::new();
            write_uleb128(&mut out, value);
            assert!(out.len() <= 10);
            let mut reader = Reader::new(&out);
            assert_eq!(reader.read_uleb128().unwrap(), value, "value {}", value);
        }
    }

    #[test]
    fn sleb128_roundtrip() {
        for value in [0i64, 1, -1, 63, -64, 64, -65, 127, -128, i64::MAX, i64::MIN] {
            let mut out = Vec::new();
            write_sleb128(&mut out, value);
            assert!(out.len() <= 10);
            let mut reader = Reader::new(&out);
            assert_eq!(reader.read_sleb128().unwrap(), value, "value {}", value);
        }
    }

    #[test]
    fn uleb128_accepts_non_canonical() {
        let mut reader = Reader::new(&[0x80, 0x00]);
        assert_eq!(reader.read_uleb128().unwrap(), 0);
    }

    #[test]
    fn uleb128_rejects_eleven_byte_encodings() {
        let mut reader = Reader::new(&[0x80; 10]);
        assert_eq!(reader.read_uleb128(), Err(SnapshotError::InvalidLeb128));
    }

    #[test]
    fn uleb128_rejects_65_bit_values() {
        let mut encoded = vec![0xff; 9];
        encoded.push(0x02); // bit 64
        let mut reader = Reader::new(&encoded);
        assert_eq!(reader.read_uleb128(), Err(SnapshotError::InvalidLeb128));
    }

    #[test]
    fn sleb128_rejects_i64_overflow() {
        let mut encoded = vec![0xff; 9];
        encoded.push(0x3f); // payload beyond the final sign bit
        let mut reader = Reader::new(&encoded);
        assert_eq!(reader.read_sleb128(), Err(SnapshotError::InvalidLeb128));
    }

    #[test]
    fn fingerprint_is_stable_within_a_build() {
        assert_eq!(bytecode_abi_fingerprint(), bytecode_abi_fingerprint());
    }

    #[test]
    fn minimal_empty_program_is_valid() {
        let restored = read_bytecode(&raw_file(0, &[], &[0], &[])).unwrap();
        assert!(restored.instructions.data.is_empty());
        assert!(restored.constants.is_empty());
        assert!(restored.debug_info.pc_spans.is_empty());
        assert!(restored.function_debug_info.is_empty());
    }

    #[test]
    fn rejects_bad_magic() {
        let mut blob = raw_file(0, &[], &[0], &[]);
        blob[0] = b'X';
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::BadMagic));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut blob = raw_file(0, &[], &[0], &[]);
        blob[4] = 0xff;
        assert_eq!(
            read_bytecode(&blob),
            Err(SnapshotError::UnsupportedVersion {
                found: 0xff,
                expected: FORMAT_VERSION,
            })
        );
    }

    #[test]
    fn rejects_fingerprint_mismatch() {
        let mut blob = raw_file(0, &[], &[0], &[]);
        blob[5] ^= 0xff;
        assert!(matches!(read_bytecode(&blob), Err(SnapshotError::AbiFingerprintMismatch { .. })));
    }

    #[test]
    fn rejects_unknown_flags() {
        let blob = raw_file(0b0000_0010, &[], &[0], &[]);
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::BadFlags(0b10)));
    }

    #[test]
    fn rejects_header_only_input() {
        assert_eq!(read_bytecode(&header(0)), Err(SnapshotError::UnexpectedEof));
    }

    #[test]
    fn rejects_length_beyond_remaining_input() {
        let mut blob = header(0);
        blob.extend_from_slice(&[0x10, 0x01, 0x02]); // main claims 16 bytes, 2 remain
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::LimitExceeded));
    }

    #[test]
    fn rejects_count_beyond_remaining_input() {
        let mut blob = header(0);
        blob.push(0); // empty main
        blob.push(0x7f); // 127 constants declared, 0 bytes remain
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::LimitExceeded));
    }

    #[test]
    fn rejects_overlong_varint_in_file() {
        let mut blob = header(0);
        blob.extend_from_slice(&[0x80; 10]); // main length varint never terminates
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::InvalidLeb128));
    }

    #[test]
    fn rejects_unknown_constant_tag() {
        let blob = raw_file(0, &[], &[1, 9], &[]);
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::BadTag(9)));
    }

    #[test]
    fn rejects_invalid_utf8_string_constant() {
        let blob = raw_file(0, &[], &[1, TAG_STRING, 2, 0xff, 0xfe], &[]);
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::BadUtf8));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let blob = raw_file(0, &[], &[0], &[0x00]);
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::TrailingBytes));
    }

    fn assert_invalid_instruction(blob: &[u8]) {
        match read_bytecode(blob) {
            Err(SnapshotError::InvalidInstruction(_)) => {}
            other => panic!("expected InvalidInstruction, got {:?}", other),
        }
    }

    #[test]
    fn rejects_unknown_opcode_byte() {
        assert_invalid_instruction(&raw_file(0, &[0xff], &[0], &[]));
    }

    #[test]
    fn rejects_truncated_operands() {
        assert_invalid_instruction(&raw_file(0, &[Opcode::OpConst as u8, 0], &[0], &[]));
    }

    #[test]
    fn rejects_jump_into_operand_bytes() {
        // Offset 1 is inside OpJump's own operand, not a boundary.
        assert_invalid_instruction(&raw_file(0, &[Opcode::OpJump as u8, 0, 1], &[0], &[]));
    }

    #[test]
    fn accepts_jump_to_stream_end() {
        let blob = raw_file(0, &[Opcode::OpJump as u8, 0, 3], &[0], &[]);
        assert!(read_bytecode(&blob).is_ok());
    }

    #[test]
    fn rejects_constant_index_out_of_range() {
        assert_invalid_instruction(&raw_file(0, &[Opcode::OpConst as u8, 0, 0], &[0], &[]));
    }

    #[test]
    fn rejects_closure_over_non_function_constant() {
        let blob = raw_file(0, &[Opcode::OpClosure as u8, 0, 0, 0], &[1, TAG_INTEGER, 7], &[]);
        assert_invalid_instruction(&blob);
    }

    #[test]
    fn rejects_class_name_that_is_not_a_string() {
        let blob = raw_file(0, &[Opcode::OpClass as u8, 0, 0], &[1, TAG_INTEGER, 7], &[]);
        assert_invalid_instruction(&blob);
    }

    #[test]
    fn rejects_builtin_index_out_of_range() {
        assert_invalid_instruction(&raw_file(0, &[Opcode::OpGetBuiltin as u8, 200], &[0], &[]));
    }

    #[test]
    fn rejects_odd_hash_element_count() {
        assert_invalid_instruction(&raw_file(0, &[Opcode::OpHash as u8, 0, 1], &[0], &[]));
    }

    #[test]
    fn validates_function_constant_instruction_streams() {
        // TAG_FUNCTION: empty name, 0 locals, 0 params, body = one unknown byte.
        let blob = raw_file(0, &[], &[1, TAG_FUNCTION, 0, 0, 0, 1, 0xff], &[]);
        assert_invalid_instruction(&blob);
    }

    #[test]
    fn rejects_debug_pc_out_of_range() {
        let main = [Opcode::OpTrue as u8, Opcode::OpPop as u8];
        // main_debug: 1 entry { pc 9, span 0..1 }, then 0 function entries.
        let blob = raw_file(FLAG_HAS_DEBUG_INFO, &main, &[0], &[1, 9, 0, 1, 0]);
        assert_eq!(
            read_bytecode(&blob),
            Err(SnapshotError::DebugPcOutOfRange {
                pc: 9,
                len: 2
            })
        );
    }

    #[test]
    fn rejects_debug_pc_not_increasing() {
        let main = [Opcode::OpTrue as u8, Opcode::OpPop as u8];
        // main_debug: 2 entries with pc 1, 1.
        let blob = raw_file(FLAG_HAS_DEBUG_INFO, &main, &[0], &[2, 1, 0, 1, 1, 0, 1, 0]);
        assert_eq!(
            read_bytecode(&blob),
            Err(SnapshotError::DebugPcNotIncreasing {
                pc: 1
            })
        );
    }

    #[test]
    fn rejects_duplicate_function_debug_entries() {
        // One function constant with an empty body; two fn_debug entries for it.
        let constants = [1, TAG_FUNCTION, 0, 0, 0, 0];
        let blob = raw_file(FLAG_HAS_DEBUG_INFO, &[], &constants, &[0, 2, 0, 0, 0, 0]);
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::DuplicateDebugEntry(0)));
    }

    #[test]
    fn rejects_debug_entry_for_non_function_constant() {
        let blob = raw_file(FLAG_HAS_DEBUG_INFO, &[], &[1, TAG_INTEGER, 7], &[0, 1, 0, 0]);
        assert_eq!(read_bytecode(&blob), Err(SnapshotError::DebugIndexNotFunction(0)));
    }

    // Fuzz-lite (design doc §8): reading arbitrarily truncated or corrupted
    // input must never panic. Ok results are acceptable — flipping a bit in
    // an integer payload just yields a different valid file.
    #[test]
    fn read_never_panics_on_truncated_or_flipped_input() {
        let blob = write_bytecode(&compile(ROUNDTRIP_SOURCE), false).unwrap();
        for end in 0..blob.len() {
            let _ = read_bytecode(&blob[..end]);
        }
        for index in 0..blob.len() {
            for pattern in [0x01u8, 0x80, 0xff] {
                let mut mutated = blob.clone();
                mutated[index] ^= pattern;
                let _ = read_bytecode(&mutated);
            }
        }
    }
}
