import { describe, expect, it } from 'vitest'

import { minify } from '../src'

describe('constant folding and conservative DCE', () => {
  const optimize = (source: string) =>
    minify(source, { fold: true, mangle: false }).code

  it.each([
    ['40 + 2', '42;'],
    ['9223372036854775807 + 2', '-9223372036854775807;'],
    ['"mon" + "key"', '"monkey";'],
    ['1 < 2 == true', 'true;'],
    ['!!1', 'true;'],
    ['if (true) { 1 } else { 2 }', '1;'],
  ])('folds %s', (source, expected) => {
    expect(optimize(source)).toBe(expected)
  })

  it('retains arithmetic errors and unprintable i64::MIN results', () => {
    expect(optimize('1 / 0')).toBe('1/0;')
    expect(optimize('9223372036854775807 + 1')).toBe('9223372036854775807+1;')
    expect(optimize('(-9223372036854775807 - 1) / -1')).toBe(
      '(-9223372036854775807-1)/-1;'
    )
  })

  it('only folds if branches that do not alter compiler scope', () => {
    const code = optimize('if (true) { 1 } else { let value = 2; value }')
    expect(code).toContain('if(true)')
    expect(code).toContain('let value=2')
  })

  it('does not hide diagnostics in an unselected branch', () => {
    expect(optimize('if (true) { 1 } else { missing };')).toBe(
      'if(true){1;}else{missing;};'
    )
  })

  it('does not turn an indirectly assigned function into a recursive one', () => {
    expect(
      optimize(
        'let value = 1; let value = if (true) { fn() { value } } else { fn() { 0 } }; value();'
      )
    ).toContain('if(true)')
  })

  it('deletes pure unused lets to a fixed point', () => {
    expect(optimize('let first = 1; let second = first; 42;')).toBe('42;')
    expect(optimize('let helper = fn(x) { puts(x) }; 0;')).toBe('0;')
  })

  it('keeps trailing lets that determine block value semantics', () => {
    expect(optimize('let f = fn() { 42; let unused = 1; }; f();')).toBe(
      'let f=fn(){42;let unused=1;};f();'
    )
    expect(optimize('if (true) { 42; let unused = 1; };')).toBe(
      'if(true){42;let unused=1;};'
    )
  })

  it('treats debugger as an undeletable, completion-transparent statement', () => {
    // The statement before a trailing run of debuggers decides the block's
    // value, so the trailing-let barrier must look through the suffix.
    expect(
      optimize('let f = fn() { 42; let unused = 1; debugger; }; f();')
    ).toBe('let f=fn(){42;let unused=1;debugger;};f();')
    // A debugger-only arm yields null; collapsing `if (true) { debugger; }` to
    // its body would let the preceding value leak through as the result.
    expect(optimize('1; if (true) { debugger; };')).toBe(
      '1;if(true){debugger;};'
    )
    expect(optimize('debugger; 42;')).toBe('debugger;42;')
  })

  it('keeps a stack-sensitive callable local layout intact', () => {
    const source = `
      let f = fn() {
        if (true) { 1; let first = 2; }
        else { let second = 2; 1; }
      };
      f();
    `
    expect(optimize(source)).toBe(
      'let f=fn(){if(true){1;let first=2;}else{let second=2;1;};};f();'
    )
  })

  it('retains effectful and potentially throwing initializers', () => {
    expect(optimize('let value = puts("visible");')).toBe(
      'let value=puts("visible");'
    )
    expect(optimize('let value = 1 / 0;')).toBe('let value=1/0;')
    expect(optimize('let value = 1.missing;')).toBe('let value=1.missing;')
    expect(optimize('let value = [][0];')).toBe('let value=[][0];')
  })
})

