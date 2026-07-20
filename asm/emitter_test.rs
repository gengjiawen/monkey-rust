//! Emitter unit tests (design §10.3): encoding-limit tiers, span bookkeeping,
//! buffer splicing, and layout math — checked on the finished module text.

use parser::lexer::token::Span;

use crate::emitter::*;

/// Finishes a module around whatever was emitted and returns its lines.
fn finished_lines(emitter: Emitter) -> Vec<String> {
    emitter
        .finish(0, ".Lmain_exit", false)
        .text
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect()
}

fn instruction(line: &str) -> &str {
    let line = line.trim();
    match line.find("//") {
        Some(position) => line[..position].trim_end(),
        None => line,
    }
}

/// The emitted instructions between the main prologue and epilogue.
fn body_instructions(emitter: Emitter) -> Vec<String> {
    let lines = finished_lines(emitter);
    let start = lines
        .iter()
        .position(|line| instruction(line) == "bl rt_globals_init")
        .unwrap();
    let end = lines
        .iter()
        .position(|line| instruction(line) == ".Lmain_exit:")
        .unwrap();
    lines[start + 1..end]
        .iter()
        .map(|line| instruction(line).to_string())
        .collect()
}

#[test]
fn layout_math() {
    assert_eq!(slot_offset(0), 32);
    assert_eq!(slot_offset(1), 48);
    assert_eq!(CLOSURE_SLOT_OFFSET, 16);
    // callee + args, packed by 8, 16-aligned.
    assert_eq!(call_area_size(0), 16);
    assert_eq!(call_area_size(1), 16);
    assert_eq!(call_area_size(2), 32);
    assert_eq!(call_area_size(7), 64);
    assert_eq!(scratch_area_size(0), 0);
    assert_eq!(scratch_area_size(1), 16);
    assert_eq!(scratch_area_size(2), 16);
    assert_eq!(scratch_area_size(3), 32);
}

#[test]
fn load_imm64_skips_zero_halfwords() {
    let mut emitter = Emitter::new();
    emitter.load_imm64("x0", 0, "");
    emitter.load_imm64("x1", 5, "");
    emitter.load_imm64("x2", 0x0123_4567_89ab_cdef, "");
    emitter.load_imm64("x3", 0x1_0000_0000, ""); // only halfword 2 set
    emitter.load_imm64("x4", u64::MAX, "");
    assert_eq!(
        body_instructions(emitter),
        vec![
            "movz x0, #0",
            "movz x1, #0x5",
            "movz x2, #0xcdef",
            "movk x2, #0x89ab, lsl #16",
            "movk x2, #0x4567, lsl #32",
            "movk x2, #0x123, lsl #48",
            "movz x3, #0x1, lsl #32",
            "movz x4, #0xffff",
            "movk x4, #0xffff, lsl #16",
            "movk x4, #0xffff, lsl #32",
            "movk x4, #0xffff, lsl #48",
        ]
    );
}

#[test]
fn sp_adjustment_tiers() {
    let mut emitter = Emitter::new();
    emitter.sp_sub(0); // no code
    emitter.sp_sub(16);
    emitter.sp_sub(4080); // largest 16-multiple within #4095
    emitter.sp_sub(8192); // lsl #12, no low part
    emitter.sp_sub(8208); // lsl #12 + low part
    emitter.sp_add(16 * (1 << 24)); // beyond both immediates: via x8
    assert_eq!(
        body_instructions(emitter),
        vec![
            "sub sp, sp, #16",
            "sub sp, sp, #4080",
            "sub sp, sp, #2, lsl #12",
            "sub sp, sp, #2, lsl #12",
            "sub sp, sp, #16",
            "movz x8, #0x1000, lsl #16",
            "add sp, sp, x8",
        ]
    );
}

