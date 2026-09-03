import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import {
  analyze_lossless,
  type FunctionCall,
  type Identifier,
  type Program,
} from '@gengjiawen/monkey-wasm'
import { describe, expect, it } from 'vitest'

import { check, checkProgram } from '../src/node'
import type { TypeDiagnostic } from '../src/diagnostics'

function codes(source: string): string[] {
  return check(source).diagnostics.map((diagnostic) => diagnostic.code)
}

function only(source: string): TypeDiagnostic {
  const { diagnostics } = check(source)
  expect(diagnostics).toHaveLength(1)
  return diagnostics[0]!
}

/** The source text a diagnostic points at — spans are UTF-8 byte offsets. */
function slice(source: string, diagnostic: TypeDiagnostic): string {
  const bytes = Buffer.from(source, 'utf8')
  const { start, end } = diagnostic.span!
  return bytes.subarray(start, end).toString('utf8')
}

function clean(source: string): void {
  expect(check(source).diagnostics).toEqual([])
}

describe('analysis failures', () => {
  it('reports a parse error instead of checking a broken tree', () => {
    const { diagnostics } = check('let x: = 5;')
    expect(diagnostics).toHaveLength(1)
    expect(diagnostics[0]!.code).toBe('parse-error')
  })

  it('reports a validation error', () => {
    expect(codes('nope;')).toEqual(['validation-error'])
  })
})

// --- 7.3 inference and join --------------------------------------------------

describe('literals and join', () => {
  it('accepts matching annotations', () => {
    clean('let a: int = 1; let b: string = "s"; let c: bool = true;')
    clean('let xs: [int] = [1, 2]; let h: {string: int} = {"a": 1};')
  })

  it('rejects a mismatched annotation', () => {
    expect(only('let a: int = "s";').message).toBe(
      "type 'string' is not assignable to type 'int'"
    )
  })

  it('joins array elements into a union', () => {
    clean('let xs: [any] = [1, "a"];')
    expect(only('let xs: [int] = [1, "a"];').message).toBe(
      "type '[int | string]' is not assignable to type '[int]'"
    )
  })

  it('gives an empty array an any element', () => {
    clean('let xs: [int] = [];')
    clean('let h: {string: int} = {};')
  })

  it('joins hash keys and values separately', () => {
    // Union members keep the order they were first seen, so the value type of
    // this literal reads `string | int`, not the other way round.
    expect(
      only('let h: {string: int} = {"name": "Anna", "age": 24};').message
    ).toBe(
      "type '{string: string | int}' is not assignable to type '{string: int}'"
    )
  })

  it('binds a let to its annotation, not to the initializer type', () => {
    // `x` is `any`, so the mismatched use below is exempt.
    clean('let x: any = 1; x + "s";')
  })

  it('lets a re-let shadow the earlier type', () => {
    clean('let a: int = 1; let a: string = "s"; a + "t";')
  })

  it('joins the arms of an if expression', () => {
    expect(
      only('let c = true; let x: int = if (c) { 1 } else { "a" };').message
    ).toBe("type 'int | string' is not assignable to type 'int'")
  })

  it('joins a missing else arm with null', () => {
    clean('let c = true; let x: int? = if (c) { 1 };')
    // Null-stripping means `int?` still satisfies `int`; the nullability shows
    // up in the diagnostic for a genuinely wrong annotation.
    clean('let c = true; let x: int = if (c) { 1 };')
    expect(only('let c = true; let x: string = if (c) { 1 };').message).toBe(
      "type 'int?' is not assignable to type 'string'"
    )
  })

  it('reports the span of the offending initializer', () => {
    const source = 'let a: int = "nope";'
    expect(slice(source, only(source))).toBe('"nope"')
  })
})

// --- 7.2 assignability -------------------------------------------------------

