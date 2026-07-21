/**
 * Beginner-facing documentation for every token the arm64 backend can emit
 * (see asm/emitter.rs and asm/lower.rs — the instruction set is a small,
 * closed set, so this dictionary is complete). Pure data plus lookup; the
 * CodeMirror glue lives in arm64Language.ts.
 */

export interface Arm64TokenDoc {
  /** Headline: the token plus its expansion or role. */
  title: string
  /** One-to-three sentences grounded in this backend's idioms. */
  detail: string
}

/** Every instruction mnemonic the backend emits; also drives the tokenizer. */
export const ARM64_MNEMONICS: ReadonlySet<string> = new Set([
  'add',
  'adds',
  'adrp',
  'b',
  'bl',
  'bvs',
  'cbz',
  'ldp',
  'ldr',
  'ldur',
  'mov',
  'movk',
  'movz',
  'orr',
  'ret',
  'stp',
  'str',
  'stur',
  'sub',
  'tbnz',
])

const MNEMONIC_DOCS: Record<string, Arm64TokenDoc> = {
  mov: {
    title: 'mov — copy',
    detail:
      'Copies a value into a register, from another register (mov x29, sp) or a small constant (mov w0, #0).',
  },
  movz: {
    title: 'movz — move with zero',
    detail:
      'Loads a 16-bit constant into a register and zeroes the rest. With lsl #16/32/48 the constant lands in a higher slice; this is the first step of building any constant.',
  },
  movk: {
    title: 'movk — move with keep',
    detail:
      'Overwrites one 16-bit slice of a register and keeps the other bits — follows movz to assemble constants wider than 16 bits.',
  },
  adrp: {
    title: 'adrp — address of page',
    detail:
      'Loads the 4 KB-page address of a symbol such as g_globals. The following add …, :lo12:… fills in the final 12 bits; together they materialize the full address.',
  },
  add: {
    title: 'add — addition',
    detail:
      'Adds registers or an immediate. Also finishes an adrp address (:lo12:), computes stack addresses, and releases stack space (add sp, sp, #n).',
  },
  adds: {
    title: 'adds — add, set flags',
    detail:
      'Addition that also sets the condition flags. The inline integer fast path uses it so the following bvs can detect overflow.',
  },
  sub: {
    title: 'sub — subtraction',
    detail:
      'Subtracts. sub sp, sp, #n grows the stack frame — the stack grows toward lower addresses.',
  },
  orr: {
    title: 'orr — bitwise OR',
    detail:
      'ORs two registers. The fast path merges both operands so a single tbnz can test the tag bits of both values at once.',
  },
  tbnz: {
    title: 'tbnz — test bit, branch if non-zero',
    detail:
      'Tests one bit and branches when it is 1. tbnz x8, #0, … jumps to the slow path when either operand is not an inline small integer (tag bit 0 set).',
  },
  cbz: {
    title: 'cbz — compare, branch if zero',
    detail:
      'Branches when the register is zero. After rt_truthy (which returns 0 or 1), cbz x0, … jumps to the else-branch when the condition is false.',
  },
  b: {
    title: 'b — branch',
    detail: 'Unconditional jump to a label.',
  },
  bvs: {
    title: 'bvs — branch if overflow',
    detail:
      'Branches when the previous flag-setting instruction overflowed — bails out of the inline adds fast path into the runtime call.',
  },
  bl: {
    title: 'bl — branch with link (call)',
    detail:
      'A function call: stores the return address in x30 and jumps. bl rt_* calls into the Monkey runtime — arguments in x0, x1, …; result back in x0.',
  },
  ret: {
    title: 'ret — return',
    detail: 'Returns to the address in x30, ending the function.',
  },
  stp: {
    title: 'stp — store pair',
    detail:
      'Stores two registers with one instruction. stp x29, x30, [sp, #-16]! grows the stack 16 bytes and saves the frame pointer and return address — the standard prologue.',
  },
  ldp: {
    title: 'ldp — load pair',
    detail:
      'Loads two registers back. ldp x29, x30, [sp], #16 restores the frame pointer and return address and shrinks the stack — the epilogue, mirroring stp.',
  },
  str: {
    title: 'str — store',
    detail:
      'Stores a register to memory. str x0, [sp, #-16]! first moves sp down 16 bytes, then stores — a push of the accumulator.',
  },
  ldr: {
    title: 'ldr — load',
    detail:
      'Loads from memory into a register. ldr x0, [sp], #16 loads, then moves sp up 16 bytes — a pop.',
  },
  stur: {
    title: 'stur — store (unscaled offset)',
    detail:
      'Store with a byte offset that may be negative — used for slots below the frame pointer, e.g. stur x1, [x29, #-32] writes a local variable.',
  },
  ldur: {
    title: 'ldur — load (unscaled offset)',
    detail:
      'Load with a possibly negative byte offset — reads a local variable or the hidden closure slot from below the frame pointer.',
  },
}

