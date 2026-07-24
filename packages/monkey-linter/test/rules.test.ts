import { describe, expect, it } from 'vitest'

import { compact, rulesOf } from './helpers'

describe('no-unused-let', () => {
  it.each([
    'let x = 1; puts("hi");',
    'class Counter { constructor() { this.n = 0; } } puts(1);',
  ])('flags an unreferenced binding: %s', (source) => {
    expect(rulesOf(source)).toEqual(['no-unused-let'])
  })

  it.each([
    'let x = 1; puts(x);',
    // A rebinding's initializer references the previous binding, so it is used.
    'let x = 1; let x = x + 1; puts(x);',
    // A `let`-bound function may reference its own name (recursion).
    'let f = fn(n) { f(n); }; f(1);',
  ])('stays quiet when the binding is used: %s', (source) => {
    expect(rulesOf(source)).toEqual([])
  })

  it('reports the declared name span and message', () => {
    expect(compact('let unused = 1; puts(2);')).toEqual([
      "no-unused-let@4-10: 'unused' is declared but never used",
    ])
  })

  it('flags a binding only referenced by itself', () => {
    // `f` recurses, but nothing outside the declaration ever calls it — the
    // whole definition is dead, recursion and all.
    expect(compact('let f = fn(n) { f(n); };')).toEqual([
      "no-unused-let@4-5: 'f' is only referenced by itself and never used",
    ])
    expect(
      compact('class C { constructor() {} m() { new C(); } } puts(1);')
    ).toEqual([
      "no-unused-let@6-7: class 'C' is only referenced by itself and never used",
    ])
  })

  it('does not resolve an else-arm reference to a then-arm declaration', () => {
    const source = 'let x = 1; if (false) { let x = 2; } else { puts(x); }'
    const unused = compact(source).filter((diagnostic) =>
      diagnostic.startsWith('no-unused-let@')
    )

    // The declaration in the untaken arm is unused; the outer x is referenced
    // by the alternative and must not be reported.
    expect(unused).toEqual([
      "no-unused-let@28-29: 'x' is declared but never used",
    ])
  })

  it('credits a post-conditional reference to every possible binding', () => {
    const source = 'let c = true; let x = 1; if (c) { let x = 2; } puts(x);'
    expect(rulesOf(source)).not.toContain('no-unused-let')
  })
})

describe('no-unused-param', () => {
  it('flags a parameter that is never referenced', () => {
    expect(rulesOf('let f = fn(a, b) { a; }; f(1, 2);')).toEqual([
      'no-unused-param',
    ])
  })

  it.each([
    'let f = fn(a) { a; }; f(1);',
    // A leading underscore is an explicit "unused on purpose" opt-out.
    'let f = fn(_unused) { 1; }; f(1);',
    // Referenced from a nested closure still counts as used.
    'let f = fn(a) { fn() { a; }; }; f(1);',
  ])('stays quiet otherwise: %s', (source) => {
    expect(rulesOf(source)).toEqual([])
  })
})

describe('no-unused-expression', () => {
  it.each([
    ['let x = 5; x; x;', 1],
    ['1; 2; puts(3);', 2],
    // Non-tail statement inside a function body.
    ['let f = fn() { 1; 2; }; f();', 1],
    // Non-tail statement inside an if branch (branch tail is the return value).
    ['let f = fn(flag) { if (flag) { 1; flag; } else { 2; } }; f(true);', 1],
  ])('flags a discarded pure expression: %s', (source, count) => {
    expect(rulesOf(source)).toEqual(
      Array.from({ length: count }, () => 'no-unused-expression')
    )
  })

  it.each([
    // Every tail is observed: program result, function return, if-branch value.
    'let x = 1; x;',
    'puts(1); puts(2);',
    'let compute = fn() { puts(0); 40 + 2 }; compute();',
    // Calls, `new`, and index reads may have effects, so they are never flagged.
    'let store = fn() { puts(1); }; store(); puts(2);',
    // Discarded index/property reads can raise runtime errors (bad index,
    // missing property), so v0 leaves them alone.
    'let xs = [1, 2]; xs[5]; puts(xs);',
    'class P { constructor() { this.v = 1; } } let p = new P(); p.v; puts(p);',
  ])('stays quiet otherwise: %s', (source) => {
    expect(rulesOf(source)).not.toContain('no-unused-expression')
  })

  it.each([
    // Initializers and return arguments consume the conditional's value.
    'let c = true; let x = if (c) { 1; } else { 2; }; puts(x);',
    'let f = fn(c) { return if (c) { 1; } else { 2; }; }; f(true);',
    // Operators and collection literals consume their operands/elements.
    'let c = true; !(if (c) { true; } else { false; }); puts(c);',
    'let c = true; 1 + if (c) { 2; } else { 3; }; puts(c);',
    'let c = true; [if (c) { 1; } else { 2; }]; puts(c);',
    'let c = true; {"k": if (c) { 1; } else { 2; }}; puts(c);',
    // Calls/new and index/property access consume all nested values.
    'let c = true; let id = fn(x) { x; }; id(if (c) { 1; } else { 2; });',
    'let c = true; let id = fn(x) { x; }; (if (c) { id; } else { id; })(1);',
    'class Box { constructor(v) { this.v = v; } } let c = true; new Box(if (c) { 1; } else { 2; });',
    'let c = true; let xs = [1]; xs[if (c) { 0; } else { 0; }];',
    'class Box { constructor() { this.v = 1; } } let c = true; let b = new Box(); (if (c) { b; } else { b; }).v;',
    // A property assignment consumes both the receiver and assigned value.
    'class Box { constructor() { this.v = 1; } } let c = true; let b = new Box(); b.v = if (c) { 2; } else { 3; };',
  ])(
    'does not flag branch tails in a consumed value position: %s',
    (source) => {
      expect(rulesOf(source)).not.toContain('no-unused-expression')
    }
  )

  it.each(['1 / 0; puts(1);', '-"x"; puts(1);', '{[]: 1}; puts(1);'])(
    'does not suggest removing an expression that can fail: %s',
    (source) => {
      expect(rulesOf(source)).not.toContain('no-unused-expression')
    }
  )
})