describe('assignability', () => {
  it('is covariant in array and hash positions', () => {
    clean('let xs: [any] = [1, 2];')
    clean('let h: {string: any} = {"a": 1};')
    expect(codes('let xs: [int] = ["a"];')).toEqual(['type-mismatch'])
  })

  it('is contravariant in function parameters and covariant in returns', () => {
    clean('let f: fn(int): any = fn(x: int): int { 1 };')
    expect(codes('let f: fn(int): int = fn(x: string): int { 1 };')).toEqual([
      'type-mismatch',
    ])
    expect(codes('let f: fn(int): int = fn(x: int): string { "s" };')).toEqual([
      'type-mismatch',
    ])
  })

  it('requires equal function arity', () => {
    expect(
      codes('let f: fn(int): int = fn(a: int, b: int): int { a };')
    ).toEqual(['type-mismatch'])
  })

  it('accepts a member of the target union', () => {
    clean('let a: int? = 1;')
    clean('let a: int? = if (true) { 1 };')
  })

  it('strips null from the source union', () => {
    clean('let xs: [int] = [1]; let a: int = xs[0];')
  })
})

// --- 7.4 operators -----------------------------------------------------------

describe('operators', () => {
  it('accepts int and string addition', () => {
    clean('1 + 2; "a" + "b";')
  })

  it('rejects mixed addition', () => {
    expect(only('1 + "a";').message).toBe(
      "operator '+' expects 'int + int' or 'string + string', got 'int + string'"
    )
  })

  it('restricts the other arithmetic operators to int', () => {
    expect(codes('"a" - "b"; "a" * 2; 1 / "b";')).toEqual([
      'operator-type',
      'operator-type',
      'operator-type',
    ])
    clean('1 - 2; 3 * 4; 8 / 2;')
  })

  it('restricts comparison to int and yields bool', () => {
    clean('let b: bool = 1 < 2;')
    expect(codes('"a" < "b";')).toEqual(['operator-type'])
  })

  it('types the prefix operators', () => {
    clean('let a: int = -1; let b: bool = !1; let c: bool = !"s";')
    expect(codes('-"s";')).toEqual(['operator-type'])
  })

  it('exempts any and picks the overload from the other side', () => {
    clean('let x: any = 0; let a: int = x + 1; let b: string = x + "s";')
    clean('let x: any = 0; let c: any = x + true; let d: any = x + [1];')
    clean('let x: any = 0; let e: int = x - 1; let f: bool = x < 1;')
  })

  it('applies the operator to every union member', () => {
    expect(
      codes('let c = true; let y = if (c) { 1 } else { "s" }; y + 1;')
    ).toEqual(['operator-type'])
    // `int | string` has no user syntax, so the result of `any + (int | string)`
    // is probed by feeding it back through an operator only one member accepts.
    const mixed =
      'let c = true; let x: any = 0; let y = if (c) { 1 } else { "s" };'
    clean(`${mixed} x + y;`)
    expect(codes(`${mixed} let z = x + y; z + 1;`)).toEqual(['operator-type'])
  })

  it('collapses to any when one member result is any', () => {
    clean(
      'let c = true; let y = if (c) { 1 } else { true }; let x: any = 0; let z: string = x + y;'
    )
  })

  it('strips null before an operator check', () => {
    clean('let xs: [int] = []; first(xs) + 1;')
  })
})

// --- 7.4 equality ------------------------------------------------------------

describe('equality', () => {
  it('accepts operands of the same category', () => {
    clean('1 == 2; "a" != "b"; true == false;')
  })

  it('warns that a mixed comparison has a known answer', () => {
    const mismatch = only('1 == "a";')
    expect(mismatch.severity).toBe('warning')
    expect(mismatch.message).toBe("comparing 'int' with 'string' is always false")
    expect(only('1 != "a";').message).toBe(
      "comparing 'int' with 'string' is always true"
    )
  })

  it('allows arrays, hashes and functions', () => {
    // Every backend compares these: arrays and hashes structurally, functions
    // by identity (gc/backend_parity_test.rs).
    clean('let xs: [int] = [1]; xs == xs;')
    clean('let f: fn(): int = fn(): int { 1 }; f == f;')
    clean('let h: {string: int} = {}; h == h;')
  })

  it('allows instances of different classes', () => {
    clean('class A {} class B {} new A() == new B();')
  })

  it('exempts any on either side', () => {
    clean('let x: any = 0; x == "a"; x == 1;')
    clean('let x: any = 0; x == [1];')
  })

  it('strips null first', () => {
    clean('let xs: [int] = [1]; first(xs) == 1;')
  })

  it('lets a union through when one member can match', () => {
    // `int | string` against `int`: the comparison is exactly how a program
    // finds out which member it holds, so its answer is not known here.
    const union = 'let c = true; let y = if (c) { 1 } else { "s" };'
    clean(`${union} y == 1;`)
    clean(`${union} y == "s";`)
    clean(`${union} 1 == y;`)
    // No member of either side can match, so the answer is known again.
    expect(only(`${union} y == true;`).message).toBe(
      "comparing 'int | string' with 'bool' is always false"
    )
  })

  it('lets two nullables through — both can be null', () => {
    const optionals =
      'let xs: [int] = [1]; let ss: [string] = ["a"]; let x = first(xs); let s = first(ss);'
    clean(`${optionals} x == s;`)
    // Only one side can be null, so `null == null` is not reachable and no
    // non-null member matches either.
    expect(only(`${optionals} x == "a";`).message).toBe(
      "comparing 'int?' with 'string' is always false"
    )
  })
})

