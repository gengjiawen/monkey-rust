import type {
  GcCollectionReport,
  ObjectDecision,
  RestorationWitness,
} from './gcReport'
import { formatEdgeRelation } from './gcReport'
import type { HeapGraphModel } from './heapGraph'
import { buildHeapGraphModel, renderHeapGraphModel } from './heapGraph'

/**
 * Node state as the replay steps through the report. `pending` objects have
 * not been classified yet and keep the default node style.
 */
export type ReplayNodeStatus =
  | 'pending'
  | 'candidate'
  | 'survivor'
  | 'restored'
  | 'garbage'
  | 'freed'

export type ReplayPhase =
  | 'Start'
  | 'Trial deletion'
  | 'Scan'
  | 'Free cycles'
  | 'Done'

export interface ReplayCounts {
  candidates: number
  restored: number
  garbage: number
  freed: number
}

export interface ReplayStep {
  phase: ReplayPhase
  caption: string
  statuses: ReadonlyMap<number, ReplayNodeStatus>
  /** Objects this step is about; they get the heavier `active` outline. */
  activeNodeIds: number[]
  /** mergedEdges indices drawn bold in this frame. */
  boldEdgeIndices: number[]
  /** mergedEdges indices drawn dimmed in this frame. */
  dimmedEdgeIndices: number[]
  counts: ReplayCounts
}

export type ReplayPlan =
  | { status: 'ok'; model: HeapGraphModel; steps: ReplayStep[] }
  | { status: 'unavailable'; reason: string }

const INCONSISTENT_REASON =
  'This report’s details do not fold into a consistent step-by-step replay; use the walkthrough table instead.'

function plural(count: number, word: string): string {
  return count === 1 ? word : `${word}s`
}

function listLabels(ids: number[], label: (id: number) => string): string {
  const shown = ids.slice(0, 3).map(label).join(', ')
  return ids.length <= 3 ? shown : `${shown} +${ids.length - 3} more`
}

/**
 * Fold a collection report into an ordered list of replay steps: one per
 * subtracted edge (trial deletion), one per restoration witness (scan), and
 * one per freed object (free cycles), with phase-boundary summaries between.
 *
 * The replay is a deterministic reconstruction from the report's sorted
 * edges and witness paths, not a recording of the collector's own traversal
 * order. It refuses to build (status `unavailable`) whenever the topology
 * graph is unavailable, when witnesses were truncated, or when the details
 * do not fold back into the reported per-object numbers.
 */
