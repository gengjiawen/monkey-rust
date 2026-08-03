function utf8Width(character: string): number {
  const codePoint = character.codePointAt(0) ?? 0
  if (codePoint <= 0x7f) return 1
  if (codePoint <= 0x7ff) return 2
  if (codePoint <= 0xffff) return 3
  return 4
}

/**
 * The Rust lexer counts spans in UTF-8 bytes, while VS Code document offsets
 * count UTF-16 code units. Map a byte boundary back into the document string,
 * clamping malformed or out-of-range offsets to a safe position.
 *
 * The playground needs the same conversion for CodeMirror; the extension keeps
 * its own copy so that it depends on nothing but wasm and the type checker.
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
