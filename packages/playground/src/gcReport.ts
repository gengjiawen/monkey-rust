export const valueKinds = [
  'class',
  'instance',
  'boundMethod',
  'closure',
  'array',
  'hash',
  'other',
] as const

export type ValueKind = (typeof valueKinds)[number]
export type ValueKindCounts = Record<ValueKind, number>

export interface HeapSnapshot {
  objectCount: number
  trackedBytes: number
  byValueKind: ValueKindCounts
}

export interface TrialDeletionStats {
  edgesVisited: number
  candidates: number
}

export interface ScanStats {
  restored: number
  garbageCandidates: number
}

export interface FreeCycleStats {
  freed: number
}

export interface GcCollectionReport {
  before: HeapSnapshot
  after: HeapSnapshot
  phases: {
    trialDeletion: TrialDeletionStats
    scan: ScanStats
    freeCycles: FreeCycleStats
  }
  collectedByValueKind: ValueKindCounts
}

export interface GcRunSuccess {
  status: 'ok'
  result: string
  report: GcCollectionReport
}

export type GcRunStage = 'parse' | 'compile' | 'runtime'

export interface SourceSpan {
  start: number
  end: number
}

export interface GcRunError {
  status: 'error'
  stage: GcRunStage
  message: string
  span: SourceSpan | null
}

export type GcRunEnvelope = GcRunSuccess | GcRunError

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function readNumber(
  record: Record<string, unknown>,
  key: string,
  path: string
): number {
  const value = record[key]
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) {
    throw new Error(`${path}.${key} must be a non-negative number`)
  }
  return value
}

function readRecord(
  record: Record<string, unknown>,
  key: string,
  path: string
): Record<string, unknown> {
  const value = record[key]
  if (!isRecord(value)) {
    throw new Error(`${path}.${key} must be an object`)
  }
  return value
}

function readValueKindCounts(value: unknown, path: string): ValueKindCounts {
  if (!isRecord(value)) {
    throw new Error(`${path} must be an object`)
  }

  return Object.fromEntries(
    valueKinds.map((kind) => [kind, readNumber(value, kind, path)])
  ) as ValueKindCounts
}

function readSnapshot(value: unknown, path: string): HeapSnapshot {
  if (!isRecord(value)) {
    throw new Error(`${path} must be an object`)
  }

  return {
    objectCount: readNumber(value, 'objectCount', path),
    trackedBytes: readNumber(value, 'trackedBytes', path),
    byValueKind: readValueKindCounts(value.byValueKind, `${path}.byValueKind`),
  }
}

function readReport(value: unknown): GcCollectionReport {
  if (!isRecord(value)) {
    throw new Error('report must be an object')
  }

  const phases = readRecord(value, 'phases', 'report')
  const trialDeletion = readRecord(phases, 'trialDeletion', 'report.phases')
  const scan = readRecord(phases, 'scan', 'report.phases')
  const freeCycles = readRecord(phases, 'freeCycles', 'report.phases')

  return {
    before: readSnapshot(value.before, 'report.before'),
    after: readSnapshot(value.after, 'report.after'),
    phases: {
      trialDeletion: {
        edgesVisited: readNumber(
          trialDeletion,
          'edgesVisited',
          'report.phases.trialDeletion'
        ),
        candidates: readNumber(
          trialDeletion,
          'candidates',
          'report.phases.trialDeletion'
        ),
      },
      scan: {
        restored: readNumber(scan, 'restored', 'report.phases.scan'),
        garbageCandidates: readNumber(
          scan,
          'garbageCandidates',
          'report.phases.scan'
        ),
      },
      freeCycles: {
        freed: readNumber(freeCycles, 'freed', 'report.phases.freeCycles'),
      },
    },
    collectedByValueKind: readValueKindCounts(
      value.collectedByValueKind,
      'report.collectedByValueKind'
    ),
  }
}

function readSpan(value: unknown): SourceSpan | null {
  if (value === null) {
    return null
  }
  if (!isRecord(value)) {
    throw new Error('span must be an object or null')
  }

  return {
    start: readNumber(value, 'start', 'span'),
    end: readNumber(value, 'end', 'span'),
  }
}

export function parseGcRunEnvelope(serialized: string): GcRunEnvelope {
  let value: unknown
  try {
    value = JSON.parse(serialized) as unknown
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new Error(`GC response is not valid JSON: ${message}`)
  }

  if (!isRecord(value)) {
    throw new Error('GC response must be an object')
  }

  if (value.status === 'ok') {
    if (typeof value.result !== 'string') {
      throw new Error('result must be a string')
    }
    return {
      status: 'ok',
      result: value.result,
      report: readReport(value.report),
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
    if (typeof value.message !== 'string') {
      throw new Error('message must be a string')
    }
    return {
      status: 'error',
      stage: value.stage,
      message: value.message,
      span: readSpan(value.span),
    }
  }

  throw new Error('GC response status must be ok or error')
}
