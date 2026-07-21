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
  'let unused = 1.missing; 42;',
  'let f = fn() { 42; let unused = 1; }; f();',
  'let f = fn() { if (true) { 1; let first = 2; } else { let second = 2; 1; } }; f();',
  'if (true) { 42; let unused = 1; };',
  'if (true) { 1 } else { missing };',
  'let value = 1; let value = if (true) { fn() { value } } else { fn() { 0 } }; value();',
]

describe('GC VM differential semantics', () => {
  it.each(programs)('preserves status/result/stdout for %s', (source) => {
    const optimized = minify(source).code
    expect(observe(optimized)).toEqual(observe(source))
  })
})
