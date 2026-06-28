export interface Span {
  start: number
  end: number
}

export interface PcSpan {
  pc: number
  span: Span
}

export interface DebugInfo {
  pcSpans: PcSpan[]
}

export type InstructionScope =
  | { type: 'main' }
  | { type: 'function'; constantIndex: number }

export interface InstructionLineMapping {
  line: number
  pc: number
  scope: InstructionScope
}

export interface BytecodeDebugView {
  detail: string
  mainDebugInfo: DebugInfo
  functionDebugInfo: Record<string, DebugInfo>
  instructionLines: InstructionLineMapping[]
}

export function spanForPc(
  debugInfo: DebugInfo,
  pc: number,
): Span | null {
  for (let index = debugInfo.pcSpans.length - 1; index >= 0; index -= 1) {
    const entry = debugInfo.pcSpans[index]
    if (entry.pc <= pc) {
      return entry.span
    }
  }

  return null
}

export function lineAtOffset(text: string, offset: number): number {
  if (offset <= 0) {
    return 0
  }

  let line = 0
  for (let index = 0; index < offset && index < text.length; index += 1) {
    if (text[index] === '\n') {
      line += 1
    }
  }

  return line
}

export function findInstructionLineAt(
  instructionLines: InstructionLineMapping[],
  line: number,
): InstructionLineMapping | null {
  return instructionLines.find((entry) => entry.line === line) ?? null
}

export function debugInfoForScope(
  view: BytecodeDebugView,
  scope: InstructionScope,
): DebugInfo | null {
  if (scope.type === 'main') {
    return view.mainDebugInfo
  }

  return view.functionDebugInfo[String(scope.constantIndex)] ?? null
}

export function spanForBytecodeCursor(
  view: BytecodeDebugView,
  offset: number,
): Span | null {
  const line = lineAtOffset(view.detail, offset)
  const mapping = findInstructionLineAt(view.instructionLines, line)
  if (mapping == null) {
    return null
  }

  const debugInfo = debugInfoForScope(view, mapping.scope)
  if (debugInfo == null) {
    return null
  }

  return spanForPc(debugInfo, mapping.pc)
}
