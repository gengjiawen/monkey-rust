import { describe, expect, it } from 'vitest'

import {
  utf16OffsetToUtf8Byte,
  utf8ByteOffsetToUtf16,
  utf8ByteSpanToUtf16,
} from '../sourceSpan'

describe('source span offsets', () => {
  it('leaves ASCII offsets unchanged', () => {
    const source = 'let value = 1;'

    expect(utf8ByteOffsetToUtf16(source, 9)).toBe(9)
    expect(utf16OffsetToUtf8Byte(source, 9)).toBe(9)
    expect(utf8ByteSpanToUtf16(source, { start: 4, end: 9 })).toEqual({
      start: 4,
      end: 9,
    })
  })

  it('maps two-byte characters between UTF-8 bytes and UTF-16 positions', () => {
    const source = 'éx'

    expect(utf8ByteOffsetToUtf16(source, 1)).toBe(0)
    expect(utf8ByteOffsetToUtf16(source, 2)).toBe(1)
    expect(utf8ByteOffsetToUtf16(source, 3)).toBe(2)
    expect(utf16OffsetToUtf8Byte(source, 1)).toBe(2)
    expect(utf16OffsetToUtf8Byte(source, 2)).toBe(3)
  })

  it('maps three-byte characters between UTF-8 bytes and UTF-16 positions', () => {
    const source = '中x'

    expect(utf8ByteOffsetToUtf16(source, 1)).toBe(0)
    expect(utf8ByteOffsetToUtf16(source, 3)).toBe(1)
    expect(utf8ByteOffsetToUtf16(source, 4)).toBe(2)
    expect(utf16OffsetToUtf8Byte(source, 1)).toBe(3)
    expect(utf16OffsetToUtf8Byte(source, 2)).toBe(4)

    // U+0800 and U+FFFF bracket the three-byte encoding range.
    const boundaries = '\u0800\uffffx'
    expect(utf8ByteOffsetToUtf16(boundaries, 3)).toBe(1)
    expect(utf8ByteOffsetToUtf16(boundaries, 6)).toBe(2)
    expect(utf16OffsetToUtf8Byte(boundaries, 1)).toBe(3)
    expect(utf16OffsetToUtf8Byte(boundaries, 2)).toBe(6)
  })

  it('maps astral characters without splitting surrogate pairs', () => {
    const source = '😀x'

    expect(utf8ByteOffsetToUtf16(source, 3)).toBe(0)
    expect(utf8ByteOffsetToUtf16(source, 4)).toBe(2)
    expect(utf8ByteOffsetToUtf16(source, 5)).toBe(3)
    expect(utf16OffsetToUtf8Byte(source, 1)).toBe(0)
    expect(utf16OffsetToUtf8Byte(source, 2)).toBe(4)
    expect(utf16OffsetToUtf8Byte(source, 3)).toBe(5)
  })

  it('clamps invalid and out-of-range offsets to document boundaries', () => {
    const source = 'éx'

    expect(utf8ByteOffsetToUtf16(source, -1)).toBe(0)
    expect(utf8ByteOffsetToUtf16(source, Number.NaN)).toBe(0)
    expect(utf8ByteOffsetToUtf16(source, Number.POSITIVE_INFINITY)).toBe(2)
    expect(utf8ByteOffsetToUtf16(source, 99)).toBe(2)

    expect(utf16OffsetToUtf8Byte(source, -1)).toBe(0)
    expect(utf16OffsetToUtf8Byte(source, Number.NaN)).toBe(0)
    expect(utf16OffsetToUtf8Byte(source, Number.POSITIVE_INFINITY)).toBe(3)
    expect(utf16OffsetToUtf8Byte(source, 99)).toBe(3)
  })
})
