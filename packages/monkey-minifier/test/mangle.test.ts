import { describe, expect, it } from 'vitest'

import { minify } from '../src'

describe('binding-aware mangling', () => {
  const mangleOnly = (source: string) =>
    minify(source, { fold: false, mangle: true }).code

  it('tracks source-ordered rebinding and RHS lookup', () => {
    expect(mangleOnly('let value = 1; let value = value + 1; value;')).toBe(
      'let a=1;let b=a+1;b;'
    )
  })

  it('keeps recursive function metadata in sync with its let binding', () => {
    const code = mangleOnly(
      'let recurse = fn(number) { if (number == 0) { 0 } else { recurse(number - 1) } }; recurse(2);'
    )
    expect(code).not.toContain('recurse')
    expect(() => minify(code, { fold: false, mangle: false })).not.toThrow()
  })

  it('supports builtin shadowing and reserved names', () => {
    expect(mangleOnly('let len = fn(value) { value }; len(1);')).not.toContain(
      'let len'
    )
    expect(
      minify('let publicName = 1; publicName;', {
        fold: false,
        mangle: { reserved: ['publicName'] },
      }).code
    ).toBe('let publicName=1;publicName;')
  })

  it('never mangles class, property, or method names', () => {
    const code = mangleOnly(`
      class VeryLongClass {
        constructor(longValue) { this.longProperty = longValue; }
        longMethod() { this.longProperty }
      }
      let longInstance = new VeryLongClass(1);
      longInstance.longMethod();
    `)
    expect(code).toContain('class VeryLongClass')
    expect(code).toContain('new VeryLongClass')
    expect(code).toContain('.longProperty')
    expect(code).toContain('.longMethod')
    expect(code).not.toContain('longInstance')
    expect(code).not.toContain('longValue')
  })

  it('does not capture unresolved external names', () => {
    const code = mangleOnly('let longName = external; longName;')
    expect(code).toContain('external')
    expect(code).not.toContain('let external=')
  })
})