// --- 7.4/7.5 indexing --------------------------------------------------------

describe('indexing', () => {
  it('yields an optional element type', () => {
    clean('let xs: [int] = [1]; let a: int? = xs[0];')
    clean('let h: {string: int} = {}; let a: int? = h["k"];')
  })

  it('rejects a non-int array subscript', () => {
    expect(only('let xs: [int] = [1]; xs["a"];').message).toBe(
      "type 'string' cannot index '[int]'"
    )
  })

  it('rejects a hash subscript that cannot match the key type', () => {
    expect(codes('let h: {string: int} = {}; h[1];')).toEqual(['invalid-index'])
  })

  it('rejects an unindexable target', () => {
    expect(only('let s: string = "a"; s[0];').message).toBe(
      "type 'string' is not indexable"
    )
  })

  it('exempts any on both sides', () => {
    clean('let x: any = 0; x[0]; x["k"]; let xs: [int] = [1]; xs[x];')
  })

  it('indexes every member of a union', () => {
    const branches = 'let c = true; let xs = if (c) { [1] } else { ["a"] };'
    clean(`${branches} xs[0];`)
    // The element type is `int | string`: usable as `any`, but not as `int`.
    clean(`${branches} let a: any = xs[0];`)
    expect(codes(`${branches} xs[0] + 1;`)).toEqual(['operator-type'])
  })
})

// --- 7.8 hash keys -----------------------------------------------------------

describe('hash keys', () => {
  it('rejects an unhashable literal key', () => {
    expect(only('let xs: [int] = [1]; {xs: 1};').message).toBe(
      "type '[int]' cannot be used as a hash key"
    )
  })

  it('rejects an unhashable annotation key', () => {
    expect(codes('let h: {[int]: string} = {};')).toEqual(['invalid-hash-key'])
  })

  it('rejects an instance key', () => {
    expect(codes('class A {} let h = {new A(): 1};')).toEqual([
      'invalid-hash-key',
    ])
  })
})

// --- 7.6 builtins ------------------------------------------------------------

