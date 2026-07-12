export const valueKinds = [
  'class',
  'instance',
  'boundMethod',
  'closure',
  'array',
  'hash',
  'integer',
  'boolean',
  'string',
  'null',
  'error',
  'compiledFunction',
  'builtin',
  'other',
] as const

export type ValueKind = (typeof valueKinds)[number]
export type ValueKindCounts = Record<ValueKind, number>

export const trialDecisions = ['candidate', 'survivor'] as const
export type TrialDecision = (typeof trialDecisions)[number]

export const finalFates = ['retained', 'freed'] as const
export type FinalFate = (typeof finalFates)[number]

export const edgeRelationKinds = [
  'arrayElement',
  'hashValue',
  'closureFunction',
  'closureFree',
  'classConstructor',
  'classMethod',
  'instanceClass',
  'instanceField',
  'boundMethodReceiver',
  'boundMethodFunction',
  'unknown',
] as const

export type EdgeRelationKind = (typeof edgeRelationKinds)[number]

export const hashKeyKinds = ['integer', 'boolean', 'string'] as const
export type HashKeyKind = (typeof hashKeyKinds)[number]

export type EdgeRelation =
  | { kind: 'arrayElement'; index: number }
  | { kind: 'hashValue'; keyKind: HashKeyKind; key: string }
  | { kind: 'closureFunction' }
  | { kind: 'closureFree'; index: number }
  | { kind: 'classConstructor' }
  | { kind: 'classMethod'; name: string }
  | { kind: 'instanceClass' }
  | { kind: 'instanceField'; name: string }
  | { kind: 'boundMethodReceiver' }
  | { kind: 'boundMethodFunction' }
  | { kind: 'unknown' }

export interface HeapSnapshot {
  objectCount: number
  trackedBytes: number
  byValueKind: ValueKindCounts
}

export interface ObjectDecision {
  objectId: number
  refCountBefore: number
  heapIncomingEdges: number
  trialRefCount: number
  decision: TrialDecision
  final: FinalFate
}

export interface VisitedEdge {
  fromId: number
  toId: number
  relation: EdgeRelation
}

export interface RestorationWitness {
  objectId: number
  rootId: number
  predecessorId: number
  relation: EdgeRelation
}

export interface TrialDeletionStats {
  edgesVisited: number
  candidates: number
  objectDecisions: ObjectDecision[]
  visitedEdges: VisitedEdge[]
  omittedObjectDecisions: number
  omittedEdgeDetails: number
}

export interface GcObjectSummary {
  id: number
  kind: ValueKind
  label: string
}

/**
 * A global variable name and the object its slot references at report time.
 * This is the named root set stated as a fact, not an alias guess.
 */
export interface GlobalRoot {
  name: string
  objectId: number
}

export interface ScanStats {
  restored: number
  garbageCandidates: number
  restoredObjects: GcObjectSummary[]
  garbageCandidateObjects: GcObjectSummary[]
  restorationWitnesses: RestorationWitness[]
  omittedWitnesses: number
}

export interface FreeCycleStats {
  freed: number
}

