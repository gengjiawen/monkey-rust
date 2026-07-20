import { lineAtOffset } from './bytecodeDebug'
import type { SourceSpan } from './gcReport'

export const arm64LineKinds = [
  'code',
  'label',
  'directive',
  'comment',
  'blank',
] as const

export type Arm64LineKind = (typeof arm64LineKinds)[number]

/**
 * One emitted `.s` line. `span` is the UTF-8 byte range of the source it was
 * lowered from; synthetic lines (prologues, directives, data) carry null.
 */
export interface Arm64Line {
  text: string
  kind: Arm64LineKind
  span: SourceSpan | null
}

export interface Arm64BuildSuccess {
  status: 'ok'
  lines: Arm64Line[]
  /** The assembly document: `lines[i].text` joined with newlines. */
  text: string
}

export type Arm64BuildStage = 'parse' | 'compile'

export interface Arm64BuildError {
  status: 'error'
  stage: Arm64BuildStage
  message: string
  span: SourceSpan | null
}

export type Arm64BuildEnvelope = Arm64BuildSuccess | Arm64BuildError

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function isLineKind(value: unknown): value is Arm64LineKind {
  return arm64LineKinds.includes(value as Arm64LineKind)
}

function parseSpan(value: unknown, path: string): SourceSpan | null {
  if (value === null || value === undefined) {
    return null
  }
  if (!isRecord(value)) {
    throw new Error(`${path} must be an object or null`)
  }
  const { start, end } = value
  if (
    typeof start !== 'number' ||
    typeof end !== 'number' ||
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(end) ||
    start < 0 ||
    end < start
  ) {
    throw new Error(`${path} must satisfy 0 <= start <= end`)
  }
  return { start, end }
}

export function parseArm64BuildEnvelope(
  serialized: string
): Arm64BuildEnvelope {
  let value: unknown
  try {
    value = JSON.parse(serialized) as unknown
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new Error(`arm64 envelope is not valid JSON: ${message}`)
  }
  if (!isRecord(value)) {
    throw new Error('arm64 envelope must be an object')
  }

  if (value.status === 'error') {
    const { stage, message } = value
    if (stage !== 'parse' && stage !== 'compile') {
      throw new Error('arm64 envelope.stage must be "parse" or "compile"')
    }
    if (typeof message !== 'string') {
      throw new Error('arm64 envelope.message must be a string')
    }
    return {
      status: 'error',
      stage,
      message,
      span: parseSpan(value.span, 'arm64 envelope.span'),
    }
  }

  if (value.status !== 'ok') {
    throw new Error('arm64 envelope.status must be "ok" or "error"')
  }
  if (!Array.isArray(value.lines)) {
    throw new Error('arm64 envelope.lines must be an array')
  }
  const lines = value.lines.map((line, index): Arm64Line => {
    const path = `arm64 envelope.lines[${index}]`
    if (!isRecord(line)) {
      throw new Error(`${path} must be an object`)
    }
    if (typeof line.text !== 'string') {
      throw new Error(`${path}.text must be a string`)
    }
    if (!isLineKind(line.kind)) {
      throw new Error(
        `${path}.kind must be one of ${arm64LineKinds.join(', ')}`
      )
    }
    return {
      text: line.text,
      kind: line.kind,
      span: parseSpan(line.span, `${path}.span`),
    }
  })
  return {
    status: 'ok',
    lines,
    text: lines.map((line) => line.text).join('\n'),
  }
}

/** Assembly-pane cursor (UTF-16 offset into `build.text`) → source span. */
export function spanForArm64Cursor(
  build: Arm64BuildSuccess,
  offset: number
): SourceSpan | null {
  return build.lines[lineAtOffset(build.text, offset)]?.span ?? null
}

/**
 * Source cursor (UTF-8 byte offset) → the assembly ranges to light up: every
 * line lowered from the narrowest source span containing the cursor, i.e. the
 * code of the most specific enclosing AST node. Ranges are UTF-16 offsets
 * into `build.text`; runs of adjacent matching lines merge into one range
 * (newline included between them, excluded at the end).
 */
export function arm64RangesForSourceOffset(
  build: Arm64BuildSuccess,
  byteOffset: number
): Array<{ from: number; to: number }> {
  let best: SourceSpan | null = null
  for (const { span } of build.lines) {
    if (span === null || byteOffset < span.start || byteOffset > span.end) {
      continue
    }
    if (best === null || span.end - span.start < best.end - best.start) {
      best = span
    }
  }
  if (best === null) {
    return []
  }
  const { start, end } = best
  const ranges: Array<{ from: number; to: number }> = []
  let from = 0
  for (const line of build.lines) {
    if (
      line.span !== null &&
      line.span.start === start &&
      line.span.end === end
    ) {
      const to = from + line.text.length
      const last = ranges[ranges.length - 1]
      if (last !== undefined && last.to === from - 1) {
        last.to = to
      } else {
        ranges.push({ from, to })
      }
    }
    from += line.text.length + 1
  }
  return ranges
}
