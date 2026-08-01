import type {
  DebuggerFrame,
  DebuggerHeap,
  DebuggerHit,
  DebuggerRunError,
  DebuggerRunOk,
  DebuggerSlot,
  DebuggerValue,
} from '../debuggerReport'
import type { ValueKind } from '../gcReport'

export function inlineValue(
  display: string,
  kind: ValueKind = 'integer'
): DebuggerValue {
  return { kind, display, heapId: null }
}

export function refValue(
  heapId: number,
  display: string,
  kind: ValueKind = 'array'
): DebuggerValue {
  return { kind, display, heapId }
}

export function slot(
  name: string,
  slotIndex: number,
  value: DebuggerValue | null
): DebuggerSlot {
  return { name, slot: slotIndex, initialized: value !== null, value }
}

export function frame(overrides: Partial<DebuggerFrame> = {}): DebuggerFrame {
  return {
    name: 'main',
    currentSpan: { start: 0, end: 1 },
    callee: null,
    locals: [],
    captures: [],
    temporaries: [],
    ...overrides,
  }
}

export function emptyHeap(overrides: Partial<DebuggerHeap> = {}): DebuggerHeap {
  return {
    objects: [],
    edges: [],
    omittedObjects: 0,
    omittedEdges: 0,
    ...overrides,
  }
}

export function hit(
  index: number,
  overrides: Partial<DebuggerHit> = {}
): DebuggerHit {
  return {
    index,
    span: { start: 10 * index, end: 10 * index + 9 },
    frames: [frame()],
    globals: [],
    heap: emptyHeap(),
    ...overrides,
  }
}

export function okEnvelope(
  hits: DebuggerHit[],
  overrides: Partial<DebuggerRunOk> = {}
): DebuggerRunOk {
  return {
    status: 'ok',
    result: 'null',
    stdout: '',
    hits,
    droppedHits: 0,
    ...overrides,
  }
}

export function errorEnvelope(
  hits: DebuggerHit[],
  overrides: Partial<Omit<DebuggerRunError, 'status'>> = {}
): DebuggerRunError {
  return {
    status: 'error',
    stage: 'runtime',
    kind: 'call',
    message: 'calling non-function',
    span: { start: 3, end: 8 },
    stdout: '',
    hits,
    droppedHits: 0,
    ...overrides,
  }
}