describe('constant propagation', () => {
  const optimize = (source: string) =>
    minify(source, { fold: true, mangle: false }).code

  it('inlines literal bindings through folding to a fixed point', () => {
    expect(optimize('let a = 1 + 1;\nlet b= a + 1;\nprint(a)')).toBe(
      'print(2);'
    )
    expect(minify('let a = 1 + 1;\nlet b= a + 1;\nprint(a)').code).toBe(
      'print(2);'
    )
    expect(optimize('let a = 2; let b = a; let c = b; c;')).toBe('2;')
  })

  it('propagates every literal shape', () => {
    expect(optimize('let n = 0 - 5; n;')).toBe('-5;')
    expect(optimize('let t = true; t == false;')).toBe('false;')
    expect(optimize('let s = "hello"; puts(s);')).toBe('puts("hello");')
  })

  it('keeps a binding when inlined copies would outweigh it', () => {
    expect(optimize('let s = "hello"; puts(s); puts(s); puts(s);')).toBe(
      'let s="hello";puts(s);puts(s);puts(s);'
    )
  })

  it('keeps a let that a conditional block shadows', () => {
    // Blocks are not scopes, and an arm is skippable: after the block the name
    // means the arm's `let` or the one before the branch, whichever ran.
    // Dropping the outer one would leave `puts(v)` reading an unwritten slot
    // whenever the branch is skipped, and neither initializer can be
    // propagated because the binding no longer has just one.
    expect(optimize('let v = 1; if (1 > 2) { let v = 2; }; puts(v);')).toBe(
      'let v=1;if(false){let v=2;};puts(v);'
    )
    expect(
      optimize('if (true) { let v = 1; } if (false) { let v = 2; } puts(v);')
    ).toBe('if(true){let v=1;};if(false){let v=2;};puts(v);')
    // Every `let` of the name in the arm joins that one binding, so the last
    // of them stays under the same name as the binding it converges with.
    expect(
      optimize('let v = 1; if (1 > 2) { let v = 2; let v = 3; }; puts(v);')
    ).toBe('let v=1;if(false){let v=2;let v=3;};puts(v);')
  })

  it('keeps a let that a conditional block in a nested scope shadows', () => {
    // The binding an arm shadows can sit in an enclosing scope, and a skipped
    // arm leaves the read after the block on that captured value. Mangling
    // shows the two stay one name; renaming them apart would strand the read.
    const source =
      'let v = 1; let f = fn() { if (1 > 2) { let v = 2; } v }; f();'
    expect(optimize(source)).toBe(
      'let v=1;let f=fn(){if(false){let v=2;};v;};f();'
    )
    expect(minify(source).code).toBe(
      'let a=1;let b=fn(){if(false){let a=2;};a;};b();'
    )
  })

  it('keeps the arms of a name the branch introduces under one name', () => {
    // Neither arm inherits `n`, but the read after the branch means whichever
    // arm ran, so the two `let`s are one binding — renaming them apart would
    // strand that read on the arm that did not run.
    const source = 'if (1 < 2) { let n = 2; } else { let n = 3; }; puts(n);'
    expect(optimize(source)).toBe('if(true){let n=2;}else{let n=3;};puts(n);')
    expect(minify(source).code).toBe(
      'if(true){let a=2;}else{let a=3;};puts(a);'
    )
  })

  it('leaves a program where a block shadows a builtin alone', () => {
    // `len` cannot be renamed, so the arm's `let` and the builtin cannot be
    // brought under one name; nothing in the program is touched.
    expect(optimize('if (1 > 2) { let len = 5; }; puts(len("ab"));')).toBe(
      'if(1>2){let len=5;};puts(len("ab"));'
    )
  })

  it('starts a new binding at the first let after the block', () => {
    // Once the arm closes the name is unconditional again, so this `let` takes
    // a slot of its own (compiler/symbol_table.rs) — the closure keeps the
    // value it captured, and the fresh binding has one initializer to
    // propagate.
    expect(
      optimize(
        'if (true) { let v = 1; let g = fn() { v }; }; let v = 2; g() + v;'
      )
    ).toBe('if(true){let v=1;let g=fn(){v;};};g()+2;')
    expect(
      optimize('let v = 1; if (1 > 2) { let v = 2; }; let v = 3; puts(v);')
    ).toBe('if(false){let v=2;};puts(3);')
  })

  it('leaves a conditional let alone: its slot may never be written', () => {
    expect(optimize('if (1 > 2) { let v = 2; }; puts(v);')).toBe(
      'if(false){let v=2;};puts(v);'
    )
  })

  it('keeps a redeclaration inside one block separate', () => {
    // Two `let`s at the same depth take a slot each, so the closure keeps the
    // value it captured. Mangling shows the two `v`s stay distinct bindings.
    const source =
      'if (true) { let v = 1; let g = fn() { v }; let v = 2; g() + v }'
    expect(optimize(source)).toBe(
      'if(true){let v=1;let g=fn(){v;};let v=2;g()+v;};'
    )
    expect(minify(source).code).toBe(
      'if(true){let a=1;let b=fn(){a;};let c=2;b()+c;};'
    )
  })

  it('propagates across a redeclaration: each slot is written once', () => {
    expect(optimize('let v = 1; let g = fn() { v }; let v = 2; g() + v;')).toBe(
      'let g=fn(){1;};g()+2;'
    )
  })
})