describe('builtins', () => {
  it('types len over strings and arrays', () => {
    clean('let a: int = len("abc"); let b: int = len([1, 2]);')
    expect(codes('len(1);')).toEqual(['type-mismatch'])
  })

  it('checks builtin arity, exempting the variadic ones', () => {
    expect(only('len();').message).toBe('len expects 1 argument, got 0')
    clean('puts(); puts(1); puts(1, "a", [2]);')
    clean('print(1, 2);')
  })

  it('makes first, last and rest nullable', () => {
    clean(
      'let xs: [int] = [1]; let a: int? = first(xs); let b: int? = last(xs);'
    )
    clean('let xs: [int] = [1]; let c: [int]? = rest(xs);')
    expect(codes('let xs: [int] = [1]; let a: string = first(xs);')).toEqual([
      'type-mismatch',
    ])
    expect(codes('let xs: [int] = [1]; let c: [string]? = rest(xs);')).toEqual([
      'type-mismatch',
    ])
  })

  it('instantiates push from the join of its constraints', () => {
    clean('let xs: [any] = push([1], "a");')
    expect(codes('let ys: [int] = push([1], "a");')).toEqual(['type-mismatch'])
    expect(codes('push(1, "a");')).toEqual(['type-mismatch'])
  })

  it('degrades a type variable constrained by any', () => {
    clean('let x: any = 0; let a: any = first(x); let b: [any] = push(x, 1);')
    // `[any]`, not `[int]`: an `any` argument hides the shape the variable
    // would otherwise be read off, so the second argument cannot pin it down.
    clean('let x: any = 0; push(x, 1)[0] + "s";')
    expect(codes('first([1]) + "s";')).toEqual(['operator-type'])
  })

  it('lets a user binding shadow a builtin', () => {
    clean('let len = fn(a: int, b: int): int { a + b }; len(1, 2);')
    expect(codes('let len = fn(a: int): int { a }; len("s");')).toEqual([
      'type-mismatch',
    ])
  })

  it('does not mistake an Object.prototype method for a builtin', () => {
    // Validation rejects `toString(1)` as an undefined variable, so only a
    // tree handed straight to `checkProgram` can reach the signature lookup
    // with such a name. It used to find `Object.prototype.toString` and throw.
    const analyzed = JSON.parse(analyze_lossless('len(1);')) as {
      status: 'ok'
      program: Program
    }
    expect(analyzed.status).toBe('ok')
    const call = analyzed.program.body[0] as FunctionCall
    ;(call.callee as Identifier).name = 'toString'
    expect(checkProgram(analyzed.program)).toEqual([])
  })
})

// --- 7.7 functions, calls, recursion -----------------------------------------

