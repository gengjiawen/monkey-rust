//! Mutation test for the VM's L3 defences.
//!
//! `snapshot.rs` validates the shape of a `.mbc` file (L1) but deliberately
//! leaves stack depth and local/free index validity to the VM, because those
//! depend on execution state. This flips single bytes in a real snapshot,
//! keeps the mutants `read_bytecode` accepts, and runs each one: the VM has to
//! answer with a `VmRuntimeError` rather than an index-out-of-bounds panic or
//! an endless loop.
#[cfg(test)]
mod tests {
    use parser::parse;

    use crate::compiler::Compiler;
    use crate::snapshot::{read_bytecode, write_bytecode};
    use crate::vm::VM;

    /// Exercises every opcode family with an index operand: constants,
    /// globals, locals, free variables, builtins, arrays, hashes, calls,
    /// classes and properties.
    const PROGRAM: &str = r#"
        let numbers = [1, 2, 3];
        let table = {"a": 1, "b": 2};
        let adder = fn(base) { fn(extra) { base + extra + len(numbers) } };
        let add_two = adder(2);
        class Counter {
            constructor(start) { this.value = start; }
            next() { this.value = this.value + 1; this.value }
        }
        let counter = new Counter(add_two(3));
        let total = counter.next() + table["a"] + numbers[0];
        if (total > 0) { puts(total); }
        total
    "#;

    /// A mutant that loops forever is a legitimate outcome — a jump can be
    /// bent onto itself by one byte — so every run gets a budget.
    const INSTRUCTION_BUDGET: usize = 100_000;

    fn snapshot() -> Vec<u8> {
        let program = parse(PROGRAM).expect("program should parse");
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).expect("program should compile");
        write_bytecode(&bytecode, false).expect("bytecode should serialize")
    }

    #[test]
    fn the_vm_survives_every_mutant_read_bytecode_accepts() {
        let original = snapshot();
        let mut accepted = 0;
        let mut errors = 0;

        // Deterministic: every byte of the file, flipped through a fixed set
        // of replacements. No RNG, so a failure reproduces from the report.
        for offset in 0..original.len() {
            for delta in [1u8, 7, 64, 128, 255] {
                let mut mutant = original.clone();
                mutant[offset] = mutant[offset].wrapping_add(delta);
                let Ok(bytecode) = read_bytecode(&mutant) else {
                    continue;
                };
                accepted += 1;
                let mut vm = VM::new(bytecode);
                if vm.run_with_budget(INSTRUCTION_BUDGET).is_err() {
                    errors += 1;
                }
            }
        }

        // The point of the test is that the loop above finished at all. These
        // guard against it passing vacuously if the corpus ever stops
        // producing readable mutants.
        assert!(
            accepted > 100,
            "expected read_bytecode to accept a meaningful sample, got {}",
            accepted
        );
        assert!(
            errors > 0,
            "expected some mutants to be rejected at runtime, got {} of {}",
            errors,
            accepted
        );
        println!("{} accepted mutants, {} rejected at runtime", accepted, errors);
    }

    /// Hand-assembled streams for the individual unchecked sites, so the
    /// error each one raises is pinned rather than only "not a panic".
    #[test]
    fn out_of_range_operands_are_runtime_errors() {
        use crate::compiler::{Bytecode, DebugInfo};
        use crate::op_code::{make_instructions, Instructions, Opcode};
        use crate::vm::VmRuntimeErrorKind;

        let cases: Vec<(Vec<u8>, VmRuntimeErrorKind, &str)> = vec![
            (
                make_instructions(Opcode::OpPop, &[]).data,
                VmRuntimeErrorKind::Stack,
                "stack underflow",
            ),
            (
                make_instructions(Opcode::OpArray, &[4]).data,
                VmRuntimeErrorKind::Stack,
                "stack underflow",
            ),
            (
                make_instructions(Opcode::OpCall, &[3]).data,
                VmRuntimeErrorKind::Stack,
                "stack underflow",
            ),
            (
                make_instructions(Opcode::OpGetFree, &[7]).data,
                VmRuntimeErrorKind::InvalidBytecode,
                "free variable index 7 out of range",
            ),
            (
                make_instructions(Opcode::OpConst, &[9]).data,
                VmRuntimeErrorKind::InvalidBytecode,
                "constant index 9 out of range",
            ),
            (
                make_instructions(Opcode::OpGetBuiltin, &[200]).data,
                VmRuntimeErrorKind::InvalidBytecode,
                "builtin index 200 out of range",
            ),
        ];

        for (mut data, kind, message) in cases {
            // The dispatch loop stops one byte before the end, so every
            // stream needs a trailing instruction it never reaches.
            data.extend(make_instructions(Opcode::OpNull, &[]).data);
            let mut vm = VM::new(Bytecode {
                instructions: Instructions {
                    data,
                },
                constants: vec![],
                debug_info: DebugInfo::default(),
                function_debug_info: Default::default(),
            });
            let result = vm.run_with_budget(INSTRUCTION_BUDGET);
            assert!(result.is_err(), "{:?} should be a runtime error", message);
            let error = result.unwrap_err();
            assert_eq!(error.kind, kind, "{}", error.message);
            assert_eq!(error.message, message);
        }
    }

    #[test]
    fn a_mutant_that_loops_forever_hits_the_budget() {
        // OpJump back to its own offset: the smallest infinite loop.
        let program = parse("let x = 1; if (x > 0) { x } else { x }; x").unwrap();
        let mut compiler = Compiler::new();
        let mut bytecode = compiler.compile(&program).unwrap();
        let jump = bytecode
            .instructions
            .data
            .iter()
            .position(|byte| *byte == crate::op_code::Opcode::OpJump as u8)
            .expect("program should contain a jump");
        let target = (jump as u16).to_be_bytes();
        bytecode.instructions.data[jump + 1] = target[0];
        bytecode.instructions.data[jump + 2] = target[1];

        let mut vm = VM::new(bytecode);
        let error = vm
            .run_with_budget(1_000)
            .expect_err("the jump loops forever");
        assert_eq!(error.kind, crate::vm::VmRuntimeErrorKind::ExecutionLimit);
        assert_eq!(error.message, "instruction limit exceeded (budget: 1000)");
    }
}