export function buildReplayPlan(report: GcCollectionReport): ReplayPlan {
  const modelResult = buildHeapGraphModel(report)
  if (modelResult.status === 'unavailable') {
    // Deliberately different copy from the topology card right above, which
    // already explains the underlying limit in detail.
    return {
      status: 'unavailable',
      reason:
        'The replay steps through the drawn heap topology, which is unavailable for this report; the topology card above explains why.',
    }
  }
  const { model } = modelResult
  const scan = report.phases.scan
  if (scan.omittedWitnesses > 0) {
    return {
      status: 'unavailable',
      reason:
        'This report truncated restoration witnesses, so the Scan phase cannot be replayed faithfully. Use the walkthrough table instead.',
    }
  }

  const inconsistent = (): ReplayPlan => ({
    status: 'unavailable',
    reason: INCONSISTENT_REASON,
  })

  const decisionOf = (id: number): ObjectDecision | undefined =>
    model.decisions.get(id)
  const label = (id: number): string => model.labels.get(id) ?? `Object#${id}`

  // Every drawn object needs a decision to seed and check the RC ledger.
  const rc = new Map<number, number>()
  for (const id of model.sortedIds) {
    const decision = decisionOf(id)
    if (!decision) {
      return inconsistent()
    }
    rc.set(id, decision.refCountBefore)
  }

  const candidateIds = model.sortedIds.filter(
    (id) => decisionOf(id)?.decision === 'candidate'
  )
  const survivorIds = model.sortedIds.filter(
    (id) => decisionOf(id)?.decision === 'survivor'
  )
  const restoredIds = candidateIds.filter(
    (id) => decisionOf(id)?.final === 'retained'
  )
  const garbageIds = candidateIds.filter(
    (id) => decisionOf(id)?.final === 'freed'
  )
  if (
    scan.restored !== restoredIds.length ||
    scan.garbageCandidates !== garbageIds.length ||
    report.phases.freeCycles.freed !== garbageIds.length
  ) {
    return inconsistent()
  }

  // Merged-edge index by "from->to" so trial and witness steps can point at
  // the exact arrow they narrate (linkStyle N addresses mergedEdges[N]).
  const edgeIndexByKey = new Map<string, number>()
  model.mergedEdges.forEach((edge, index) => {
    edgeIndexByKey.set(`${edge.fromId}->${edge.toId}`, index)
  })

  const witnessByObject = new Map<number, RestorationWitness>()
  for (const witness of scan.restorationWitnesses) {
    if (witnessByObject.has(witness.objectId)) {
      return inconsistent()
    }
    witnessByObject.set(witness.objectId, witness)
  }
  if (witnessByObject.size !== restoredIds.length) {
    return inconsistent()
  }
  for (const id of restoredIds) {
    const witness = witnessByObject.get(id)
    if (
      !witness ||
      !model.decisions.has(witness.rootId) ||
      !model.decisions.has(witness.predecessorId) ||
      !edgeIndexByKey.has(`${witness.predecessorId}->${witness.objectId}`)
    ) {
      return inconsistent()
    }
  }

  // Order restore steps parents-first: depth along the predecessor chain
  // (survivor predecessors sit at depth 0), ties by object id. Witnesses form
  // a forest; a predecessor cycle means the report is broken.
  const depths = new Map<number, number>()
  const resolveDepth = (id: number, trail: Set<number>): number | null => {
    const witness = witnessByObject.get(id)
    if (!witness) {
      return 0
    }
    const known = depths.get(id)
    if (known !== undefined) {
      return known
    }
    if (trail.has(id)) {
      return null
    }
    trail.add(id)
    const parent = resolveDepth(witness.predecessorId, trail)
    if (parent === null) {
      return null
    }
    depths.set(id, parent + 1)
    return parent + 1
  }
  for (const id of restoredIds) {
    if (resolveDepth(id, new Set()) === null) {
      return inconsistent()
    }
  }
  const orderedWitnesses = restoredIds
    .flatMap((id) => {
      const witness = witnessByObject.get(id)
      return witness ? [witness] : []
    })
    .sort(
      (left, right) =>
        (depths.get(left.objectId) ?? 0) - (depths.get(right.objectId) ?? 0) ||
        left.objectId - right.objectId
    )

  const statuses = new Map<number, ReplayNodeStatus>(
    model.sortedIds.map((id) => [id, 'pending' as ReplayNodeStatus])
  )
  const steps: ReplayStep[] = []
  const pushStep = (step: {
    phase: ReplayPhase
    caption: string
    activeNodeIds?: number[]
    boldEdgeIndices?: number[]
    dimmedEdgeIndices?: number[]
  }) => {
    const counts: ReplayCounts = { candidates: 0, restored: 0, garbage: 0, freed: 0 }
    for (const status of statuses.values()) {
      if (status === 'candidate') {
        counts.candidates += 1
      } else if (status === 'restored') {
        counts.restored += 1
      } else if (status === 'garbage') {
        counts.garbage += 1
      } else if (status === 'freed') {
        counts.freed += 1
      }
    }
    steps.push({
      phase: step.phase,
      caption: step.caption,
      statuses: new Map(statuses),
      activeNodeIds: step.activeNodeIds ?? [],
      boldEdgeIndices: step.boldEdgeIndices ?? [],
      dimmedEdgeIndices: step.dimmedEdgeIndices ?? [],
      counts,
    })
  }

  pushStep({
    phase: 'Start',
    caption: `Before the collector runs: ${model.sortedIds.length} drawn ${plural(
      model.sortedIds.length,
      'object'
    )} hold their current reference counts. Solid arrows are the heap-to-heap references Trial deletion will subtract.`,
  })

  // Phase 1 — one step per visited edge, in the report's sorted order.
  const consumedPerEdge = model.mergedEdges.map(() => 0)
  const dimmedEdges: number[] = []
  for (const edge of report.phases.trialDeletion.visitedEdges) {
    const mergedIndex = edgeIndexByKey.get(`${edge.fromId}->${edge.toId}`)
    const before = rc.get(edge.toId)
    if (mergedIndex === undefined || before === undefined || before <= 0) {
      return inconsistent()
    }
    const after = before - 1
    rc.set(edge.toId, after)
    consumedPerEdge[mergedIndex] += 1
    if (consumedPerEdge[mergedIndex] === model.mergedEdges[mergedIndex].labels.length) {
      dimmedEdges.push(mergedIndex)
    }
    let caption = `Trial deletion subtracts the ${formatEdgeRelation(
      edge.relation
    )} reference ${label(edge.fromId)} → ${label(edge.toId)}: RC ${before} → ${after}.`
    if (after === 0) {
      statuses.set(edge.toId, 'candidate')
      caption += ` ${label(edge.toId)} drops to RC 0 and becomes a Candidate (not yet known to be garbage).`
    }
    pushStep({
      phase: 'Trial deletion',
      caption,
      activeNodeIds: [edge.fromId, edge.toId],
      boldEdgeIndices: [mergedIndex],
      dimmedEdgeIndices: dimmedEdges.filter((index) => index !== mergedIndex),
    })
  }

  // The fold must land exactly on the reported trial RCs, and every reported
  // candidate must have been caught reaching zero along the way.
  for (const id of model.sortedIds) {
    if (rc.get(id) !== decisionOf(id)?.trialRefCount) {
      return inconsistent()
    }
  }
  for (const id of candidateIds) {
    if (statuses.get(id) !== 'candidate') {
      return inconsistent()
    }
  }

  for (const id of survivorIds) {
    statuses.set(id, 'survivor')
  }
  const allDimmed = [...dimmedEdges]

  if (candidateIds.length === 0) {
    pushStep({
      phase: 'Trial deletion',
      caption:
        'Trial deletion complete: every drawn object kept RC above 0, so there are no Candidates and nothing for Scan or Free cycles to do.',
      dimmedEdgeIndices: allDimmed,
    })
    pushStep({
      phase: 'Done',
      caption:
        'Collection complete: nothing was freed — every drawn object is retained.',
    })
    return { status: 'ok', model, steps }
  }

  pushStep({
    phase: 'Trial deletion',
    caption:
      `Trial deletion complete: ${candidateIds.length} ${plural(
        candidateIds.length,
        'Candidate'
      )} at RC 0` +
      (survivorIds.length > 0
        ? `; ${survivorIds.length} Trial ${plural(
            survivorIds.length,
            'survivor'
          )} ${survivorIds.length === 1 ? 'keeps' : 'keep'} a positive RC from non-heap references (dotted arrows).`
        : '; no drawn object kept a positive RC.'),
    activeNodeIds: survivorIds,
    dimmedEdgeIndices: allDimmed,
  })

  // Phase 2 — witness-ordered restores, then the garbage verdict.
  const boldWitnessEdges: number[] = []
  if (orderedWitnesses.length > 0) {
    pushStep({
      phase: 'Scan',
      caption: `Scan walks the Trial ${plural(
        survivorIds.length,
        'survivor'
      )} and gives back the references they hold; any Candidate reachable from a survivor is rescued.`,
      activeNodeIds: survivorIds,
    })
    for (const witness of orderedWitnesses) {
      const edgeIndex = edgeIndexByKey.get(
        `${witness.predecessorId}->${witness.objectId}`
      )
      if (edgeIndex === undefined) {
        return inconsistent()
      }
      statuses.set(witness.objectId, 'restored')
      boldWitnessEdges.push(edgeIndex)
      const via =
        witness.predecessorId === witness.rootId
          ? 'directly'
          : `through ${label(witness.predecessorId)}`
      pushStep({
        phase: 'Scan',
        caption: `Scan restores ${label(
          witness.objectId
        )}: the witness path from Trial survivor ${label(
          witness.rootId
        )} reaches it ${via} via ${formatEdgeRelation(
          witness.relation
        )}, so its RC climbs back above zero — it will be retained.`,
        activeNodeIds: [witness.predecessorId, witness.objectId],
        boldEdgeIndices: [...boldWitnessEdges],
      })
    }
  }

  if (garbageIds.length > 0) {
    for (const id of garbageIds) {
      statuses.set(id, 'garbage')
    }
    const listed = listLabels(garbageIds, label)
    pushStep({
      phase: 'Scan',
      caption:
        orderedWitnesses.length > 0
          ? `Scan complete: no Trial survivor reaches ${listed}, so ${
              garbageIds.length === 1 ? 'it stays' : 'they stay'
            } on the candidate list as garbage.`
          : `Scan rescues nothing: no Trial survivor reaches any Candidate, so ${listed} ${
              garbageIds.length === 1 ? 'stays' : 'stay'
            } on the candidate list as garbage.`,
      activeNodeIds: garbageIds,
      boldEdgeIndices: [...boldWitnessEdges],
    })
  } else {
    pushStep({
      phase: 'Scan',
      caption:
        'Scan complete: every Candidate was reachable from a Trial survivor; the candidate list is empty and Free cycles has nothing to free.',
      boldEdgeIndices: [...boldWitnessEdges],
    })
  }

  // Phase 3 — free the remaining candidates in id order.
  const freedEdgeDim = new Set<number>()
  for (const id of garbageIds) {
    statuses.set(id, 'freed')
    model.mergedEdges.forEach((edge, index) => {
      if (edge.fromId === id || edge.toId === id) {
        freedEdgeDim.add(index)
      }
    })
    pushStep({
      phase: 'Free cycles',
      caption: `Free cycles frees ${label(id)} and returns its memory to the allocator.`,
      activeNodeIds: [id],
      boldEdgeIndices: [...boldWitnessEdges],
      dimmedEdgeIndices: [...freedEdgeDim],
    })
  }

  const retained = survivorIds.length + restoredIds.length
  pushStep({
    phase: 'Done',
    caption:
      garbageIds.length > 0
        ? `Collection complete: ${garbageIds.length} ${plural(
            garbageIds.length,
            'object'
          )} freed, ${retained} drawn ${plural(retained, 'object')} retained.`
        : `Collection complete: nothing was freed — all ${retained} drawn objects are retained.`,
    boldEdgeIndices: [...boldWitnessEdges],
    dimmedEdgeIndices: [...freedEdgeDim],
  })

  return { status: 'ok', model, steps }
}

/**
 * Mermaid source for one replay frame. Every frame emits the identical node
 * and edge statements (same labels, same order) so mermaid computes the same
 * layout; frames differ only in ::: status classes, the `active` class list,
 * and theme-neutral linkStyle emphasis (bold width / dimmed opacity).
 */
export function replayStepSource(
  model: HeapGraphModel,
  step: ReplayStep
): string {
  const extraLines: string[] = []
  for (const index of step.dimmedEdgeIndices) {
    if (!step.boldEdgeIndices.includes(index)) {
      extraLines.push(`  linkStyle ${index} opacity:0.35`)
    }
  }
  for (const index of step.boldEdgeIndices) {
    extraLines.push(`  linkStyle ${index} stroke-width:3.5px`)
  }
  if (step.activeNodeIds.length > 0) {
    extraLines.push(
      `  class ${step.activeNodeIds.map((id) => `o${id}`).join(',')} active`
    )
  }
  return renderHeapGraphModel(
    model,
    (id) => {
      const status = step.statuses.get(id)
      return status && status !== 'pending' ? { className: status } : {}
    },
    extraLines
  )
}
