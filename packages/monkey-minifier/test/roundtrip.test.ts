import { readFileSync } from 'node:fs'

import { describe, expect, it } from 'vitest'

import {
  eliminateDeadLets,
  foldConstants,
  mangle,
  minify,
  printProgram,
} from '../src'
import { propagateConstants } from '../src/propagate'
import { canonical, parseProgram } from './helpers'

const corpus = [
  readFileSync(new URL('./corpus/core.monkey', import.meta.url), 'utf8'),
  readFileSync(new URL('./corpus/classes.monkey', import.meta.url), 'utf8'),
  readFileSync(
    new URL('../../../examples/hello.monkey', import.meta.url),
    'utf8'
  ),
  'let value = 1 + 2 * 3; value;',
  'let choose = fn(flag, left, right) { if (flag) { left } else { right } }; choose(true, 1, 2);',
  'let data = [1, 9007199254740993, {"key": true}]; data[2]["key"];',
  'class Box { constructor(value) { this.value = value; } get() { this.value } } let box = new Box(42); box.get();',
  'let make = fn(x) { fn(y) { x + y } }; make(1)(2);',
]

describe('structural round trip', () => {
  it.each(corpus)('preserves the lossless AST for %s', (source) => {
    const code = minify(source, { fold: false, mangle: false }).code
    expect(canonical(parseProgram(code))).toEqual(
      canonical(parseProgram(source))
    )
  })

  it.each(corpus)(
    'prints every transformed AST without structural drift for %s',
    (source) => {
      const transformed = parseProgram(source)
      foldConstants(transformed)
      while (propagateConstants(transformed)) {
        foldConstants(transformed)
      }
      eliminateDeadLets(transformed)
      mangle(transformed)
      const reparsed = parseProgram(printProgram(transformed))
      expect(canonical(reparsed)).toEqual(canonical(transformed))
    }
  )

  it('is idempotent for every pass combination', () => {
    const source = `
      let unused = 10 + 20;
      let longFunction = fn(longArgument) { longArgument + 1 };
      longFunction(41);
    `
    for (const options of [
      { fold: false, mangle: false },
      { fold: false, mangle: true },
      { fold: true, mangle: false },
      { fold: true, mangle: true },
    ]) {
      const once = minify(source, options).code
      expect(minify(once, options).code).toBe(once)
    }
  })

  it('reports parser failures as SyntaxError', () => {
    expect(() => minify('let =')).toThrow(SyntaxError)
  })
})