describe('functions', () => {
  it('checks arity and argument types', () => {
    clean('let f = fn(a: int, b: string): int { a }; f(1, "s");')
    expect(only('let f = fn(a: int): int { a }; f(1, 2);').message).toBe(
      'expected 1 argument, got 2'
    )
    expect(codes('let f = fn(a: int): int { a }; f("s");')).toEqual([
      'type-mismatch',
    ])
  })

  it('rejects a non-callable callee', () => {
    expect(only('let a: int = 1; a();').message).toBe(
      "type 'int' is not callable"
    )
  })

  it('exempts an any callee', () => {
    clean('let x: any = 0; x(1, 2, 3);')
  })

  it('requires a union callee to accept the argument on every member', () => {
    const source =
      'let c = true; let f = if (c) { fn(x: int): int { x } } else { fn(x: string): string { x } }; f(1);'
    expect(codes(source)).toEqual(['type-mismatch'])
  })

  it('infers a return type from the body', () => {
    clean('let f = fn(a: int) { a + 1 }; let b: int = f(1);')
    expect(
      codes('let f = fn(a: int) { a + 1 }; let b: string = f(1);')
    ).toEqual(['type-mismatch'])
  })

  it('checks a return against the annotation', () => {
    clean('let f = fn(a: int): int { return a; };')
    expect(codes('let f = fn(a: int): int { return "s"; };')).toEqual([
      'type-mismatch',
    ])
  })

  it('checks a fallthrough tail against the annotation', () => {
    expect(codes('let f = fn(): int { "s" };')).toEqual(['type-mismatch'])
  })

  it('gives an empty body a null return type', () => {
    clean('let f = fn(): null { };')
    expect(codes('let f = fn(): int { };')).toEqual(['type-mismatch'])
  })

  it('points an empty body at its braces', () => {
    // There is no tail expression to blame, and {0, 0} is not an answer.
    const source = 'let f = fn(): int { };'
    expect(slice(source, only(source))).toBe('{ }')

    const method = 'class A { m(): int {} }'
    expect(slice(method, only(method))).toBe('{}')
  })

  it('keeps unreachable statements out of the inferred return type', () => {
    clean('let f = fn(): int { return 1; "s"; };')
    clean('let f = fn() { return 1; "s"; }; let a: int = f();')
  })

  it('merges a conditional return with the fallthrough value', () => {
    expect(
      codes('let f = fn(flag: bool): int { if (flag) { return 1; } "s"; };')
    ).toEqual(['type-mismatch'])
    clean('let f = fn(flag: bool): int { if (flag) { return 1; } 2; };')
  })

  it('ends the block after an expression-position if that always returns', () => {
    // The `let` never completes, so the tail is dead code — the same as the
    // statement form of the `if`, and what the compiler and VM execute.
    clean(
      'let f = fn(c: bool): int { let x = if (c) { return 1; } else { return 2; }; "s" };'
    )
    clean(
      'let f = fn(c: bool) { let x = if (c) { return 1; } else { return 2; }; "s" }; let a: int = f(true);'
    )
    clean(
      'let f = fn(c: bool): int { len([if (c) { return 1; } else { return 2; }]); "s" };'
    )
    // An arm that falls through keeps the rest of the block live.
    expect(
      codes(
        'let f = fn(c: bool): int { let x = if (c) { return 1; } else { 2 }; "s" };'
      )
    ).toEqual(['type-mismatch'])
    // Divergence inside a nested block or closure stays there.
    clean(
      'let f = fn(c: bool): string { if (c) { let x = if (c) { return "a"; } else { return "b"; }; } "s" };'
    )
    expect(
      codes(
        'let f = fn(c: bool): int { let g = fn() { if (c) { return 1; } else { return 2; } }; "s" };'
      )
    ).toEqual(['type-mismatch'])
  })

  it('accepts a guard clause whose only fallthrough is the implicit null', () => {
    // The body joins to `int?`; the optimistic null policy accepts it, same
    // as the expression-valued `fn(): int { if (flag) { 1 } }`.
    clean('let f = fn(flag: bool): int { if (flag) { return 1; } };')
    // Without an annotation the inferred type still remembers the null path.
    clean(
      'let f = fn(flag: bool) { if (flag) { return 1; } }; let a: int? = f(true);'
    )
    expect(
      codes(
        'let f = fn(flag: bool) { if (flag) { return 1; } }; let a: string = f(true);'
      )
    ).toEqual(['type-mismatch'])
  })

  it('reports a union callee whose members disagree on arity', () => {
    const source =
      'let c = true; let f = if (c) { fn(x: int): int { x } } else { fn(): int { 1 } }; f(1);'
    expect(only(source).message).toBe(
      "members of '(fn(int): int) | (fn(): int)' disagree on arity (0 vs 1); no call satisfies every member"
    )
  })

  it('closes over the defining type of a free variable', () => {
    clean('let a: int = 1; let f = fn(): int { a }; f();')
    expect(codes('let a: string = "s"; let f = fn(): int { a };')).toEqual([
      'type-mismatch',
    ])
  })

  it('leaves an unannotated recursive function silent', () => {
    clean(readFile('../../../examples/hello.monkey'))
    clean(
      'let fib = fn(x) { if (x < 2) { x } else { fib(x - 1) + fib(x - 2) } };'
    )
  })

  it('checks a recursive function once its return type is annotated', () => {
    clean(
      'let fib = fn(x: int): int { if (x < 2) { return x; } fib(x - 1) + fib(x - 2) };'
    )
    expect(
      codes(
        'let fib = fn(x: int): int { if (x < 2) { return "a"; } fib(x - 1) };'
      )
    ).toEqual(['type-mismatch'])
  })
})

// --- 7.8 classes -------------------------------------------------------------

