import { describe, expect, it } from 'vitest'

import { parseGcRunEnvelope } from '../gcReport'

describe('parseGcRunEnvelope', () => {
  it('rejects an untagged partial report', () => {
    expect(() => parseGcRunEnvelope('{"report":{}}')).toThrow(
      'GC response status must be ok or error'
    )
  })

  it('accepts a structured stage error', () => {
    expect(
      parseGcRunEnvelope(
        JSON.stringify({
          status: 'error',
          stage: 'parse',
          message: 'expected expression',
          span: null,
        })
      )
    ).toEqual({
      status: 'error',
      stage: 'parse',
      message: 'expected expression',
      span: null,
    })
  })
})
