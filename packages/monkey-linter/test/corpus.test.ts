import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { lint } from '../src'
import type { Diagnostic } from '../src/types'

const examplesDir = fileURLToPath(new URL('../../../examples', import.meta.url))
const exampleFiles = readdirSync(examplesDir).filter((name) =>
  name.endsWith('.monkey')
)

// Valid programs (every reference resolves) covering the constructs the walk and
// scope analysis descend into: closures, recursion, arrays, hashes, index and
// property access, `new`/`this`, unary/binary operators, and redeclaration.
const validPrograms = [
  'puts("before"); 40 + 2;',
  'let people = [{"name": "Anna"}]; people[0]["name"];',
  'let factorial = fn(n) { if (n == 0) { 1 } else { n * factorial(n - 1) } }; factorial(6);',
  'let make = fn(value) { fn(extra) { value + extra } }; make(40)(2);',
  'let flag = !true; if (flag) { 1 } else { 2 };',
  'let xs = [1, 2, 3]; puts(len(push(xs, 4))); first(xs); last(xs); rest(xs);',
  `class Point {
     constructor(x) { this.x = x; }
     read() { this.x }
   }
   let p = new Point(42);
   puts(p);
   p.read();`,
  'let v = 1; let g = fn() { v }; let v = 2; puts(g()); v;',
  '9223372036854775807 + 2;',
  'let h = {"a": 1, "b": 2}; h["a"];',
]

function byteLength(source: string): number {
  return new TextEncoder().encode(source).length
}

function assertWellFormed(diagnostics: Diagnostic[], source: string): void {
  const limit = byteLength(source)
  for (const diagnostic of diagnostics) {
    expect(typeof diagnostic.rule).toBe('string')
    expect(diagnostic.severity === 'warn' || diagnostic.severity === 'error').toBe(
      true
    )
    if (diagnostic.span) {
      expect(diagnostic.span.start).toBeGreaterThanOrEqual(0)
      expect(diagnostic.span.end).toBeGreaterThanOrEqual(diagnostic.span.start)
      expect(diagnostic.span.end).toBeLessThanOrEqual(limit)
    }
  }
}

describe('corpus smoke', () => {
  it.each(exampleFiles)(
    'lints example %s with in-bounds spans',
    (file) => {
      const source = readFileSync(`${examplesDir}/${file}`, 'utf8')
      const { diagnostics } = lint(source)
      assertWellFormed(diagnostics, source)
      const rules = diagnostics.map((diagnostic) => diagnostic.rule)
      expect(rules).not.toContain('parse-error')
      expect(rules).not.toContain('validation-error')
    }
  )

  it.each(validPrograms)('lints %s cleanly of analysis errors', (source) => {
    const { diagnostics } = lint(source)
    assertWellFormed(diagnostics, source)
    const rules = diagnostics.map((diagnostic) => diagnostic.rule)
    expect(rules).not.toContain('parse-error')
    expect(rules).not.toContain('validation-error')
  })
})
