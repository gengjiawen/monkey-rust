//! AArch64 text emitter (design §6): buffers, labels, span stack, and the
//! encoding-limit helpers (`load_imm64`, frame/sp/global addressing) that
//! `lower.rs` must never bypass.
//!
//! Two dialects (design §9): Linux GNU `as`/ELF (bare symbols, `.L` locals,
//! `:lo12:` relocations, `.rodata`/`.bss`) and macOS Mach-O via clang
//! (`_`-prefixed C symbols, `L` locals, `@PAGE`/`@PAGEOFF`,
//! `__TEXT,__const`/`.zerofill`). Instructions are identical on both.

use std::collections::HashMap;

use parser::lexer::token::Span;

use crate::runtime_core::NULL_VALUE;

/// Negative-offset load/store (`ldur`/`stur`) immediates cover `[-256, 255]`
/// (design §6); larger frame offsets go through address materialization.
const MAX_UNSCALED_OFFSET: u64 = 256;
/// `add`/`sub` 12-bit immediate.
const MAX_ARITH_IMM: u64 = 4095;
/// `add`/`sub` with `lsl #12` covers another 12 bits.
const MAX_ARITH_IMM_SHIFTED: u64 = 0xFF_F000;
/// Positive scaled `ldr`/`str` x-register immediate: 8-byte multiples up to
/// `4095 * 8`.
const MAX_SCALED_OFFSET: u64 = 32760;

/// Byte offset below `x29` of the hidden closure slot (design §6).
pub const CLOSURE_SLOT_OFFSET: u64 = 16;

/// Byte offset below `x29` of symbol slot `index` (design §6:
/// `[x29, #-16*(i+2)]`).
pub fn slot_offset(index: usize) -> u64 {
    16 * (index as u64 + 2)
}

/// Call-site argument area size: 8-byte packed callee + args, 16-aligned
/// (design §7.1).
pub fn call_area_size(argc: usize) -> u64 {
    let packed = 8 * (argc as u64 + 1);
    (packed + 15) & !15
}

/// 8-byte packed scratch area (array/hash/free-variable lists), 16-aligned.
pub fn scratch_area_size(len: usize) -> u64 {
    let packed = 8 * len as u64;
    (packed + 15) & !15
}

/// Assembler/object-format dialect (design §9). Everything the two supported
/// platforms disagree on — C symbol prefixes, private-label spelling, `adrp`
/// relocation syntax, and data-section directives — routes through here; the
/// instruction stream itself is identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsmDialect {
    /// Linux GNU `as` + ELF (also what the playground shows).
    LinuxElf,
    /// macOS Apple Silicon: clang assembler + Mach-O.
    MachO,
}

impl AsmDialect {
    /// Spells a C-ABI symbol the way the platform assembler expects it:
    /// Mach-O prefixes every C-visible name with `_` (`_main`, `_rt_add`),
    /// ELF uses the bare name.
    pub fn global_symbol(self, name: &str) -> String {
        match self {
            AsmDialect::LinuxElf => name.to_string(),
            AsmDialect::MachO => format!("_{}", name),
        }
    }

    /// Prefix that keeps a label out of the object's symbol table: `.L` on
    /// ELF, `L` on Mach-O.
    pub(crate) fn local_label_prefix(self) -> &'static str {
        match self {
            AsmDialect::LinuxElf => ".L",
            AsmDialect::MachO => "L",
        }
    }
}

#[derive(Clone, Debug)]
struct Line {
    text: String,
    span: Option<(usize, usize)>,
}

/// Finished assembly module plus a per-line source span map for snapshots
/// and the playground (design §6.2, §12).
#[derive(Clone, Debug)]
pub struct Assembly {
    pub text: String,
    pub line_spans: Vec<Option<(usize, usize)>>,
}