const SHIFT_DOC: Arm64TokenDoc = {
  title: 'lsl — logical shift left',
  detail:
    'As an operand suffix (movz …, lsl #16) it shifts the immediate into a higher position, selecting which 16-bit chunk of a constant is written.',
}

const RELOC_DOC: Arm64TokenDoc = {
  title: ':lo12: — low 12 bits of an address',
  detail:
    'ELF relocation syntax: adrp loaded the symbol’s 4 KB page; add …, :lo12:sym adds the remaining 12 bits to complete the address.',
}

const REGISTER_DOCS: Record<string, Arm64TokenDoc> = {
  x0: {
    title: 'x0 — argument 1 / return value',
    detail:
      'The workhorse: every expression leaves its result here, calls take their first argument and return their result here, and compiled Monkey functions receive their closure here.',
  },
  x1: {
    title: 'x1 — argument 2',
    detail:
      'Second call argument; binary operators put the right-hand operand here. Monkey function parameters arrive in x1 and up.',
  },
  x2: {
    title: 'x2 — argument 3',
    detail:
      'Third call argument — e.g. rt_call’s argument-array pointer, or a function’s second parameter.',
  },
  x3: {
    title: 'x3 — argument 4',
    detail: 'Fourth call argument, or a function’s third parameter.',
  },
  x4: {
    title: 'x4 — argument 5',
    detail: 'Fifth call argument, or a function’s fourth parameter.',
  },
  x8: {
    title: 'x8 — scratch',
    detail:
      'Temporary used within a single operation: fast-path arithmetic and addressing stack slots or globals that are too far away for one instruction. Never holds a value across a call.',
  },
  x9: {
    title: 'x9 — scratch',
    detail:
      'Second temporary: large offsets, and null-initializing local slots in prologues.',
  },
  x29: {
    title: 'x29 — frame pointer (fp)',
    detail:
      'Anchors the current function’s stack frame. The hidden closure sits at [x29, #-16] and every parameter and local at a fixed offset below it.',
  },
  x30: {
    title: 'x30 — link register (lr)',
    detail:
      'Holds the return address: bl writes it, ret jumps to it. The prologue’s stp saves it so nested calls do not lose it.',
  },
  sp: {
    title: 'sp — stack pointer',
    detail:
      'Top of the stack. arm64 requires sp to stay 16-byte aligned at every access, which is why every push moves it by 16.',
  },
  w0: {
    title: 'w0 — low half of x0',
    detail: 'The 32-bit view of x0. mov w0, #0 sets main’s process exit code.',
  },
}

const DIRECTIVE_DOCS: Record<string, Arm64TokenDoc> = {
  '.text': {
    title: '.text — code section',
    detail: 'Everything after this goes into the executable-code section.',
  },
  '.globl': {
    title: '.globl — export symbol',
    detail:
      'Exports a symbol from this file — the C startup code can only call main because it is global.',
  },
  '.p2align': {
    title: '.p2align — align to 2ⁿ bytes',
    detail:
      'Aligns the next address to 2^n bytes; arm64 instructions must start on 4-byte boundaries (.p2align 2).',
  },
  '.section': {
    title: '.section — switch section',
    detail:
      'Switches output to a named section — .section .rodata starts the read-only data that holds string bytes.',
  },
  '.rodata': {
    title: '.rodata — read-only data',
    detail: 'The read-only data section: string literals live here as raw bytes.',
  },
  '.byte': {
    title: '.byte — emit bytes',
    detail: 'Emits literal bytes — the UTF-8 contents of a Monkey string.',
  },
  '.bss': {
    title: '.bss — zero-initialized data',
    detail:
      'The zero-initialized data section: g_globals lives here without storing any bytes in the binary.',
  },
  '.balign': {
    title: '.balign — align to n bytes',
    detail: 'Aligns the next address to n bytes (.balign 8 for 8-byte value slots).',
  },
  '.skip': {
    title: '.skip — reserve bytes',
    detail: 'Reserves n bytes of zeros — 8 bytes per global variable slot.',
  },
}

