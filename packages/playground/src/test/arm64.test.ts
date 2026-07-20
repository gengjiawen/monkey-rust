import { describe, expect, it } from 'vitest'

import {
  arm64RangesForSourceOffset,
  parseArm64BuildEnvelope,
  spanForArm64Cursor,
  type Arm64BuildSuccess,
} from '../arm64'

// Fixture for source `1 + 2`: spans {0,1} for `1`, {4,5} for `2`, {0,5} for
// the addition. Assembly text offsets per line (newline-separated):
// {0,5} {6,11} {12,24} {25,37} {38,54} {55,60} {61,61} {62,74}.
const okLines = [
  { text: '.text', kind: 'directive', span: null },
  { text: 'main:', kind: 'label', span: null },
  { text: '  mov x0, #1', kind: 'code', span: { start: 0, end: 1 } },
  { text: '  mov x1, #2', kind: 'code', span: { start: 4, end: 5 } },
  { text: '  add x0, x0, x1', kind: 'code', span: { start: 0, end: 5 } },
  { text: '  ret', kind: 'code', span: { start: 0, end: 5 } },
  { text: '', kind: 'blank', span: null },
  { text: '  mov x2, #1', kind: 'code', span: { start: 0, end: 1 } },
]

function okBuild(): Arm64BuildSuccess {
  const build = parseArm64BuildEnvelope(
    JSON.stringify({ status: 'ok', lines: okLines })
  )
  if (build.status !== 'ok') {
    throw new Error('fixture must parse as a success envelope')
  }
  return build
}

describe('parseArm64BuildEnvelope', () => {
  it('joins parsed lines into the assembly document', () => {
    const build = okBuild()
    expect(build.text).toBe(okLines.map((line) => line.text).join('\n'))
    expect(build.lines[2]).toEqual({
      text: '  mov x0, #1',
      kind: 'code',
      span: { start: 0, end: 1 },
    })
    expect(build.lines[1].span).toBeNull()
  })

  it('passes error envelopes through with their span', () => {
    expect(
      parseArm64BuildEnvelope(
        JSON.stringify({
          status: 'error',
          stage: 'compile',
          message: "undefined variable 'missing'",
          span: { start: 0, end: 7 },
        })
      )
    ).toEqual({
      status: 'error',
      stage: 'compile',
      message: "undefined variable 'missing'",
      span: { start: 0, end: 7 },
    })
    expect(
      parseArm64BuildEnvelope(
        JSON.stringify({
          status: 'error',
          stage: 'parse',
          message: 'expected token',
          span: null,
        })
      ).span
    ).toBeNull()
  })

  it.each([
    ['nope', /not valid JSON/],
    ['[]', /must be an object/],
    ['{"status":"weird"}', /status must be "ok" or "error"/],
    ['{"status":"error","stage":"weird","message":"m"}', /stage must be/],
    ['{"status":"error","stage":"parse","message":5}', /message must be/],
    ['{"status":"ok","lines":5}', /lines must be an array/],
    ['{"status":"ok","lines":[5]}', /lines\[0\] must be an object/],
    ['{"status":"ok","lines":[{"text":5,"kind":"code"}]}', /text must be/],
    ['{"status":"ok","lines":[{"text":"x","kind":"nop"}]}', /kind must be/],
    [
      '{"status":"ok","lines":[{"text":"x","kind":"code","span":{"start":-1,"end":0}}]}',
      /0 <= start <= end/,
    ],
    [
      '{"status":"ok","lines":[{"text":"x","kind":"code","span":{"start":3,"end":1}}]}',
      /0 <= start <= end/,
    ],
  ])('rejects malformed envelope %s', (serialized, message) => {
    expect(() => parseArm64BuildEnvelope(serialized)).toThrow(message)
  })
})

describe('spanForArm64Cursor', () => {
  it('returns the span of the line under the cursor', () => {
    const build = okBuild()
    expect(spanForArm64Cursor(build, 13)).toEqual({ start: 0, end: 1 })
    expect(spanForArm64Cursor(build, 38)).toEqual({ start: 0, end: 5 })
  })

  it('returns null on synthetic lines and clamps past the end', () => {
    const build = okBuild()
    expect(spanForArm64Cursor(build, 0)).toBeNull()
    expect(spanForArm64Cursor(build, 10_000)).toEqual({ start: 0, end: 1 })
  })
})

describe('arm64RangesForSourceOffset', () => {
  it('picks the narrowest enclosing span', () => {
    // Offset 0 sits in {0,1} and {0,5}; the literal wins over the addition.
    expect(arm64RangesForSourceOffset(okBuild(), 4)).toEqual([
      { from: 25, to: 37 },
    ])
  })

  it('merges adjacent matching lines into one range', () => {
    // Offset 2 only falls in {0,5}: the add and ret lines are adjacent.
    expect(arm64RangesForSourceOffset(okBuild(), 2)).toEqual([
      { from: 38, to: 60 },
    ])
  })

  it('keeps non-adjacent matching lines as separate ranges', () => {
    expect(arm64RangesForSourceOffset(okBuild(), 0)).toEqual([
      { from: 12, to: 24 },
      { from: 62, to: 74 },
    ])
  })

  it('returns no ranges when nothing encloses the offset', () => {
    expect(arm64RangesForSourceOffset(okBuild(), 99)).toEqual([])
  })
})