/// Everything `end_function` needs to splice prologue and epilogue around a
/// finished body (design §6.1).
pub struct FunctionFrame {
    pub label: String,
    /// Human-readable signature for the label comment, e.g. `fn add(a, b)`.
    pub comment: String,
    /// Parameters already counting a method's implicit `this`; spilled from
    /// `x1..x{n}` into symbol slots `0..n-1`.
    pub num_parameters: usize,
    /// `SymbolTable::num_definitions` of the finished scope.
    pub num_definitions: usize,
    /// Label the body's return paths branch to; the epilogue lands here.
    pub epilogue_label: String,
    /// Comments for the spilled parameter slots (name per parameter).
    pub parameter_names: Vec<String>,
}

pub struct Emitter {
    dialect: AsmDialect,
    main_body: Vec<Line>,
    functions: Vec<Vec<Line>>,
    rodata: Vec<Line>,
    /// Stack of in-progress function bodies; instructions go to the top one,
    /// or to `main_body` when empty (design §6.1).
    open_functions: Vec<Vec<Line>>,
    /// `with_span` pushes `Some`, `without_span` pushes `None` for synthetic
    /// prologue/epilogue code (design §6.2).
    span_stack: Vec<Option<(usize, usize)>>,
    label_count: usize,
    function_count: usize,
    strings: HashMap<Vec<u8>, (String, u64)>,
}

impl Emitter {
    pub fn new(dialect: AsmDialect) -> Emitter {
        Emitter {
            dialect,
            main_body: vec![],
            functions: vec![],
            rodata: vec![],
            open_functions: vec![],
            span_stack: vec![],
            label_count: 0,
            function_count: 0,
            strings: HashMap::new(),
        }
    }

    fn current_span(&self) -> Option<(usize, usize)> {
        self.span_stack.last().copied().flatten()
    }

    fn buffer(&mut self) -> &mut Vec<Line> {
        self.open_functions
            .last_mut()
            .unwrap_or(&mut self.main_body)
    }

    fn push_line(&mut self, text: String) {
        let span = self.current_span();
        self.buffer().push(Line {
            text,
            span,
        });
    }

    pub fn with_span<R>(&mut self, span: &Span, f: impl FnOnce(&mut Emitter) -> R) -> R {
        self.span_stack.push(Some((span.start, span.end)));
        let result = f(self);
        self.span_stack.pop();
        result
    }

    pub fn without_span<R>(&mut self, f: impl FnOnce(&mut Emitter) -> R) -> R {
        self.span_stack.push(None);
        let result = f(self);
        self.span_stack.pop();
        result
    }

    /// One instruction or directive line.
    pub fn ins(&mut self, text: &str) {
        self.push_line(format!("    {}", text));
    }

    /// Instruction with a trailing `//` comment (source snippets, slot names).
    pub fn ins_cmt(&mut self, text: &str, comment: &str) {
        self.push_line(format!("    {:<31} // {}", text, comment));
    }

    /// Standalone comment line.
    pub fn comment(&mut self, comment: &str) {
        self.push_line(format!("    // {}", comment));
    }

    pub fn label(&mut self, name: &str) {
        self.push_line(format!("{}:", name));
    }

    pub fn label_cmt(&mut self, name: &str, comment: &str) {
        self.push_line(format!("{:<35} // {}", format!("{}:", name), comment));
    }

    pub fn new_label(&mut self) -> String {
        let label = format!("{}{}", self.dialect.local_label_prefix(), self.label_count);
        self.label_count += 1;
        label
    }

    pub fn new_function_label(&mut self) -> String {
        let label = format!("{}fn{}", self.dialect.local_label_prefix(), self.function_count);
        self.function_count += 1;
        label
    }