#[test]
fn frame_access_materializes_large_offsets() {
    let mut emitter = Emitter::new();
    emitter.frame_store("x0", 256, ""); // largest stur offset
    emitter.frame_store("x0", slot_offset(15), ""); // 272: beyond stur
    emitter.frame_load("x0", 16, "");
    emitter.frame_load("x0", 272, "");
    assert_eq!(
        body_instructions(emitter),
        vec![
            "stur x0, [x29, #-256]",
            "movz x8, #0x110",
            "sub x8, x29, x8",
            "str x0, [x8]",
            "ldur x0, [x29, #-16]",
            "movz x8, #0x110",
            "sub x8, x29, x8",
            "ldr x0, [x8]",
        ]
    );
}

#[test]
fn sp_store_and_globals_respect_scaled_limits() {
    let mut emitter = Emitter::new();
    emitter.sp_store("x0", 0, "");
    emitter.sp_store("x0", 32760, ""); // largest scaled offset
    emitter.sp_store("x0", 32768, ""); // via x8
    emitter.global_load("x0", 0, "");
    emitter.global_store("x1", 4095, ""); // offset 32760, still scaled
    emitter.global_load("x0", 4096, ""); // offset 32768: via x9
    assert_eq!(
        body_instructions(emitter),
        vec![
            "str x0, [sp]",
            "str x0, [sp, #32760]",
            "movz x8, #0x8000",
            "add x8, sp, x8",
            "str x0, [x8]",
            "adrp x8, g_globals",
            "add x8, x8, :lo12:g_globals",
            "ldr x0, [x8]",
            "adrp x8, g_globals",
            "add x8, x8, :lo12:g_globals",
            "str x1, [x8, #32760]",
            "adrp x8, g_globals",
            "add x8, x8, :lo12:g_globals",
            "movz x9, #0x8000",
            "ldr x0, [x8, x9]",
        ]
    );
}

#[test]
fn span_stack_maps_lines_to_source() {
    let mut emitter = Emitter::new();
    let span = Span {
        start: 5,
        end: 9,
    };
    emitter.with_span(&span, |emitter| {
        emitter.ins("mov x0, x1");
        // Synthetic code inside a spanned region carries no span.
        emitter.without_span(|emitter| emitter.ins("nop"));
        emitter.ins("mov x1, x0");
    });
    emitter.ins("ret");
    let assembly = emitter.finish(0, ".Lmain_exit", false);
    let lines: Vec<&str> = assembly.text.lines().collect();
    assert_eq!(lines.len(), assembly.line_spans.len());
    let spanned: Vec<(&str, Option<(usize, usize)>)> = lines
        .iter()
        .zip(assembly.line_spans.iter())
        .filter(|(line, _)| {
            let instruction = instruction(line);
            instruction == "mov x0, x1" || instruction == "nop" || instruction == "mov x1, x0"
        })
        .map(|(line, span)| (instruction(line), *span))
        .collect();
    assert_eq!(
        spanned,
        vec![
            ("mov x0, x1", Some((5, 9))),
            ("nop", None),
            ("mov x1, x0", Some((5, 9))),
        ]
    );
    // Prologue/epilogue lines are synthetic.
    assert_eq!(assembly.line_spans[0], None);
}

#[test]
fn intern_string_deduplicates() {
    let mut emitter = Emitter::new();
    let (label_a, len_a) = emitter.intern_string(b"hello");
    let (label_b, len_b) = emitter.intern_string(b"hello");
    let (label_c, _) = emitter.intern_string("héllo".as_bytes());
    assert_eq!(label_a, label_b);
    assert_eq!(len_a, 5);
    assert_eq!(len_b, 5);
    assert_ne!(label_a, label_c);
    let text = emitter.finish(0, ".Lmain_exit", false).text;
    assert_eq!(text.matches(&format!("{}:", label_a)).count(), 1);
    assert!(text.contains(".section .rodata"));
    // Arbitrary UTF-8 goes out as .byte lists, never quoted strings.
    assert!(text.contains(".byte 0x68, 0xc3, 0xa9, 0x6c, 0x6c, 0x6f"));
}

