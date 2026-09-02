// Differential tests against the GC VM, in the shape of the minifier's
// `differential.test.ts` (docs/type-system-design.md section 12.3).
//
// The forward direction only holds on a *sound subset*: fully annotated, no
// `any` at an interface, nothing that leans on null-stripping, and no read of a
// field that may still be unset. On that corpus, zero checker errors implies
// the program runs without a type/arity/property failure.
//
// The gradual corpus gets the reverse assertion only — a checker error there
// must correspond to a failure the runtime can actually reach. The two
// directions are deliberately not symmetric: the checker is unsound by design
// on the gradual side, and stricter than the runtime in a few documented places
// (heterogeneous hash lookup, unknown property write).

import { run_gc_with_report } from '@gengjiawen/monkey-wasm'
import { describe, expect, it } from 'vitest'

import { check } from '../src/node'

interface RunReport {
  status: 'ok' | 'error'
  stage?: string
  kind?: string
  message?: string
}

function run(source: string): RunReport {
  return JSON.parse(run_gc_with_report(source)) as RunReport
}

function errors(source: string): string[] {
  return check(source)
    .diagnostics.filter((diagnostic) => diagnostic.severity === 'error')
    .map((diagnostic) => diagnostic.code)
}

/** Fully annotated programs that must both check and run cleanly. */
const sound = [
  'let a: int = 1 + 2 * 3 - 4 / 2; puts(a);',
  'let greet = fn(name: string): string { "hi, " + name }; puts(greet("anna"));',
  'let twice = fn(f: fn(int): int, n: int): int { f(f(n)) }; puts(twice(fn(x: int): int { x * 2 }, 3));',
  'let xs: [int] = push(push([], 1), 2); puts(len(xs));',
  'let cmp = fn(a: int, b: int): bool { a < b }; puts(cmp(1, 2));',
  'let pick = fn(flag: bool, a: int, b: int): int { if (flag) { return a; } b }; puts(pick(true, 1, 2));',
  'let fib = fn(n: int): int { if (n < 2) { return n; } fib(n - 1) + fib(n - 2) }; puts(fib(10));',
  'let h: {string: int} = {"a": 1, "b": 2}; let v: int? = h["a"]; puts(v);',
  'let adder = fn(a: int): fn(int): int { fn(b: int): int { a + b } }; puts(adder(1)(2));',
  'let captured: int = 1; let read: fn(): int = fn(): int { captured + 1 }; let captured: string = "later"; puts(read());',
  'class Point { constructor(x: int, y: int) { this.x = x; this.y = y; } sum(): int { this.x + this.y } } let p = new Point(1, 2); puts(p.sum());',
  'class Counter { constructor() { this.n = 0; } bump(k: int): int { this.n + k } } puts(new Counter().bump(2));',
  'let sizes = fn(a: [int], b: [string]): int { len(a) + len(b) }; puts(sizes([1], ["x", "y"]));',
  // Equality is total in every backend, so none of these is a checker error
  // and none of them fails at runtime (gc/backend_parity_test.rs).
  'let xs: [int] = [1]; puts(xs == xs);',
  'let h: {string: int} = {"a": 1}; puts(h == h);',
  'let f: fn(): int = fn(): int { 1 }; puts(f == f);',
]

/** Gradual programs the checker rejects and the GC VM also refuses to run. */
const rejected: [string, string][] = [
  ['1 + "a";', 'operator-type'],
  ['let a: int = 1; a();', 'not-callable'],
  ['let f = fn(a: int): int { a }; f(1, 2);', 'arity-mismatch'],
  ['let f = fn(a: int): int { a + 1 }; f("s");', 'type-mismatch'],
  ['let s: string = "a"; s[0];', 'invalid-index'],
  ['let xs: [int] = [1]; {xs: 1};', 'invalid-hash-key'],
  [
    'class Point { constructor(x: int) { this.x = x; } } new Point(1, 2);',
    'arity-mismatch',
  ],
  [
    'class Point { constructor(x: int) { this.x = x; } } let p = new Point(1); p.nmae;',
    'unknown-property',
  ],
]

describe('sound subset', () => {
  it.each(sound)('checks clean: %s', (source) => {
    expect(errors(source)).toEqual([])
  })

  it.each(sound)('runs clean: %s', (source) => {
    expect(run(source).status).toBe('ok')
  })
})

describe('rejected programs really fail at runtime', () => {
  it.each(rejected)('reports %s as %s', (source, code) => {
    expect(errors(source)).toContain(code)
  })

  it.each(rejected)('and the GC VM refuses to run %s', (source) => {
    const report = run(source)
    expect(report.status).toBe('error')
    expect(report.stage).toBe('runtime')
  })
})
