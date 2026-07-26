#[cfg(test)]
mod tests {
    use crate::op_code::Opcode::{OpAdd, OpClosure, OpConst, OpGetLocal};
    use crate::op_code::*;
    use std::collections::HashSet;
    use strum::EnumCount;

    #[test]
    fn test_make() {
        struct Test {
            op: Opcode,
            operands: Vec<usize>,
            expected: Vec<u8>,
        }

        let tests = vec![
            Test {
                op: Opcode::OpConst,
                operands: vec![65534],
                expected: vec![OpConst as u8, 255, 254],
            },
            Test {
                op: Opcode::OpAdd,
                operands: vec![],
                expected: vec![OpAdd as u8],
            },
            Test {
                op: Opcode::OpGetLocal,
                operands: vec![255],
                expected: vec![OpGetLocal as u8, 255],
            },
        ];

        for t in tests {
            let ins = make_instructions(t.op, &t.operands);
            assert_eq!(ins.data, t.expected)
        }
    }

    #[test]
    fn test_read_operands() {
        struct Test {
            op: Opcode,
            operands: Vec<usize>,
            bytes_read: usize,
        }

        let tests = vec![
            Test {
                op: Opcode::OpConst,
                operands: vec![65534],
                bytes_read: 2,
            },
            Test {
                op: Opcode::OpConst,
                operands: vec![255],
                bytes_read: 2,
            },
        ];

        for t in tests {
            let ins = make_instructions(t.op, &t.operands);
            let (operands_read, n) = read_operands(DEFINITIONS.get(&t.op).unwrap(), &ins.data[1..]);
            assert_eq!(operands_read, t.operands);
            assert_eq!(n, t.bytes_read);
        }
    }
    #[test]
    fn test_instructions_legal() {
        let opcode_count = Opcode::COUNT;
        let keys_count = DEFINITIONS.keys().count();
        let op_keys = DEFINITIONS
            .values()
            .map(|d| d.name.to_string())
            .collect::<HashSet<String>>();
        assert_eq!(opcode_count, keys_count);
        // description is distinct
        assert_eq!(opcode_count, op_keys.len());
    }

    #[test]
    fn test_instructions_string() {
        let ins = [
            make_instructions(OpAdd, &[]),
            make_instructions(OpGetLocal, &[1]),
            make_instructions(OpConst, &[2]),
            make_instructions(OpConst, &[65535]),
            make_instructions(OpClosure, &[65535, 255]),
        ];

        let expected = "0000 OpAdd\n\
                             0001 OpGetLocal 1\n\
                             0003 OpConst 2\n\
                             0006 OpConst 65535\n\
                             0009 OpClosure 65535 255\n";
        // how-to-concatenate-immutable-vectors-in-one-line
        // https://stackoverflow.com/a/69578632/1713757
        let merged_ins = ins
            .iter()
            .fold(vec![], |sum, i| [sum.as_slice(), i.data.as_slice()].concat());

        let concatted = Instructions {
            data: merged_ins,
        }
        .string();

        assert_eq!(concatted, expected);
    }

    // `make_instructions` truncates with `as u8` / `as u16`, which turns an
    // oversized operand into a silent miscompile. The compiler is expected to
    // reject those before emitting; these asserts are the backstop that turns
    // a future missed call site into a loud failure instead. Deliberately not
    // `debug_assert!` — release builds are the ones shipping the bytecode, so
    // these tests must exercise the same code path CI does under `--release`.
    #[test]
    #[should_panic(expected = "OpGetLocal operand 256 does not fit in 1 byte")]
    fn make_instructions_rejects_an_oversized_u8_operand() {
        make_instructions(OpGetLocal, &[256]);
    }

    #[test]
    #[should_panic(expected = "OpConst operand 65536 does not fit in 2 bytes")]
    fn make_instructions_rejects_an_oversized_u16_operand() {
        make_instructions(OpConst, &[65536]);
    }

    #[test]
    fn make_instructions_accepts_the_widest_operands() {
        assert_eq!(make_instructions(OpGetLocal, &[255]).data, vec![OpGetLocal as u8, 255]);
        assert_eq!(make_instructions(OpConst, &[65535]).data, vec![OpConst as u8, 255, 255]);
    }
}