describe('no-unreachable-code', () => {
  it.each([
    'let f = fn() { return 1; 2; }; f();',
    // Only the first statement after the return is reported.
    'let f = fn() { return 1; puts(2); puts(3); }; f();',
  ])('flags a statement after return: %s', (source) => {
    expect(rulesOf(source)).toEqual(['no-unreachable-code'])
  })

  it.each([
    'let f = fn() { puts(1); return 2; }; f();',
    // A return nested in a branch does not make code after the `if` unreachable.
    'let f = fn(c) { if (c) { return 1; }; puts(2); }; f(true);',
  ])('stays quiet otherwise: %s', (source) => {
    expect(rulesOf(source)).toEqual([])
  })
})

describe('no-duplicate-hash-key', () => {
  it.each(['{1: "a", 1: "b"};', '{"k": 1, "k": 2};', '{true: 1, true: 2};'])(
    'flags a repeated scalar key: %s',
    (source) => {
      expect(rulesOf(source)).toEqual(['no-duplicate-hash-key'])
    }
  )

  it.each([
    '{1: "a", 2: "b"};',
    // An integer key and a string key never collide.
    '{1: "a", "1": "b"};',
    // Non-literal keys cannot be compared statically.
    'let a = 1; {a: 1, a: 2};',
  ])('stays quiet otherwise: %s', (source) => {
    expect(rulesOf(source)).toEqual([])
  })
})

describe('builtin-arity', () => {
  it.each(['len();', 'len(1, 2);', 'len([1], [2], [3]);'])(
    'flags a wrong-arity len call: %s',
    (source) => {
      expect(rulesOf(source)).toEqual(['builtin-arity'])
    }
  )

  it.each([
    'len("hi");',
    'len([1, 2]);',
    // first/last/rest/push are not checked: the interpreter's loose arity
    // handling diverges from the VM's.
    'first([1], [2]);',
    'push([1]);',
  ])('stays quiet otherwise: %s', (source) => {
    expect(rulesOf(source)).not.toContain('builtin-arity')
  })

  it('does not fire when len is shadowed by a user binding', () => {
    expect(rulesOf('let len = fn(a) { a; }; len(1, 2);')).toEqual([
      'no-shadowed-builtin',
    ])
  })
})

describe('no-shadowed-builtin', () => {
  it.each([
    'let puts = 1; puts;',
    'let f = fn(len) { len; }; f(1);',
    'class first { constructor() {} } first;',
  ])('flags a binding that shadows a builtin: %s', (source) => {
    expect(rulesOf(source)).toEqual(['no-shadowed-builtin'])
  })

  it('stays quiet for non-builtin names', () => {
    expect(rulesOf('let x = 1; puts(x);')).toEqual([])
  })
})

describe('no-constant-condition', () => {
  it.each([
    ['if (true) { 1; };', 'truthy'],
    // Every integer is truthy in Monkey, even zero.
    ['if (0) { 1; };', 'truthy'],
    ['if ("") { 1; };', 'truthy'],
    ['if (false) { 1; } else { 2; };', 'falsy'],
  ])('flags a literal condition: %s', (source, outcome) => {
    expect(compact(source)[0]).toContain(`no-constant-condition@`)
    expect(compact(source)[0]).toContain(`always ${outcome}`)
  })

  it('stays quiet for a computed condition', () => {
    expect(rulesOf('let c = true; if (c) { 1; };')).toEqual([])
  })
})

describe('no-literal-type-mismatch', () => {
  it.each(['1 + "a";', 'true + 1;', '"a" - "b";', '1 < "a";', 'true * false;'])(
    'flags an incompatible literal operation: %s',
    (source) => {
      expect(rulesOf(source)).toEqual(['no-literal-type-mismatch'])
    }
  )

  it.each([
    '1 + 2;',
    '"a" + "b";',
    '1 < 2;',
    // Equality never errors in the interpreter, so it is never flagged.
    '1 == true;',
    '"a" != 1;',
    // A non-literal operand could hold any type.
    'let x = true; x + 1;',
  ])('stays quiet otherwise: %s', (source) => {
    expect(rulesOf(source)).toEqual([])
  })
})
