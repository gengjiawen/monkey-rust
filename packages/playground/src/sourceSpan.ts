export interface SourceSpanLike {
  start: number
  end: number
}

function utf8Width(character: string): number {
  const codePoint = character.codePointAt(0) ?? 0
  if (codePoint <= 0x7f) return 1
  if (codePoint <= 0x7ff) return 2
  if (codePoint <= 0xffff) return 3
  return 4
}

/**
 * Rust lexer spans count UTF-8 bytes, while CodeMirror positions count UTF-16
 * code units. Map a byte boundary back into the browser string, clamping
 * malformed or out-of-range offsets to a safe document position.
 */
export function utf8ByteOffsetToUtf16(
  source: string,
  byteOffset: number
): number {
  if (Number.isNaN(byteOffset) || byteOffset <= 0) {
    return 0
  }

  const target = Math.floor(byteOffset)
  let bytes = 0
  let utf16 = 0

  for (const character of source) {
    const width = utf8Width(character)

    if (bytes + width > target) {
      return utf16
    }

    bytes += width
    utf16 += character.length
    if (bytes === target) {
      return utf16
    }
  }

  return source.length
}

export function utf8ByteSpanToUtf16(
  source: string,
  span: SourceSpanLike
): SourceSpanLike {
  return {
    start: utf8ByteOffsetToUtf16(source, span.start),
    end: utf8ByteOffsetToUtf16(source, span.end),
  }
}

/** Map a CodeMirror UTF-16 position back to a Rust UTF-8 byte boundary. */
export function utf16OffsetToUtf8Byte(
  source: string,
  utf16Offset: number
): number {
  if (Number.isNaN(utf16Offset) || utf16Offset <= 0) {
    return 0
  }

  const target = Math.floor(utf16Offset)
  let bytes = 0
  let utf16 = 0

  for (const character of source) {
    if (utf16 + character.length > target) {
      return bytes
    }

    utf16 += character.length
    bytes += utf8Width(character)
    if (utf16 === target) {
      return bytes
    }
  }

  return bytes
}

export function utf16SpanToUtf8Byte(
  source: string,
  span: SourceSpanLike
): SourceSpanLike {
  return {
    start: utf16OffsetToUtf8Byte(source, span.start),
    end: utf16OffsetToUtf8Byte(source, span.end),
  }
}