    /// Interns a string literal into `.rodata`, deduplicated; returns
    /// `(label, byte length)`.
    pub fn intern_string(&mut self, bytes: &[u8]) -> (String, u64) {
        if let Some((label, len)) = self.strings.get(bytes) {
            return (label.clone(), *len);
        }
        let label = format!("{}str{}", self.dialect.local_label_prefix(), self.strings.len());
        let len = bytes.len() as u64;
        let preview: String = String::from_utf8_lossy(bytes)
            .chars()
            .take(32)
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        self.rodata.push(Line {
            text: format!("{:<35} // \"{}\"", format!("{}:", label), preview),
            span: None,
        });
        // `.byte` lists sidestep GNU as string escaping for arbitrary UTF-8.
        for chunk in bytes.chunks(16) {
            let rendered: Vec<String> = chunk.iter().map(|b| format!("0x{:02x}", b)).collect();
            self.rodata.push(Line {
                text: format!("    .byte {}", rendered.join(", ")),
                span: None,
            });
        }
        self.strings.insert(bytes.to_vec(), (label.clone(), len));
        (label, len)
    }

    /// Materializes any 64-bit constant: one `movz` plus up to three `movk`,
    /// skipping zero halfwords (design §6). Never assumes `mov reg, #imm`
    /// encodes.
    pub fn load_imm64(&mut self, reg: &str, value: u64, comment: &str) {
        let halfwords: Vec<u64> = (0..4).map(|i| (value >> (16 * i)) & 0xFFFF).collect();
        let mut emitted = false;
        for (index, halfword) in halfwords.iter().enumerate() {
            if *halfword == 0 {
                continue;
            }
            let shift = 16 * index;
            let text = if !emitted {
                if shift == 0 {
                    format!("movz {}, #0x{:x}", reg, halfword)
                } else {
                    format!("movz {}, #0x{:x}, lsl #{}", reg, halfword, shift)
                }
            } else if shift == 0 {
                format!("movk {}, #0x{:x}", reg, halfword)
            } else {
                format!("movk {}, #0x{:x}, lsl #{}", reg, halfword, shift)
            };
            if !emitted && !comment.is_empty() {
                self.ins_cmt(&text, comment);
            } else {
                self.ins(&text);
            }
            emitted = true;
        }
        if !emitted {
            let text = format!("movz {}, #0", reg);
            if comment.is_empty() {
                self.ins(&text);
            } else {
                self.ins_cmt(&text, comment);
            }
        }
    }

    /// `reg = address of label` via `adrp` plus the low-12-bits add, spelled
    /// `:lo12:` on ELF and `@PAGE`/`@PAGEOFF` on Mach-O (design §6, §9).
    pub fn load_label_address(&mut self, reg: &str, label: &str, comment: &str) {
        let (page, low) = match self.dialect {
            AsmDialect::LinuxElf => (
                format!("adrp {}, {}", reg, label),
                format!("add {}, {}, :lo12:{}", reg, reg, label),
            ),
            AsmDialect::MachO => (
                format!("adrp {}, {}@PAGE", reg, label),
                format!("add {}, {}, {}@PAGEOFF", reg, reg, label),
            ),
        };
        if comment.is_empty() {
            self.ins(&page);
        } else {
            self.ins_cmt(&page, comment);
        }
        self.ins(&low);
    }

    /// `bl` to an `rt_*` entry point by its C name; the dialect supplies the
    /// Mach-O `_` prefix (`bl rt_add` vs `bl _rt_add`). Every runtime call
    /// must go through here so no `bl` hardcodes a platform spelling.
    pub fn call_runtime(&mut self, name: &str, comment: &str) {
        let text = format!("bl {}", self.dialect.global_symbol(name));
        if comment.is_empty() {
            self.ins(&text);
        } else {
            self.ins_cmt(&text, comment);
        }
    }

    /// Pushes the accumulator, one value per 16-byte slot (design §6).
    pub fn push_acc(&mut self, comment: &str) {
        if comment.is_empty() {
            self.ins("str x0, [sp, #-16]!");
        } else {
            self.ins_cmt("str x0, [sp, #-16]!", comment);
        }
    }

