import { describe, expect, it } from 'vitest'

import { minify } from '../src'

describe('compact printer', () => {
  const print = (source: string) =>
    minify(source, { fold: false, mangle: false }).code

  it.each([
    ['1 + 2 * 3', '1+2*3;'],
    ['(1 + 2) * 3', '(1+2)*3;'],
    ['1 - (2 - 3)', '1-(2-3);'],
    ['(1 - 2) - 3', '1-2-3;'],
    ['-(a + b)', '-(a+b);'],
    ['-a[0]', '-a[0];'],
    ['a - -b', 'a--b;'],
    ['if (true) { 1 } else { 2 }', 'if(true){1;}else{2;};'],
    ['fn(x, y) { return x + y }', 'fn(x,y){return x+y;};'],
    ['(fn(x) { x })(1)', '(fn(x){x;})(1);'],
    ['{"a": [1, 2], true: {}}', '{"a":[1,2],true:{}};'],
    ['new Thing(1).value[0]', 'new Thing(1).value[0];'],
  ])('prints %s', (source, expected) => {
    expect(print(source)).toBe(expected)
  })

  it('separates statements and does not append a semicolon to classes', () => {
    expect(
      print(
        'class Box { constructor(value) { this.value = value } get() { this.value } } let box = new Box(1); box.get()'
      )
    ).toBe(
      'class Box{constructor(value){this.value=value;}get(){this.value;}}let box=new Box(1);box.get();'
    )
  })

  it('preserves lossless integer and string spelling', () => {
    expect(print('9007199254740993; 9223372036854775807; "line\nbreak"')).toBe(
      '9007199254740993;9223372036854775807;"line\nbreak";'
    )
  })
})
