import { describe, expect, it } from 'vitest'

import {
  isTypeAnnotation,
  printTypeAnnotation,
  type TypeAnnotation,
} from '../src/index'

const span = { start: 0, end: 0 }

function named(name: string): TypeAnnotation {
  return { type: 'NamedType', name, span }
}

describe('printTypeAnnotation', () => {
  it('prints every annotation shape', () => {
    expect(printTypeAnnotation(named('int'))).toBe('int')
    expect(
      printTypeAnnotation({ type: 'ArrayType', element: named('int'), span })
    ).toBe('[int]')
    expect(
      printTypeAnnotation({
        type: 'HashType',
        key: named('string'),
        value: { type: 'ArrayType', element: named('int'), span },
        span,
      })
    ).toBe('{string: [int]}')
    expect(
      printTypeAnnotation({
        type: 'FunctionType',
        params: [named('int'), named('string')],
        return_type: named('bool'),
        span,
      })
    ).toBe('fn(int, string): bool')
    expect(
      printTypeAnnotation({ type: 'OptionalType', inner: named('int'), span })
    ).toBe('int?')
  })

  it('re-parenthesises a nullable function type', () => {
    // Without the parentheses the `?` would bind to the return type.
    expect(
      printTypeAnnotation({
        type: 'OptionalType',
        inner: {
          type: 'FunctionType',
          params: [named('int')],
          return_type: named('int'),
          span,
        },
        span,
      })
    ).toBe('(fn(int): int)?')
  })
})

describe('isTypeAnnotation', () => {
  it('accepts the five annotation nodes and nothing else', () => {
    for (const type of [
      'NamedType',
      'ArrayType',
      'HashType',
      'FunctionType',
      'OptionalType',
    ]) {
      expect(isTypeAnnotation({ type })).toBe(true)
    }

    for (const type of ['Let', 'IDENTIFIER', 'Param', 'FunctionDeclaration']) {
      expect(isTypeAnnotation({ type })).toBe(false)
    }
  })
})