const SYMBOL_DOCS: Record<string, Arm64TokenDoc> = {
  main: {
    title: 'main — program entry point',
    detail:
      'The top-level Monkey program compiles into the C main function; the operating system’s C runtime calls it, and its w0 return value becomes the exit code.',
  },
  g_globals: {
    title: 'g_globals — global variable slots',
    detail:
      'One 8-byte tagged value per top-level let, living in .bss. rt_globals_init registers the array with the runtime at startup so the GC can see globals.',
  },
  '.Lmain_exit': {
    title: '.Lmain_exit — end of main',
    detail:
      'main’s epilogue label: the last top-level statement falls through (or branches) here to restore the frame and exit.',
  },
}

const RUNTIME_DOCS: Record<string, Arm64TokenDoc> = {
  rt_globals_init: {
    title: 'rt_globals_init(base, count)',
    detail:
      'Startup call: null-initializes the g_globals array and registers it with the runtime so global slots are visible to the GC.',
  },
  rt_string_from_bytes: {
    title: 'rt_string_from_bytes(ptr, len) → value',
    detail:
      'Allocates a Monkey string from the UTF-8 bytes staged in .rodata (see the .Lstr labels).',
  },
  rt_box_int: {
    title: 'rt_box_int(raw) → value',
    detail:
      'Boxes an integer too large for the inline tagged form (beyond ±2⁶²) into a heap object.',
  },
  rt_array: {
    title: 'rt_array(argv, len) → value',
    detail:
      'Builds an array from len values packed on the stack at argv (filled by the str instructions just above).',
  },
  rt_hash: {
    title: 'rt_hash(argv, pairs) → value',
    detail: 'Builds a hash from key/value pairs packed on the stack at argv.',
  },
  rt_closure: {
    title: 'rt_closure(code, num_params, free, num_free) → value',
    detail:
      'Allocates a closure: a function entry point (.Lfn label) plus its captured free variables.',
  },
  rt_get_free: {
    title: 'rt_get_free(closure, index) → value',
    detail:
      'Reads captured variable index out of the current closure (kept in the frame’s hidden slot at [x29, #-16]).',
  },
  rt_class: {
    title: 'rt_class(name, len) → value',
    detail: 'Creates a class object named by the string bytes at (name, len).',
  },
  rt_class_add_method: {
    title: 'rt_class_add_method(class, name, len, method, is_ctor)',
    detail: 'Attaches a compiled method (a closure) to a class.',
  },
  rt_get_property: {
    title: 'rt_get_property(obj, name, len) → value',
    detail: 'Property read: obj.name looked up at runtime.',
  },
  rt_set_property: {
    title: 'rt_set_property(obj, name, len, value)',
    detail: 'Property write: obj.name = value.',
  },
  rt_index: {
    title: 'rt_index(obj, index) → value',
    detail: 'The [] operator on arrays, hashes, and strings.',
  },
  rt_add: {
    title: 'rt_add(left, right) → value',
    detail:
      'The + slow path: boxed integers and string concatenation, with type checking. The inline orr/tbnz/adds sequence above it handles small-int + small-int without a call.',
  },
  rt_sub: {
    title: 'rt_sub(left, right) → value',
    detail: 'The - operator, with overflow and type checking in the runtime.',
  },
  rt_mul: {
    title: 'rt_mul(left, right) → value',
    detail: 'The * operator, with overflow and type checking in the runtime.',
  },
  rt_div: {
    title: 'rt_div(left, right) → value',
    detail:
      'The / operator, with division-by-zero, overflow, and type checking in the runtime.',
  },
  rt_eq: {
    title: 'rt_eq(left, right) → true/false',
    detail: 'The == operator.',
  },
  rt_neq: {
    title: 'rt_neq(left, right) → true/false',
    detail: 'The != operator.',
  },
  rt_gt: {
    title: 'rt_gt(left, right) → true/false',
    detail: 'The > operator; a < b compiles to this call with the operands swapped.',
  },
  rt_minus: {
    title: 'rt_minus(value) → value',
    detail: 'Unary minus (-x).',
  },
  rt_bang: {
    title: 'rt_bang(value) → value',
    detail: 'Unary ! — logical not.',
  },
  rt_truthy: {
    title: 'rt_truthy(value) → 0 or 1',
    detail:
      'Condition test for if: anything but false and null counts as true. The cbz that follows takes the else-branch on 0.',
  },
  rt_call: {
    title: 'rt_call(callee, argc, argv) → value',
    detail:
      'Calls a Monkey value — closures, named functions, and builtins like len or puts. Checks that the callee is callable and the arity matches, then jumps to its compiled code.',
  },
  rt_construct: {
    title: 'rt_construct(class, argc, argv) → value',
    detail: 'new: allocates an instance of the class and runs its constructor.',
  },
  rt_observer_init: {
    title: 'rt_observer_init(fd)',
    detail:
      'Harness plumbing: opens the channel monkey-asm run reads the program result from.',
  },
  rt_observe_result: {
    title: 'rt_observe_result(value)',
    detail:
      'Reports the final program value over the observer channel as JSON (used by monkey-asm run).',
  },
}