    pub fn pop(&mut self, reg: &str, comment: &str) {
        let text = format!("ldr {}, [sp], #16", reg);
        if comment.is_empty() {
            self.ins(&text);
        } else {
            self.ins_cmt(&text, comment);
        }
    }

    fn adjust_sp(&mut self, mnemonic: &str, bytes: u64) {
        if bytes == 0 {
            return;
        }
        if bytes <= MAX_ARITH_IMM {
            self.ins(&format!("{} sp, sp, #{}", mnemonic, bytes));
            return;
        }
        if bytes <= MAX_ARITH_IMM + MAX_ARITH_IMM_SHIFTED {
            let high = bytes >> 12;
            let low = bytes & 0xFFF;
            self.ins(&format!("{} sp, sp, #{}, lsl #12", mnemonic, high));
            if low != 0 {
                self.ins(&format!("{} sp, sp, #{}", mnemonic, low));
            }
            return;
        }
        self.load_imm64("x8", bytes, "sp adjustment beyond immediate range");
        self.ins(&format!("{} sp, sp, x8", mnemonic));
    }

    /// Grows the stack by `bytes` (16-aligned by contract), splitting or
    /// materializing when the immediate does not encode (design §6).
    pub fn sp_sub(&mut self, bytes: u64) {
        debug_assert_eq!(bytes % 16, 0);
        self.adjust_sp("sub", bytes);
    }

    pub fn sp_add(&mut self, bytes: u64) {
        debug_assert_eq!(bytes % 16, 0);
        self.adjust_sp("add", bytes);
    }

    /// Stores `reg` at `[x29, #-offset]`, going through `x8` when the
    /// unscaled immediate cannot encode the slot (design §6). `reg` must not
    /// be `x8`.
    pub fn frame_store(&mut self, reg: &str, offset: u64, comment: &str) {
        debug_assert!(reg != "x8");
        if offset <= MAX_UNSCALED_OFFSET {
            let text = format!("stur {}, [x29, #-{}]", reg, offset);
            if comment.is_empty() {
                self.ins(&text);
            } else {
                self.ins_cmt(&text, comment);
            }
            return;
        }
        self.load_imm64("x8", offset, comment);
        self.ins("sub x8, x29, x8");
        self.ins(&format!("str {}, [x8]", reg));
    }

    pub fn frame_load(&mut self, reg: &str, offset: u64, comment: &str) {
        debug_assert!(reg != "x8");
        if offset <= MAX_UNSCALED_OFFSET {
            let text = format!("ldur {}, [x29, #-{}]", reg, offset);
            if comment.is_empty() {
                self.ins(&text);
            } else {
                self.ins_cmt(&text, comment);
            }
            return;
        }
        self.load_imm64("x8", offset, comment);
        self.ins("sub x8, x29, x8");
        self.ins(&format!("ldr {}, [x8]", reg));
    }

    /// Stores `reg` at `[sp, #offset]` (call/scratch area fill); offsets
    /// beyond the scaled immediate go through `x8`. `reg` must not be `x8`.
    pub fn sp_store(&mut self, reg: &str, offset: u64, comment: &str) {
        debug_assert!(reg != "x8");
        debug_assert_eq!(offset % 8, 0);
        if offset <= MAX_SCALED_OFFSET {
            let text = if offset == 0 {
                format!("str {}, [sp]", reg)
            } else {
                format!("str {}, [sp, #{}]", reg, offset)
            };
            if comment.is_empty() {
                self.ins(&text);
            } else {
                self.ins_cmt(&text, comment);
            }
            return;
        }
        self.load_imm64("x8", offset, comment);
        self.ins("add x8, sp, x8");
        self.ins(&format!("str {}, [x8]", reg));
    }

