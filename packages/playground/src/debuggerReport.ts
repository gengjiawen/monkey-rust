import type {
  EdgeRelation,
  GcRunStage,
  SourceSpan,
  ValueKind,
} from './gcReport'
import {
  isRecord,
  readEdgeRelation,
  readNumber,
  readRecord,
  readSpan,
  readString,
  uniqueIds,
  valueKinds,
} from './gcReport'

export interface DebuggerValue {
  kind: ValueKind
  display: string
  /**
   * Heap id for user-visible heap objects, null for inlined scalars. The id
   * may reference an object the snapshot budget omitted from `heap.objects`.
   */
  heapId: number | null
}

export interface DebuggerSlot {
  name: string
  slot: number
  initialized: boolean
  /** Null exactly when the slot's `let` has not executed yet. */
  value: DebuggerValue | null
}

export interface DebuggerCapture {
  name: string
  index: number
  value: DebuggerValue
}

export interface DebuggerTemporary {
  /** Absolute VM stack slot, so gaps between temporaries stay visible. */
  slot: number
  value: DebuggerValue
}

export interface DebuggerFrame {
  name: string
  currentSpan: SourceSpan | null
  /** Null only for the synthetic main frame. */
  callee: DebuggerValue | null
  locals: DebuggerSlot[]
  captures: DebuggerCapture[]
  temporaries: DebuggerTemporary[]
}

export interface DebuggerHeapMember {
  relation: EdgeRelation
  display: string
}

export interface DebuggerHeapObject {
  id: number
  kind: ValueKind
  label: string
  members: DebuggerHeapMember[]
}

export interface DebuggerHeapEdge {
  from: number
  to: number
  relation: EdgeRelation
}

export interface DebuggerHeap {
  objects: DebuggerHeapObject[]
  edges: DebuggerHeapEdge[]
  omittedObjects: number
  omittedEdges: number
}

export interface DebuggerHit {
  /** 1-based execution order. */
  index: number
  /** Span of the `debugger;` statement that recorded this hit. */
  span: SourceSpan | null
  /** Outermost (main) first, innermost (current) last. */
  frames: DebuggerFrame[]
  globals: DebuggerSlot[]
  heap: DebuggerHeap
}

export interface DebuggerRunOk {
  status: 'ok'
  result: string
  stdout: string
  hits: DebuggerHit[]
  droppedHits: number
}

/**
 * Unlike the GC report envelope, the error arm still carries the hits and
 * stdout recorded before the failure: they are part of the story the
 * debugger tells.
 */
export interface DebuggerRunError {
  status: 'error'
  stage: GcRunStage
  kind: string
  message: string
  span: SourceSpan | null
  stdout: string
  hits: DebuggerHit[]
  droppedHits: number
}

export type DebuggerRunEnvelope = DebuggerRunOk | DebuggerRunError

function readValueKind(
  record: Record<string, unknown>,
  path: string
): ValueKind {
  const kind = record.kind
  if (typeof kind !== 'string' || !valueKinds.includes(kind as ValueKind)) {
    throw new Error(`${path}.kind must be a known value kind`)
  }
  return kind as ValueKind
}

function readBoolean(
  record: Record<string, unknown>,
  key: string,
  path: string
): boolean {
  const value = record[key]
  if (typeof value !== 'boolean') {
    throw new Error(`${path}.${key} must be a boolean`)
  }
  return value
}