const RUNTIME_FALLBACK: Arm64TokenDoc = {
  title: 'rt_* — Monkey runtime call',
  detail:
    'A runtime helper written in Rust (asm/runtime.rs), statically linked from libmonkey_asm.a. Arguments in x0, x1, …; result in x0.',
}

interface LabelPattern {
  pattern: RegExp
  doc: (token: string) => Arm64TokenDoc
}

const LABEL_PATTERNS: LabelPattern[] = [
  {
    pattern: /^\.Lfn\d+_ret$/,
    doc: (token) => ({
      title: `${token} — function epilogue`,
      detail:
        'The shared exit of this function: every return branches here to restore the frame and ret.',
    }),
  },
  {
    pattern: /^\.Lfn\d+$/,
    doc: (token) => ({
      title: `${token} — compiled Monkey function`,
      detail:
        'Entry point of a compiled Monkey function — the trailing comment names it. Ordinary calls dispatch through rt_call; constructors dispatch through rt_construct after instance allocation.',
    }),
  },
  {
    pattern: /^\.Lstr\d+$/,
    doc: (token) => ({
      title: `${token} — string literal`,
      detail:
        'An interned string constant in .rodata; the trailing comment previews it. rt_string_from_bytes turns these bytes into a Monkey string at runtime.',
    }),
  },
  {
    pattern: /^\.L\d+$/,
    doc: (token) => ({
      title: `${token} — local label`,
      detail:
        'A compiler-generated branch target, file-private (the .L prefix keeps it out of the symbol table). Used for if/else joins and fast/slow paths.',
    }),
  },
]

/**
 * Documentation for one assembly token, or null when there is nothing useful
 * to say (numbers, punctuation, unknown words). Lookup is context-free: the
 * mnemonic, register, runtime, and label vocabularies never overlap.
 */
export function arm64TokenDoc(token: string): Arm64TokenDoc | null {
  if (token === 'lsl') {
    return SHIFT_DOC
  }
  if (token === 'lo12') {
    return RELOC_DOC
  }
  const exact =
    MNEMONIC_DOCS[token] ??
    REGISTER_DOCS[token] ??
    DIRECTIVE_DOCS[token] ??
    SYMBOL_DOCS[token] ??
    RUNTIME_DOCS[token]
  if (exact !== undefined) {
    return exact
  }
  for (const { pattern, doc } of LABEL_PATTERNS) {
    if (pattern.test(token)) {
      return doc(token)
    }
  }
  if (/^rt_\w+$/.test(token)) {
    return RUNTIME_FALLBACK
  }
  const wide = /^x(\d|1\d|2[0-8])$/.exec(token)
  if (wide !== null) {
    return {
      title: `${token} — general-purpose register`,
      detail:
        'A 64-bit general-purpose register; x1–x7 carry call arguments in order.',
    }
  }
  const narrow = /^w(\d|1\d|2\d|30)$/.exec(token)
  if (narrow !== null) {
    return {
      title: `${token} — low half of x${narrow[1]}`,
      detail: `The 32-bit view of x${narrow[1]}.`,
    }
  }
  return null
}

const TOKEN_CHAR = /[A-Za-z0-9_.]/

/**
 * The hoverable non-comment token at `column` in `lineText`: the maximal run
 * of word characters (letters, digits, `_`, `.`) containing the position, so
 * `.L0`, `rt_add`, and `lo12` (between its colons) each come out whole.
 */
export function arm64TokenAt(
  lineText: string,
  column: number
): { from: number; to: number; text: string } | null {
  const commentStart = lineText.indexOf('//')
  if (commentStart !== -1 && column >= commentStart) {
    return null
  }

  let anchor = column
  if (anchor >= lineText.length || !TOKEN_CHAR.test(lineText[anchor])) {
    anchor -= 1
  }
  if (anchor < 0 || anchor >= lineText.length || !TOKEN_CHAR.test(lineText[anchor])) {
    return null
  }
  let from = anchor
  while (from > 0 && TOKEN_CHAR.test(lineText[from - 1])) {
    from -= 1
  }
  let to = anchor + 1
  while (to < lineText.length && TOKEN_CHAR.test(lineText[to])) {
    to += 1
  }
  return { from, to, text: lineText.slice(from, to) }
}