describe('classes', () => {
  const point =
    'class Point { constructor(x: int, y: int) { this.x = x; this.y = y; } sum(): int { this.x + this.y } }'

  it('checks constructor arity and argument types', () => {
    clean(`${point} new Point(1, 2);`)
    expect(only(`${point} new Point(1, 2, 3);`).message).toBe(
      'Point constructor expects 2 arguments, got 3'
    )
    expect(codes(`${point} new Point(1, "s");`)).toEqual(['type-mismatch'])
  })

  it('defaults a class without a constructor to zero parameters', () => {
    clean('class Empty {} new Empty();')
    expect(codes('class Empty {} new Empty(1);')).toEqual(['arity-mismatch'])
  })

  it('rejects constructing a non-class', () => {
    expect(only('let f = fn(a: int): int { a }; new f();').message).toBe(
      "cannot construct 'fn(int): int'"
    )
  })

  it('accepts a guard clause in a method body', () => {
    clean(
      'class C { constructor() { this.x = 1; } get(flag: bool): int { if (flag) { return this.x; } } }'
    )
  })

  it('constructs a union of classes when every member agrees', () => {
    const union =
      'class A { constructor(v: int) { this.v = v; } } class B { constructor(v: int) { this.v = v; } } let c = true; let Type = if (c) { A } else { B };'
    clean(`${union} let o = new Type(1); let x: int = o.v;`)
    expect(codes(`${union} new Type("s");`)).toEqual(['type-mismatch'])
  })

  it('reports a union of classes whose constructors disagree on arity', () => {
    const source =
      'class A { constructor(v: int) { this.v = v; } } class B { constructor() { this.v = 1; } } let c = true; let Type = if (c) { A } else { B }; new Type(1);'
    expect(only(source).message).toBe(
      "constructors of 'A | B' disagree on arity (0 vs 1); no call satisfies every member"
    )
  })

  it('strips null from an optional class before constructing', () => {
    // An else-less `if` folds null into the class value; the optimistic
    // policy strips it, mirroring calls on an optional function.
    clean('class A {} let c = true; let Type = if (c) { A }; new Type();')
  })

  it('types a field read and a method call', () => {
    clean(
      `${point} let p = new Point(1, 2); let a: int = p.x; let b: int = p.sum();`
    )
    expect(
      codes(`${point} let p = new Point(1, 2); let a: string = p.x;`)
    ).toEqual(['type-mismatch'])
  })

  it('catches a misspelled property', () => {
    const source = `${point} let p = new Point(1, 2); p.nmae;`
    const diagnostic = only(source)
    expect(diagnostic.message).toBe("property 'nmae' does not exist on 'Point'")
    expect(slice(source, diagnostic)).toBe('nmae')
  })

  it('collects fields from every method, not just the constructor', () => {
    clean(
      'class Node { constructor() { this.value = 1; } connect(n: int) { this.next = n; } read(): int { this.next } }'
    )
    expect(
      codes(
        'class Node { constructor() { this.value = 1; } connect(n: int) { this.next = n; } read(): string { this.next } }'
      )
    ).toEqual(['type-mismatch'])
  })

  it('follows simple and transitive this aliases', () => {
    clean(
      'class C { constructor() { let self = this; let other = self; other.x = 1; } read(): int { this.x } }'
    )
  })

  it('does not treat an arbitrary instance as a this alias', () => {
    expect(
      codes(
        'class C { constructor() { this.x = 1; } touch(o: C) { o.y = 2; } }'
      )
    ).toEqual(['unknown-property'])
  })

  it('degrades a field that depends on an unannotated method', () => {
    clean(
      'class C { constructor() { this.value = this.make(); } make() { 1 } read(): string { this.value } }'
    )
    expect(
      codes(
        'class C { constructor() { this.value = this.make(); } make(): int { 1 } read(): string { this.value } }'
      )
    ).toEqual(['type-mismatch'])
  })

  it('degrades a field that depends on another field', () => {
    clean(
      'class C { constructor() { this.x = 1; this.y = this.x; } read(): string { this.y } }'
    )
    expect(
      codes(
        'class C { constructor() { this.x = 1; this.y = this.x; } read(): string { this.x } }'
      )
    ).toEqual(['type-mismatch'])
  })

  it('rejects assigning to a method name', () => {
    const source =
      'class Counter { constructor() { this.n = 0; } value(): int { this.n } } let c = new Counter(); c.value = 1;'
    expect(only(source).message).toBe(
      "assigning to method 'value' shadows it only on this instance of 'Counter'; annotate the receiver as 'any' if intended"
    )
  })

  it('rejects assigning to a method name from inside the class', () => {
    expect(
      codes(
        'class Counter { constructor() { this.value = 1; } value(): int { 0 } }'
      )
    ).toEqual(['assign-to-method'])
  })

  it('does not expose the constructor as an instance method', () => {
    // Every backend rejects `instance.constructor` at runtime; the checker
    // must not type it as a bound method.
    const source = `${point} let p = new Point(1, 2); p.constructor(3, 4);`
    const diagnostic = only(source)
    expect(diagnostic.message).toBe(
      "property 'constructor' does not exist on 'Point'"
    )
    expect(slice(source, diagnostic)).toBe('constructor')
    // Writing it declares an ordinary field; it is not an `assign-to-method`.
    clean(
      'class Tagged { constructor() { this.constructor = 1; } read(): int { this.constructor } }'
    )
  })

  it('rejects writing an unknown property', () => {
    expect(codes(`${point} let p = new Point(1, 2); p.z = 1;`)).toEqual([
      'unknown-property',
    ])
  })

  it('checks the type written into a known field', () => {
    expect(codes(`${point} let p = new Point(1, 2); p.x = "s";`)).toEqual([
      'type-mismatch',
    ])
  })

  it('keeps a class identity through an alias', () => {
    clean(`${point} let Type = Point; let p: Point = new Type(1, 2);`)
  })

  it('gives a shadowing class a fresh identity', () => {
    const source =
      'class A { constructor() { this.x = 1; } } let Old = A; class A { constructor() { this.y = 2; } } let a: A = new Old();'
    expect(only(source).message).toBe(
      "type 'A' is not assignable to type 'A' (same name, different declaration)"
    )
  })

  it('warns when a class shadows a builtin type name', () => {
    const diagnostic = only('class int {}')
    expect(diagnostic.severity).toBe('warning')
    expect(diagnostic.message).toBe(
      "class 'int' shadows a builtin type name; annotations cannot refer to it"
    )
    // Only the five builtin type names are reserved, not `Object.prototype`'s.
    clean('class toString {} let t: toString = new toString();')
    clean('class constructor {} let c: constructor = new constructor();')
  })

  it('exempts an any receiver', () => {
    clean(`${point} let p: any = new Point(1, 2); p.whatever; p.whatever = 1;`)
  })

  it('rejects a property on a non-instance receiver', () => {
    expect(codes('let a: int = 1; a.x;')).toEqual(['unknown-property'])
  })

  it('resolves a property across every union member', () => {
    clean(
      'class A { constructor() { this.v = 1; } } class B { constructor() { this.v = 2; } } let c = true; let o = if (c) { new A() } else { new B() }; let x: int = o.v;'
    )
    expect(
      codes(
        'class A { constructor() { this.v = 1; } } class B { constructor() { this.w = 2; } } let c = true; let o = if (c) { new A() } else { new B() }; o.v;'
      )
    ).toEqual(['unknown-property'])
  })

  it('types this inside a nested function', () => {
    clean(
      'class C { constructor() { this.x = 1; } wrap(): fn(): int { fn(): int { this.x } } }'
    )
  })
})

