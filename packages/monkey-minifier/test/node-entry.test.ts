import { describe, expect, it } from 'vitest'

import { minify, parseProgram } from '../src/node'

describe('Node entrypoint', () => {
  it('loads the bundler-target Wasm package without native Wasm imports', () => {
    expect(minify('let longName = 40 + 2; longName;').code).toBe('let a=42;a;')
    expect(parseProgram('9007199254740993').body[0]).toMatchObject({
      type: 'Integer',
      raw: '9007199254740993',
    })
  })
})