export interface GcCollectionReport {
  before: HeapSnapshot
  after: HeapSnapshot
  objects: GcObjectSummary[]
  globalRoots: GlobalRoot[]
  omittedGlobalRoots: number
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

export type ScanResultLabel = 'Restored' | 'Garbage' | 'Scan root'

export interface WitnessPathStep {
  fromId: number
  toId: number
  relation: EdgeRelation
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function readNumber(
  record: Record<string, unknown>,
  key: string,
  path: string
): number {
  const value = record[key]
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${path}.${key} must be a non-negative safe integer`)
  }
  return value
}

function readObjectId(
  record: Record<string, unknown>,
  key: string,
  path: string
): number {
  return readNumber(record, key, path)
}

function readCatalogId(
  record: Record<string, unknown>,
  key: string,
  path: string,
  catalog: Set<number>
): number {
  const id = readObjectId(record, key, path)
  if (!catalog.has(id)) {
    throw new Error(`${path}.${key} references unknown object ${id}`)
  }
  return id
}

function readString(
  record: Record<string, unknown>,
  key: string,
  path: string
): string {
  const value = record[key]
  if (typeof value !== 'string') {
    throw new Error(`${path}.${key} must be a string`)
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

function readObjectSummaries(value: unknown, path: string): GcObjectSummary[] {
  if (!Array.isArray(value)) {
    throw new Error(`${path} must be an array`)
  }

  return value.map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    if (!valueKinds.includes(entry.kind as ValueKind)) {
      throw new Error(`${entryPath}.kind must be a known value kind`)
    }
    return {
      id: readObjectId(entry, 'id', entryPath),
      kind: entry.kind as ValueKind,
      label: readString(entry, 'label', entryPath),
    }
  })
}

function readObjectSummariesInCatalog(
  value: unknown,
  path: string,
  catalog: Set<number>
): GcObjectSummary[] {
  const summaries = readObjectSummaries(value, path)
  for (const [index, summary] of summaries.entries()) {
    if (!catalog.has(summary.id)) {
      throw new Error(
        `${path}[${index}].id references unknown object ${summary.id}`
      )
    }
  }
  return summaries
}

function readGlobalRoots(
  value: unknown,
  path: string,
  catalog: Set<number>
): GlobalRoot[] {
  if (!Array.isArray(value)) {
    throw new Error(`${path} must be an array`)
  }

  const names = new Set<string>()
  return value.map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    const name = readString(entry, 'name', entryPath)
    if (names.has(name)) {
      throw new Error(`${path} must not contain duplicate names`)
    }
    names.add(name)
    return {
      name,
      objectId: readCatalogId(entry, 'objectId', entryPath, catalog),
    }
  })
}

function uniqueIds<T>(
  values: readonly T[],
  idOf: (value: T) => number,
  path: string,
  field: string
): Set<number> {
  const ids = new Set<number>()
  for (const value of values) {
    const id = idOf(value)
    if (ids.has(id)) {
      throw new Error(`${path} must not contain duplicate ${field} values`)
    }
    ids.add(id)
  }
  return ids
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

/** Format a typed edge relation for teaching UI display. */
export function formatEdgeRelation(relation: EdgeRelation): string {
  switch (relation.kind) {
    case 'arrayElement':
      return `items[${relation.index}]`
    case 'hashValue':
      return relation.keyKind === 'string'
        ? `values["${relation.key}"]`
        : `values[${relation.key}]`
    case 'closureFunction':
      return 'function'
    case 'closureFree':
      return `free[${relation.index}]`
    case 'classConstructor':
      return 'constructor'
    case 'classMethod':
      return `methods["${relation.name}"]`
    case 'instanceClass':
      return 'class'
    case 'instanceField':
      return `fields["${relation.name}"]`
    case 'boundMethodReceiver':
      return 'receiver'
    case 'boundMethodFunction':
      return 'method'
    case 'unknown':
      return 'unknown'
  }
}

/** Map an object decision to the Scan result column label. */
export function scanResultLabel(
  decision: ObjectDecision,
  restoredIds: ReadonlySet<number>,
  garbageIds: ReadonlySet<number>
): ScanResultLabel {
  const restored = restoredIds.has(decision.objectId)
  const garbage = garbageIds.has(decision.objectId)
  if (restored && garbage) {
    throw new Error(
      `object ${decision.objectId} cannot be both restored and garbage`
    )
  }
  if (decision.decision === 'survivor') {
    if (restored || garbage) {
      throw new Error(
        `survivor object ${decision.objectId} cannot appear in Scan candidate results`
      )
    }
    return 'Scan root'
  }
  if (restored) {
    return 'Restored'
  }
  if (garbage) {
    return 'Garbage'
  }
  throw new Error(
    `candidate object ${decision.objectId} is missing from Scan candidate results`
  )
}

/**
 * Rebuild a reachability path from a restoration witness forest entry.
 * Returns steps from the trial survivor toward the restored candidate.
 */
export function rebuildWitnessPath(
  witnesses: readonly RestorationWitness[],
  objectId: number
): WitnessPathStep[] | null {
  const byObjectId = new Map(
    witnesses.map((witness) => [witness.objectId, witness])
  )
  const start = byObjectId.get(objectId)
  if (!start) {
    return null
  }

  const steps: WitnessPathStep[] = []
  let current = objectId
  const seen = new Set<number>()

  while (current !== start.rootId) {
    if (seen.has(current)) {
      return null
    }
    seen.add(current)
    const entry = byObjectId.get(current)
    if (!entry) {
      return null
    }
    steps.push({
      fromId: entry.predecessorId,
      toId: current,
      relation: entry.relation,
    })
    current = entry.predecessorId
  }

  steps.reverse()
  return steps
}

export function isCandidateRelatedEdge(
  edge: VisitedEdge,
  candidateIds: ReadonlySet<number>
): boolean {
  return candidateIds.has(edge.fromId) || candidateIds.has(edge.toId)
}

function readEdgeRelation(value: unknown, path: string): EdgeRelation {
  if (!isRecord(value)) {
    throw new Error(`${path} must be an object`)
  }
  const kind = value.kind
  if (
    typeof kind !== 'string' ||
    !edgeRelationKinds.includes(kind as EdgeRelationKind)
  ) {
    throw new Error(`${path}.kind must be a known edge relation kind`)
  }

  switch (kind as EdgeRelationKind) {
    case 'arrayElement': {
      const index = readNumber(value, 'index', path)
      if (!Number.isSafeInteger(index)) {
        throw new Error(`${path}.index must be a non-negative safe integer`)
      }
      return { kind: 'arrayElement', index }
    }
    case 'hashValue': {
      const keyKind = value.keyKind
      if (
        typeof keyKind !== 'string' ||
        !hashKeyKinds.includes(keyKind as HashKeyKind)
      ) {
        throw new Error(`${path}.keyKind must be integer, boolean, or string`)
      }
      return {
        kind: 'hashValue',
        keyKind: keyKind as HashKeyKind,
        key: readString(value, 'key', path),
      }
    }
    case 'closureFunction':
      return { kind: 'closureFunction' }
    case 'closureFree': {
      const index = readNumber(value, 'index', path)
      if (!Number.isSafeInteger(index)) {
        throw new Error(`${path}.index must be a non-negative safe integer`)
      }
      return { kind: 'closureFree', index }
    }
    case 'classConstructor':
      return { kind: 'classConstructor' }
    case 'classMethod':
      return { kind: 'classMethod', name: readString(value, 'name', path) }
    case 'instanceClass':
      return { kind: 'instanceClass' }
    case 'instanceField':
      return { kind: 'instanceField', name: readString(value, 'name', path) }
    case 'boundMethodReceiver':
      return { kind: 'boundMethodReceiver' }
    case 'boundMethodFunction':
      return { kind: 'boundMethodFunction' }
    case 'unknown':
      return { kind: 'unknown' }
  }
}

function readObjectDecisions(
  value: unknown,
  path: string,
  catalog: Set<number>,
  garbageIds: Set<number>
): ObjectDecision[] {
  if (!Array.isArray(value)) {
    throw new Error(`${path} must be an array`)
  }

  return value.map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }

    const objectId = readCatalogId(entry, 'objectId', entryPath, catalog)
    const decisionValue = entry.decision
    if (
      typeof decisionValue !== 'string' ||
      !trialDecisions.includes(decisionValue as TrialDecision)
    ) {
      throw new Error(`${entryPath}.decision must be candidate or survivor`)
    }
    const decision = decisionValue as TrialDecision

    const finalValue = entry.final
    if (
      typeof finalValue !== 'string' ||
      !finalFates.includes(finalValue as FinalFate)
    ) {
      throw new Error(`${entryPath}.final must be retained or freed`)
    }
    const finalFate = finalValue as FinalFate

    const refCountBefore = readNumber(entry, 'refCountBefore', entryPath)
    const heapIncomingEdges = readNumber(entry, 'heapIncomingEdges', entryPath)
    const trialRefCount = readNumber(entry, 'trialRefCount', entryPath)
    if (refCountBefore - heapIncomingEdges !== trialRefCount) {
      throw new Error(
        `${entryPath}.trialRefCount must equal refCountBefore - heapIncomingEdges`
      )
    }
    if ((decision === 'candidate') !== (trialRefCount === 0)) {
      throw new Error(
        `${entryPath}.decision must be candidate iff trialRefCount is zero`
      )
    }

    const expectFreed = decision === 'candidate' && garbageIds.has(objectId)
    if ((finalFate === 'freed') !== expectFreed) {
      throw new Error(
        `${entryPath}.final must be freed iff decision is candidate and object is a garbage candidate`
      )
    }

    return {
      objectId,
      refCountBefore,
      heapIncomingEdges,
      trialRefCount,
      decision,
      final: finalFate,
    }
  })
}

function readVisitedEdges(
  value: unknown,
  path: string,
  catalog: Set<number>
): VisitedEdge[] {
  if (!Array.isArray(value)) {
    throw new Error(`${path} must be an array`)
  }

  return value.map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    return {
      fromId: readCatalogId(entry, 'fromId', entryPath, catalog),
      toId: readCatalogId(entry, 'toId', entryPath, catalog),
      relation: readEdgeRelation(entry.relation, `${entryPath}.relation`),
    }
  })
}

function readRestorationWitnesses(
  value: unknown,
  path: string,
  catalog: Set<number>
): RestorationWitness[] {
  if (!Array.isArray(value)) {
    throw new Error(`${path} must be an array`)
  }

  return value.map((entry, index) => {
    const entryPath = `${path}[${index}]`
    if (!isRecord(entry)) {
      throw new Error(`${entryPath} must be an object`)
    }
    return {
      objectId: readCatalogId(entry, 'objectId', entryPath, catalog),
      rootId: readCatalogId(entry, 'rootId', entryPath, catalog),
      predecessorId: readCatalogId(entry, 'predecessorId', entryPath, catalog),
      relation: readEdgeRelation(entry.relation, `${entryPath}.relation`),
    }
  })
}

function validateWitnessForest(
  witnesses: RestorationWitness[],
  decisionsById: Map<number, ObjectDecision>,
  path: string
): void {
  const witnessObjectIds = new Set(witnesses.map((witness) => witness.objectId))
  const witnessesById = new Map(
    witnesses.map((witness) => [witness.objectId, witness])
  )

  for (const [index, witness] of witnesses.entries()) {
    const entryPath = `${path}[${index}]`
    if (witnessObjectIds.has(witness.rootId)) {
      throw new Error(
        `${entryPath}.rootId must not appear as an objectId in the witness forest`
      )
    }

    let current = witness.objectId
    const seen = new Set<number>()

    while (true) {
      if (seen.has(current)) {
        throw new Error(`${entryPath} witness chain contains a cycle`)
      }
      seen.add(current)

      const decision = decisionsById.get(current)
      if (!decision) {
        throw new Error(
          `${entryPath} witness chain references object ${current} without a decision`
        )
      }

      if (current === witness.rootId) {
        if (decision.decision !== 'survivor') {
          throw new Error(
            `${entryPath} witness chain must end at a survivor decision`
          )
        }
        break
      }

      if (decision.decision !== 'candidate') {
        throw new Error(
          `${entryPath} witness chain intermediate nodes must be candidates`
        )
      }

      const entry = witnessesById.get(current)
      if (!entry) {
        throw new Error(
          `${entryPath} witness chain is missing an entry for object ${current}`
        )
      }
      current = entry.predecessorId
    }
  }
}

function readReport(value: unknown): GcCollectionReport {
  if (!isRecord(value)) {
    throw new Error('report must be an object')
  }

  const objects = readObjectSummaries(value.objects, 'report.objects')
  const catalog = new Set(objects.map((object) => object.id))
  if (catalog.size !== objects.length) {
    throw new Error('report.objects must not contain duplicate ids')
  }

  const globalRoots = readGlobalRoots(
    value.globalRoots,
    'report.globalRoots',
    catalog
  )
  const omittedGlobalRoots = readNumber(value, 'omittedGlobalRoots', 'report')

  const phases = readRecord(value, 'phases', 'report')
  const trialDeletion = readRecord(phases, 'trialDeletion', 'report.phases')
  const scan = readRecord(phases, 'scan', 'report.phases')
  const freeCycles = readRecord(phases, 'freeCycles', 'report.phases')

  const restoredObjects = readObjectSummariesInCatalog(
    scan.restoredObjects,
    'report.phases.scan.restoredObjects',
    catalog
  )
  const garbageCandidateObjects = readObjectSummariesInCatalog(
    scan.garbageCandidateObjects,
    'report.phases.scan.garbageCandidateObjects',
    catalog
  )
  const restored = readNumber(scan, 'restored', 'report.phases.scan')
  const garbageCandidates = readNumber(
    scan,
    'garbageCandidates',
    'report.phases.scan'
  )
  if (restoredObjects.length !== restored) {
    throw new Error(
      'report.phases.scan.restoredObjects.length must equal restored'
    )
  }
  if (garbageCandidateObjects.length !== garbageCandidates) {
    throw new Error(
      'report.phases.scan.garbageCandidateObjects.length must equal garbageCandidates'
    )
  }

  const restoredIds = uniqueIds(
    restoredObjects,
    (object) => object.id,
    'report.phases.scan.restoredObjects',
    'id'
  )
  const garbageIds = uniqueIds(
    garbageCandidateObjects,
    (object) => object.id,
    'report.phases.scan.garbageCandidateObjects',
    'id'
  )
  for (const id of restoredIds) {
    if (garbageIds.has(id)) {
      throw new Error(
        'report.phases.scan restored and garbage candidate objects must be disjoint'
      )
    }
  }

  const candidates = readNumber(
    trialDeletion,
    'candidates',
    'report.phases.trialDeletion'
  )
  if (candidates !== restored + garbageCandidates) {
    throw new Error(
      'report.phases.trialDeletion.candidates must equal scan.restored + scan.garbageCandidates'
    )
  }
  const candidateIds = new Set([...restoredIds, ...garbageIds])

  const objectDecisions = readObjectDecisions(
    trialDeletion.objectDecisions,
    'report.phases.trialDeletion.objectDecisions',
    catalog,
    garbageIds
  )
  uniqueIds(
    objectDecisions,
    (decision) => decision.objectId,
    'report.phases.trialDeletion.objectDecisions',
    'objectId'
  )
  for (const [index, decision] of objectDecisions.entries()) {
    const isCandidate = candidateIds.has(decision.objectId)
    if ((decision.decision === 'candidate') !== isCandidate) {
      throw new Error(
        `report.phases.trialDeletion.objectDecisions[${index}].decision must be candidate iff the object appears in Scan candidate results`
      )
    }
  }

  const visitedEdges = readVisitedEdges(
    trialDeletion.visitedEdges,
    'report.phases.trialDeletion.visitedEdges',
    catalog
  )
  const restorationWitnesses = readRestorationWitnesses(
    scan.restorationWitnesses,
    'report.phases.scan.restorationWitnesses',
    catalog
  )
  uniqueIds(
    restorationWitnesses,
    (witness) => witness.objectId,
    'report.phases.scan.restorationWitnesses',
    'objectId'
  )
  for (const [index, witness] of restorationWitnesses.entries()) {
    if (!restoredIds.has(witness.objectId)) {
      throw new Error(
        `report.phases.scan.restorationWitnesses[${index}].objectId must reference a restored object`
      )
    }
  }

  const edgesVisited = readNumber(
    trialDeletion,
    'edgesVisited',
    'report.phases.trialDeletion'
  )
  const omittedEdgeDetails = readNumber(
    trialDeletion,
    'omittedEdgeDetails',
    'report.phases.trialDeletion'
  )
  if (edgesVisited !== visitedEdges.length + omittedEdgeDetails) {
    throw new Error(
      'report.phases.trialDeletion.edgesVisited must equal visitedEdges.length + omittedEdgeDetails'
    )
  }

  const omittedWitnesses = readNumber(
    scan,
    'omittedWitnesses',
    'report.phases.scan'
  )
  if (restored !== restorationWitnesses.length + omittedWitnesses) {
    throw new Error(
      'report.phases.scan.restored must equal restorationWitnesses.length + omittedWitnesses'
    )
  }

  const decisionsById = new Map(
    objectDecisions.map((decision) => [decision.objectId, decision])
  )
  for (const root of globalRoots) {
    const decision = decisionsById.get(root.objectId)
    if (decision && decision.decision !== 'survivor') {
      throw new Error(
        `report.globalRoots names candidate object ${root.objectId}; a named global slot is a non-heap reference, so the object must be a trial survivor`
      )
    }
  }
  validateWitnessForest(
    restorationWitnesses,
    decisionsById,
    'report.phases.scan.restorationWitnesses'
  )

  return {
    before: readSnapshot(value.before, 'report.before'),
    after: readSnapshot(value.after, 'report.after'),
    objects,
    globalRoots,
    omittedGlobalRoots,
    phases: {
      trialDeletion: {
        edgesVisited,
        candidates,
        objectDecisions,
        visitedEdges,
        omittedObjectDecisions: readNumber(
          trialDeletion,
          'omittedObjectDecisions',
          'report.phases.trialDeletion'
        ),
        omittedEdgeDetails,
      },
      scan: {
        restored,
        garbageCandidates,
        restoredObjects,
        garbageCandidateObjects,
        restorationWitnesses,
        omittedWitnesses,
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
