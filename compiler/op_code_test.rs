#[cfg(test)]
mod tests {
    use crate::op_code::Opcode::{OpAdd, OpConst};
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
            Test { op: Opcode::OpAdd, operands: vec![], expected: vec![OpAdd as u8] },
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
            Test { op: Opcode::OpConst, operands: vec![65534], bytes_read: 2 },
            Test { op: Opcode::OpConst, operands: vec![255], bytes_read: 2 },
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
        let ins = vec![
            make_instructions(OpAdd, &vec![]),
            make_instructions(OpConst, &vec![2]),
            make_instructions(OpConst, &vec![65535]),
        ];

        let expected = "0000 OpAdd\n\
                             0001 OpConst 2\n\
                             0004 OpConst 65535\n";
        // how-to-concatenate-immutable-vectors-in-one-line
        // https://stackoverflow.com/a/69578632/1713757
        let merged_ins = ins
            .iter()
            .fold(vec![], |sum, i| [sum.as_slice(), i.data.as_slice()].concat());

        let concatted = Instructions { data: merged_ins }.string();

        assert_eq!(concatted, expected);
    }
}