    /// `reg = sp + offset` (argv base and friends).
    pub fn sp_address(&mut self, reg: &str, offset: u64, comment: &str) {
        if offset <= MAX_ARITH_IMM {
            let text = format!("add {}, sp, #{}", reg, offset);
            if comment.is_empty() {
                self.ins(&text);
            } else {
                self.ins_cmt(&text, comment);
            }
            return;
        }
        self.load_imm64(reg, offset, comment);
        self.ins(&format!("add {}, sp, {}", reg, reg));
    }

    fn global_slot(&mut self, mnemonic: &str, reg: &str, index: usize, comment: &str) {
        debug_assert!(reg != "x8" && reg != "x9");
        self.load_label_address("x8", "g_globals", comment);
        let offset = 8 * index as u64;
        if offset <= MAX_SCALED_OFFSET {
            if offset == 0 {
                self.ins(&format!("{} {}, [x8]", mnemonic, reg));
            } else {
                self.ins(&format!("{} {}, [x8, #{}]", mnemonic, reg, offset));
            }
            return;
        }
        self.load_imm64("x9", offset, "global slot beyond immediate range");
        self.ins(&format!("{} {}, [x8, x9]", mnemonic, reg));
    }

    /// Reads global slot `index` from the single `g_globals` array
    /// (design §5.2); clobbers `x8`/`x9`.
    pub fn global_load(&mut self, reg: &str, index: usize, comment: &str) {
        self.global_slot("ldr", reg, index, comment);
    }

    pub fn global_store(&mut self, reg: &str, index: usize, comment: &str) {
        self.global_slot("str", reg, index, comment);
    }

    /// Runs `f` collecting lines into a detached buffer (prologue/epilogue
    /// splicing).
    fn capture(&mut self, f: impl FnOnce(&mut Emitter)) -> Vec<Line> {
        self.open_functions.push(vec![]);
        self.without_span(f);
        self.open_functions.pop().unwrap()
    }

    /// Opens a fresh function body buffer; instructions emitted until the
    /// matching `end_function` go to it (design §6.1).
    pub fn begin_function(&mut self) {
        self.open_functions.push(vec![]);
    }

    /// Closes the current function body: computes the frame from the final
    /// symbol count, splices prologue (fp/lr save, frame, spills, null
    /// initialization) and the shared epilogue, and appends the finished
    /// function (design §6, §6.1).
    pub fn end_function(&mut self, frame: FunctionFrame) {
        let body = self
            .open_functions
            .pop()
            .expect("end_function without begin_function");
        let num_slots = 1 + frame.num_definitions;
        let frame_bytes = 16 * num_slots as u64;

        let prologue = self.capture(|emitter| {
            emitter.ins("stp x29, x30, [sp, #-16]!");
            emitter.ins("mov x29, sp");
            emitter.sp_sub(frame_bytes);
            emitter.frame_store("x0", CLOSURE_SLOT_OFFSET, "closure (hidden argument)");
            for index in 0..frame.num_parameters {
                let name = frame
                    .parameter_names
                    .get(index)
                    .map(String::as_str)
                    .unwrap_or("parameter");
                let register = format!("x{}", index + 1);
                emitter.frame_store(&register, slot_offset(index), name);
            }
            if frame.num_parameters < frame.num_definitions {
                emitter.load_imm64("x9", NULL_VALUE, "null-initialize locals");
                for index in frame.num_parameters..frame.num_definitions {
                    emitter.frame_store("x9", slot_offset(index), "");
                }
            }
        });
        let epilogue = self.capture(|emitter| {
            emitter.label(&frame.epilogue_label);
            emitter.ins("mov sp, x29");
            emitter.ins("ldp x29, x30, [sp], #16");
            emitter.ins("ret");
        });

        let mut lines = Vec::with_capacity(prologue.len() + body.len() + epilogue.len() + 1);
        lines.push(Line {
            text: format!("{:<35} // {}", format!("{}:", frame.label), frame.comment),
            span: None,
        });
        lines.extend(prologue);
        lines.extend(body);
        lines.extend(epilogue);
        self.functions.push(lines);
    }