#[test]
fn labels_are_unique_per_kind() {
    let mut emitter = Emitter::new();
    assert_eq!(emitter.new_label(), ".L0");
    assert_eq!(emitter.new_label(), ".L1");
    assert_eq!(emitter.new_function_label(), ".Lfn0");
    assert_eq!(emitter.new_function_label(), ".Lfn1");
    assert_eq!(emitter.new_label(), ".L2");
}

#[test]
fn end_function_splices_prologue_and_epilogue() {
    let mut emitter = Emitter::new();
    emitter.begin_function();
    emitter.ins("mov x0, x1");
    emitter.end_function(FunctionFrame {
        label: ".Lfn0".to_string(),
        comment: "fn id(a)".to_string(),
        num_parameters: 1,
        num_definitions: 2,
        epilogue_label: ".Lfn0_exit".to_string(),
        parameter_names: vec!["a".to_string()],
    });
    let lines = finished_lines(emitter);
    let start = lines
        .iter()
        .position(|line| line.starts_with(".Lfn0:"))
        .unwrap();
    let function: Vec<&str> = lines[start..start + 10]
        .iter()
        .map(|line| instruction(line))
        .collect();
    assert_eq!(
        function,
        vec![
            ".Lfn0:",
            "stp x29, x30, [sp, #-16]!",
            "mov x29, sp",
            "sub sp, sp, #48",      // 16 * (1 closure + 2 definitions)
            "stur x0, [x29, #-16]", // closure slot
            "stur x1, [x29, #-32]", // parameter a
            "movz x9, #0xb",        // null-initialize the non-parameter local
            "stur x9, [x29, #-48]",
            "mov x0, x1", // body
            ".Lfn0_exit:",
        ]
    );
    assert_eq!(instruction(&lines[start + 10]), "mov sp, x29");
    assert_eq!(instruction(&lines[start + 11]), "ldp x29, x30, [sp], #16");
    assert_eq!(instruction(&lines[start + 12]), "ret");
    // Functions come after main's epilogue.
    let main_exit = lines
        .iter()
        .position(|line| line.starts_with(".Lmain_exit:"))
        .unwrap();
    assert!(main_exit < start);
}

#[test]
fn big_frames_still_reach_every_slot() {
    let mut emitter = Emitter::new();
    emitter.begin_function();
    emitter.ins("nop");
    emitter.end_function(FunctionFrame {
        label: ".Lfn0".to_string(),
        comment: "fn big()".to_string(),
        num_parameters: 0,
        num_definitions: 20, // frame 336 bytes; slots 15.. beyond stur range
        epilogue_label: ".Lfn0_exit".to_string(),
        parameter_names: vec![],
    });
    let text = finished_lines(emitter).join("\n");
    assert!(text.contains("sub sp, sp, #336"));
    // Null-init of the far slots must materialize addresses through x8.
    assert!(text.contains("sub x8, x29, x8"));
}

#[test]
fn finish_emits_fixed_exit_and_bss() {
    let assembly = Emitter::new().finish(3, ".Lmain_exit", false);
    let text = assembly.text;
    assert!(text.contains(".globl main"));
    assert!(text.contains("main:"));
    assert!(text.contains("bl rt_globals_init"));
    assert!(text.contains("mov w0, #0"));
    assert!(text.contains("g_globals:"));
    assert!(text.contains(".skip 24"));
    assert!(!text.contains("rt_observer_init"));
    // Globals init passes the array and its length.
    assert!(text.contains("adrp x0, g_globals"));
    assert!(text.contains("movz x1, #0x3"));
}

#[test]
fn finish_with_observe_wires_the_channel() {
    let text = Emitter::new().finish(0, ".Lmain_exit", true).text;
    let observer_init = text.find("bl rt_observer_init").unwrap();
    let globals_init = text.find("bl rt_globals_init").unwrap();
    let observe_result = text.find("bl rt_observe_result").unwrap();
    let exit = text.find("mov w0, #0").unwrap();
    assert!(observer_init < globals_init);
    assert!(globals_init < observe_result);
    assert!(observe_result < exit);
    // fd 3 goes to rt_observer_init.
    assert!(text.contains("movz x0, #0x3"));
}
