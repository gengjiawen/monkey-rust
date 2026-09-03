import { describe, expect, it } from 'vitest'

import { minify } from '../src'
import { observe } from './helpers'

const programs = [
  'puts("before"); 40 + 2;',
  'let value = 1; let read = fn() { value }; let value = 2; puts(read()); value;',
  'let factorial = fn(n) { if (n == 0) { 1 } else { n * factorial(n - 1) } }; factorial(6);',
  'let len = fn(value) { value }; len(7);',
  'let duplicate = fn(value, value) { value }; duplicate(1, 2);',
  'let outer = 1; if (false) { let branch = 2; } else { branch; };',
  'let make = fn(value) { fn(extra) { value + extra } }; make(40)(2);',
  `class VisibleClass {
     constructor(value) { this.longProperty = value; }
     read() { fn() { this.longProperty }() }
   }
   puts(VisibleClass);
   let instance = new VisibleClass(42);
   puts(instance);
   instance.read();`,
  '9223372036854775807 + 2;',
  'puts("before"); 1 / 0;',
  // DCE must retain every potentially throwing initializer.
  'let unused = 1.missing; 42;',
  'let unused = [][0]; 42;',
  'let unused = 1 + true; 42;',
  'let f = fn() { 42; let unused = 1; }; f();',
  'let f = fn() { if (true) { 1; let first = 2; } else { let second = 2; 1; } }; f();',
  'if (true) { 42; let unused = 1; };',
  'if (true) { 1 } else { missing };',
  'let value = 1; let value = if (true) { fn() { value } } else { fn() { 0 } }; value();',
  // Pins that closures render without their (mangled) parameter names.
  'let make = fn(longArg) { longArg }; make;',
  // Pins that a removed top-level trailing let never contributed to the result.
  '42; let tail = 5;',
  // Constant propagation folds this to `print(2);` without observable change.
  'let a = 1 + 1;\nlet b= a + 1;\nprint(a)',
  // A conditional let's slot may stay unset; propagation must leave it alone.
  'let v = 1; if (1 > 2) { let v = 2; }; puts(v);',
  // After the block the name means the arm's last `let` or the binding from
  // before the branch, so neither may be dropped and both must keep one name.
  'let v = 1; if (1 > 2) { let v = 2; let v = 3; }; puts(v);',
  'let v = 1; if (1 < 2) { let v = 2; let v = 3; }; puts(v);',
  'let v = 1; if (1 < 2) { let v = 2; } else { let v = 3; }; puts(v);',
  'let v = 1; if (1 > 2) { let v = 2; } else { let v = 3; }; puts(v);',
  'let v = 1; if (1 < 2) { if (1 > 2) { let v = 2; } let v = 4; }; puts(v);',
  // ... including when the binding it shadows lives in an enclosing scope,
  // where a skipped arm leaves the read on the captured free variable.
  'let v = 1; let f = fn() { if (1 > 2) { let v = 2; } v }; puts(f());',
  'let v = 1; let f = fn() { if (1 > 2) { let v = 2; let v = 3; } v }; puts(f());',
  'let v = 1; let f = fn(p) { if (1 > 2) { let p = 2; } p }; puts(f(9));',
  // A block shadowing a builtin is left alone: it cannot share a name with one.
  'if (1 > 2) { let len = 5; }; puts(len("ab"));',
  // A closure keeps observing the pre-redeclaration slot.
  'let v = 1; let g = fn() { v }; let v = 2; puts(g()); v;',
  // ... including when a block redeclares the name in between: the block
  // writes the captured slot, but the `let` after it is unconditional again
  // and starts a binding of its own.
  'let v = 1; let g = fn() { v }; if (false) { let v = 2; }; let v = 3; puts(g()); v;',
  'if (true) { let v = 1; let g = fn() { v }; }; let v = 2; puts(g()); v;',
  // `new` requires its callee to stay an identifier reference.
  'let a = 1; let b = new a(); b;',
  // `debugger` is completion-transparent: the statement before a trailing run
  // of debuggers still decides the block's value, and no pass may drop the
  // keyword or fold across it.
  'puts("before"); 5; debugger;',
  'let f = fn(n) { n * 2; debugger; }; debugger; puts(f(21)); f(2);',
  // The dead trailing let sits before the debugger and must survive DCE, or
  // the function's implicit return flips from null to 42.
  'let f = fn() { 42; let unused = 1; debugger; }; f();',
  'if (true) { 42; debugger; };',
]

describe('GC VM differential semantics', () => {
  it.each(programs)('preserves status/result/stdout for %s', (source) => {
    const optimized = minify(source).code
    expect(observe(optimized)).toEqual(observe(source))
  })
})

// Every backend erases type annotations, so the minifier must too: annotating a
// program may not change a single byte of its output. Each pair is the same
// program written with and without annotations.
const annotated: [string, string][] = [
  ['let a: int = 1; a;', 'let a = 1; a;'],
  ['let a: [int]? = [1]; a;', 'let a = [1]; a;'],
  [
    'let f: fn(int): int = fn(n: int): int { n * 2 }; f(21);',
    'let f = fn(n) { n * 2 }; f(21);',
  ],
  ['fn(a: int, b): int { a + b }(1, 2);', 'fn(a, b) { a + b }(1, 2);'],
  [
    'class Box { constructor(v: int) { this.v = v; } get(): int { this.v } } new Box(1).get();',
    'class Box { constructor(v) { this.v = v; } get() { this.v } } new Box(1).get();',
  ],
  [
    'let m: {string: [int]} = {"a": [1]}; let g: fn(): null = fn(): null { puts(m) }; g();',
    'let m = {"a": [1]}; let g = fn() { puts(m) }; g();',
  ],
]

describe('type annotations are erased', () => {
  it.each(annotated)(
    'minifies %s exactly like its erased twin',
    (withTypes, without) => {
      expect(minify(withTypes).code).toBe(minify(without).code)
    }
  )

  it.each(annotated)(
    'runs %s exactly like its erased twin',
    (withTypes, without) => {
      expect(observe(minify(withTypes).code)).toEqual(observe(without))
    }
  )

  it('never emits a colon outside a hash literal', () => {
    const { code } = minify(
      'let a: {string: int}? = {"k": 1}; let f = fn(x: int): int { x }; f(1);',
      { fold: false, mangle: false }
    )
    expect(code).toBe('let a={"k":1};let f=fn(x){x;};f(1);')
  })
})