    /// Assembles the final module: `main` (with observer/globals init and the
    /// fixed `mov w0, #0` exit), then every finished function, `.rodata`, and
    /// the `g_globals` `.bss` array (design §6, §6.1).
    pub fn finish(
        mut self,
        globals_count: usize,
        main_epilogue_label: &str,
        observe: bool,
    ) -> Assembly {
        debug_assert!(self.open_functions.is_empty(), "unfinished function buffer");
        let main_body = std::mem::replace(&mut self.main_body, vec![]);

        let prologue = self.capture(|emitter| {
            emitter.ins("stp x29, x30, [sp, #-16]!");
            emitter.ins("mov x29, sp");
            if observe {
                emitter.load_imm64("x0", 3, "observer channel fd");
                emitter.call_runtime("rt_observer_init", "");
            }
            emitter.load_label_address("x0", "g_globals", "");
            emitter.load_imm64("x1", globals_count as u64, "global slot count");
            emitter.call_runtime("rt_globals_init", "");
        });
        let epilogue = self.capture(|emitter| {
            emitter.label(main_epilogue_label);
            if observe {
                emitter.call_runtime("rt_observe_result", "program result record");
            }
            emitter.ins_cmt("mov w0, #0", "exit code is never the tagged value");
            emitter.ins("mov sp, x29");
            emitter.ins("ldp x29, x30, [sp], #16");
            emitter.ins("ret");
        });

        let mut lines: Vec<Line> = vec![];
        let mut raw = |text: &str| {
            lines.push(Line {
                text: text.to_string(),
                span: None,
            });
        };
        let main_symbol = self.dialect.global_symbol("main");
        raw("// Generated by monkey-asm (docs/arm64-asm-backend-design.md). Do not edit.");
        raw("    .text");
        raw(&format!("    .globl {}", main_symbol));
        raw("    .p2align 2");
        raw(&format!("{}:", main_symbol));
        lines.extend(prologue);
        lines.extend(main_body);
        lines.extend(epilogue);
        for function in std::mem::replace(&mut self.functions, vec![]) {
            lines.push(Line {
                text: String::new(),
                span: None,
            });
            lines.extend(function);
        }
        if !self.rodata.is_empty() {
            lines.push(Line {
                text: String::new(),
                span: None,
            });
            let rodata_section = match self.dialect {
                AsmDialect::LinuxElf => "    .section .rodata",
                AsmDialect::MachO => "    .section __TEXT,__const",
            };
            lines.push(Line {
                text: rodata_section.to_string(),
                span: None,
            });
            lines.append(&mut self.rodata);
        }
        lines.push(Line {
            text: String::new(),
            span: None,
        });
        match self.dialect {
            AsmDialect::LinuxElf => {
                lines.push(Line {
                    text: "    .bss".to_string(),
                    span: None,
                });
                lines.push(Line {
                    text: "    .balign 8".to_string(),
                    span: None,
                });
                lines.push(Line {
                    text: format!("{:<35} // {} global slot(s)", "g_globals:", globals_count),
                    span: None,
                });
                lines.push(Line {
                    text: format!("    .skip {}", 8 * globals_count),
                    span: None,
                });
            }
            AsmDialect::MachO => {
                // One directive declares section, symbol, size, and log2
                // alignment; a program without globals still reserves one
                // slot rather than betting on zero-size `.zerofill` symbols.
                let size = (8 * globals_count).max(8);
                lines.push(Line {
                    text: format!(
                        "    {:<31} // {} global slot(s)",
                        format!(".zerofill __DATA,__bss,g_globals,{},3", size),
                        globals_count
                    ),
                    span: None,
                });
            }
        }

        let mut text = String::new();
        let mut line_spans = Vec::with_capacity(lines.len());
        for line in &lines {
            text.push_str(&line.text);
            text.push('\n');
            line_spans.push(line.span);
        }
        Assembly {
            text,
            line_spans,
        }
    }
}