function readArray(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${path} must be an array`)
  }
  return value
}

function readValue(value: unknown, path: string): DebuggerValue {
  if (!isRecord(value)) {
    throw new Error(`${path} must be an object`)
  }
  const heapId = value.heapId
  if (
    heapId !== null &&
    (typeof heapId !== 'number' || !Number.isSafeInteger(heapId) || heapId < 0)
  ) {
    throw new Error(
      `${path}.heapId must be null or a non-negative safe integer`
    )
  }
  return {
    kind: readValueKind(value, path),
    display: readString(value, 'display', path),
    heapId,
  }
}

function readSlots(value: unknown, path: string): DebuggerSlot[] {
  return readArray(value, path).map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    const initialized = readBoolean(entry, 'initialized', entryPath)
    if (initialized === (entry.value === null)) {
      throw new Error(
        `${entryPath}.value must be null exactly when the slot is uninitialized`
      )
    }
    return {
      name: readString(entry, 'name', entryPath),
      slot: readNumber(entry, 'slot', entryPath),
      initialized,
      value: initialized ? readValue(entry.value, `${entryPath}.value`) : null,
    }
  })
}

function readCaptures(value: unknown, path: string): DebuggerCapture[] {
  return readArray(value, path).map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    return {
      name: readString(entry, 'name', entryPath),
      index: readNumber(entry, 'index', entryPath),
      value: readValue(entry.value, `${entryPath}.value`),
    }
  })
}

function readTemporaries(value: unknown, path: string): DebuggerTemporary[] {
  return readArray(value, path).map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    return {
      slot: readNumber(entry, 'slot', entryPath),
      value: readValue(entry.value, `${entryPath}.value`),
    }
  })
}

function readFrames(value: unknown, path: string): DebuggerFrame[] {
  const frames = readArray(value, path)
  if (frames.length === 0) {
    throw new Error(`${path} must contain at least the main frame`)
  }
  return frames.map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    return {
      name: readString(entry, 'name', entryPath),
      currentSpan: readSpan(entry.currentSpan),
      callee:
        entry.callee === null
          ? null
          : readValue(entry.callee, `${entryPath}.callee`),
      locals: readSlots(entry.locals, `${entryPath}.locals`),
      captures: readCaptures(entry.captures, `${entryPath}.captures`),
      temporaries: readTemporaries(
        entry.temporaries,
        `${entryPath}.temporaries`
      ),
    }
  })
}

function readHeap(value: unknown, path: string): DebuggerHeap {
  if (!isRecord(value)) {
    throw new Error(`${path} must be an object`)
  }
  const objects = readArray(value.objects, `${path}.objects`).map(
    (entry, index) => {
      const entryPath = `${path}.objects[${index}]`
      if (!isRecord(entry)) {
        throw new Error(`${entryPath} must be an object`)
      }
      const members = readArray(entry.members, `${entryPath}.members`).map(
        (member, memberIndex) => {
          const memberPath = `${entryPath}.members[${memberIndex}]`
          if (!isRecord(member)) {
            throw new Error(`${memberPath} must be an object`)
          }
          return {
            relation: readEdgeRelation(
              member.relation,
              `${memberPath}.relation`
            ),
            display: readString(member, 'display', memberPath),
          }
        }
      )
      return {
        id: readNumber(entry, 'id', entryPath),
        kind: readValueKind(entry, entryPath),
        label: readString(entry, 'label', entryPath),
        members,
      }
    }
  )
  const catalog = uniqueIds(
    objects,
    (object) => object.id,
    `${path}.objects`,
    'id'
  )
  const edges = readArray(value.edges, `${path}.edges`).map((entry, index) => {
    const entryPath = `${path}.edges[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    const from = readNumber(entry, 'from', entryPath)
    const to = readNumber(entry, 'to', entryPath)
    if (!catalog.has(from)) {
      throw new Error(`${entryPath}.from references unknown object ${from}`)
    }
    if (!catalog.has(to)) {
      throw new Error(`${entryPath}.to references unknown object ${to}`)
    }
    return {
      from,
      to,
      relation: readEdgeRelation(entry.relation, `${entryPath}.relation`),
    }
  })
  return {
    objects,
    edges,
    omittedObjects: readNumber(value, 'omittedObjects', path),
    omittedEdges: readNumber(value, 'omittedEdges', path),
  }
}

function readHits(value: unknown, path: string): DebuggerHit[] {
  return readArray(value, path).map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    const hitIndex = readNumber(entry, 'index', entryPath)
    if (hitIndex !== index + 1) {
      throw new Error(
        `${entryPath}.index must be ${index + 1} (hits are 1-based and in execution order)`
      )
    }
    return {
      index: hitIndex,
      span: readSpan(entry.span),
      frames: readFrames(entry.frames, `${entryPath}.frames`),
      globals: readSlots(entry.globals, `${entryPath}.globals`),
      heap: readHeap(readRecord(entry, 'heap', entryPath), `${entryPath}.heap`),
    }
  })
}

export function parseDebuggerRunEnvelope(
  serialized: string
): DebuggerRunEnvelope {
  let value: unknown
  try {
    value = JSON.parse(serialized) as unknown
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new Error(`Debugger response is not valid JSON: ${message}`)
  }

  if (!isRecord(value)) {
    throw new Error('Debugger response must be an object')
  }

  const stdout = readString(value, 'stdout', 'response')
  const hits = readHits(value.hits, 'hits')
  const droppedHits = readNumber(value, 'droppedHits', 'response')

  if (value.status === 'ok') {
    return {
      status: 'ok',
      result: readString(value, 'result', 'response'),
      stdout,
      hits,
      droppedHits,
    }
  }

  if (value.status === 'error') {
    if (
      value.stage !== 'parse' &&
      value.stage !== 'compile' &&
      value.stage !== 'runtime'
    ) {
      throw new Error('stage must be parse, compile, or runtime')
    }
    return {
      status: 'error',
      stage: value.stage,
      kind: readString(value, 'kind', 'response'),
      message: readString(value, 'message', 'response'),
      span: readSpan(value.span),
      stdout,
      hits,
      droppedHits,
    }
  }

  throw new Error('Debugger response status must be ok or error')
}
