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