// --- annotations -------------------------------------------------------------

describe('annotations', () => {
  it('reports an unknown type name and continues as any', () => {
    const source = 'let p: Pointt = 1;'
    const diagnostic = only(source)
    expect(diagnostic.message).toBe("unknown type 'Pointt'")
    expect(slice(source, diagnostic)).toBe('Pointt')
  })

  it('treats an Object.prototype key as an unknown type name', () => {
    for (const name of ['toString', 'constructor', 'hasOwnProperty']) {
      const source = `let f = fn(x: ${name}) { x }; f(1);`
      const diagnostic = only(source)
      expect(diagnostic.code).toBe('unknown-type-name')
      expect(slice(source, diagnostic)).toBe(name)
    }
  })

  it('keeps builtin type names ahead of a class of the same name', () => {
    // `class int` warns, and `int` in an annotation still means the primitive.
    expect(codes('class int {} let a: int = 1;')).toEqual([
      'reserved-type-name',
    ])
  })

  it('cannot forward-reference a class', () => {
    expect(codes('let p: Later? = if (false) { 1 }; class Later {}')).toEqual([
      'unknown-type-name',
    ])
  })

  it('normalizes a doubled question mark', () => {
    clean('let a: int?? = 1;')
  })

  it('checks parameter and return annotations on methods', () => {
    clean('class C { scale(n: int): int { n * 2 } } new C().scale(2);')
    expect(codes('class C { scale(n: int): int { "s" } }')).toEqual([
      'type-mismatch',
    ])
  })
})

// --- ordering ----------------------------------------------------------------

describe('diagnostic ordering', () => {
  it('sorts by span', () => {
    const source = 'let a: int = "s"; let b: string = 1;'
    expect(check(source).diagnostics.map((d) => d.span!.start)).toEqual([
      13, 34,
    ])
  })
})

function readFile(relative: string): string {
  return readFileSync(fileURLToPath(new URL(relative, import.meta.url)), 'utf8')
}
