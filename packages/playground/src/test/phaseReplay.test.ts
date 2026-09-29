import { describe, expect, it } from 'vitest'

import type {
  GcCollectionReport,
  GcObjectSummary,
  GlobalRoot,
  ObjectDecision,
  RestorationWitness,
  ValueKindCounts,
  VisitedEdge,
} from '../gcReport'
import { buildReplayPlan, replayStepSource } from '../phaseReplay'

const counts = (): ValueKindCounts => ({
  class: 0,
  instance: 0,
  boundMethod: 0,
  closure: 0,
  array: 0,
  hash: 0,
  integer: 0,
  boolean: 0,
  string: 0,
  null: 0,
  error: 0,
  compiledFunction: 0,
  builtin: 0,
  other: 0,
})

function survivor(objectId: number, trialRefCount = 1): ObjectDecision {
  return {
    objectId,
    refCountBefore: trialRefCount,
    heapIncomingEdges: 0,
    trialRefCount,
    decision: 'survivor',
    final: 'retained',
  }
}

function candidate(
  objectId: number,
  final: 'retained' | 'freed'
): ObjectDecision {
  return {
    objectId,
    refCountBefore: 1,
    heapIncomingEdges: 1,
    trialRefCount: 0,
    decision: 'candidate',
    final,
  }
}

function makeReport(options: {
  objects: GcObjectSummary[]
  decisions: ObjectDecision[]
  edges?: VisitedEdge[]
  globalRoots?: GlobalRoot[]
  witnesses?: RestorationWitness[]
  omittedEdgeDetails?: number
  omittedObjectDecisions?: number
  omittedWitnesses?: number
}): GcCollectionReport {
  const edges = options.edges ?? []
  const restoredObjects = options.objects.filter((object) =>
    options.decisions.some(
      (decision) =>
        decision.objectId === object.id &&
        decision.decision === 'candidate' &&
        decision.final === 'retained'
    )
  )
  const garbageObjects = options.objects.filter((object) =>
    options.decisions.some(
      (decision) => decision.objectId === object.id && decision.final === 'freed'
    )
  )
  const snapshot = {
    objectCount: options.objects.length,
    trackedBytes: 0,
    byValueKind: counts(),
  }
  return {
    before: snapshot,
    after: snapshot,
    objects: options.objects,
    globalRoots: options.globalRoots ?? [],
    omittedGlobalRoots: 0,
    phases: {
      trialDeletion: {
        edgesVisited: edges.length + (options.omittedEdgeDetails ?? 0),
        candidates: restoredObjects.length + garbageObjects.length,
        objectDecisions: options.decisions,
        visitedEdges: edges,
        omittedObjectDecisions: options.omittedObjectDecisions ?? 0,
        omittedEdgeDetails: options.omittedEdgeDetails ?? 0,
      },
      scan: {
        restored: restoredObjects.length,
        garbageCandidates: garbageObjects.length,
        restoredObjects,
        garbageCandidateObjects: garbageObjects,
        restorationWitnesses: options.witnesses ?? [],
        omittedWitnesses: options.omittedWitnesses ?? 0,
      },
      freeCycles: { freed: garbageObjects.length },
    },
    collectedByValueKind: counts(),
  }
}

/** The pure garbage cycle: 20 ↔ 21, both freed. */
function cycleReport(options: {
  omittedEdgeDetails?: number
  omittedWitnesses?: number
} = {}): GcCollectionReport {
  return makeReport({
    objects: [20, 21].map((id) => ({
      id,
      kind: 'instance' as const,
      label: `Instance(Node)#${id}`,
    })),
    decisions: [candidate(20, 'freed'), candidate(21, 'freed')],
    edges: [
      { fromId: 20, toId: 21, relation: { kind: 'instanceField', name: 'next' } },
      { fromId: 21, toId: 20, relation: { kind: 'instanceField', name: 'next' } },
    ],
    ...options,
  })
}

/** A rooted chain: survivor Array#1 → Array#2 → Array#3, both restored. */
function rootedChainReport(options: {
  witnesses?: RestorationWitness[]
  omittedWitnesses?: number
} = {}): GcCollectionReport {
  return makeReport({
    objects: [1, 2, 3].map((id) => ({
      id,
      kind: 'array' as const,
      label: `Array#${id}`,
    })),
    decisions: [
      survivor(1, 1),
      candidate(2, 'retained'),
      candidate(3, 'retained'),
    ],
    edges: [
      { fromId: 1, toId: 2, relation: { kind: 'arrayElement', index: 0 } },
      { fromId: 2, toId: 3, relation: { kind: 'arrayElement', index: 1 } },
    ],
    witnesses: options.witnesses ?? [
      {
        objectId: 2,
        rootId: 1,
        predecessorId: 1,
        relation: { kind: 'arrayElement', index: 0 },
      },
      {
        objectId: 3,
        rootId: 1,
        predecessorId: 2,
        relation: { kind: 'arrayElement', index: 1 },
      },
    ],
    omittedWitnesses: options.omittedWitnesses,
  })
}

function okPlan(report: GcCollectionReport) {
  const plan = buildReplayPlan(report)
  if (plan.status !== 'ok') {
    throw new Error(`expected an ok plan, got: ${plan.reason}`)
  }
  return plan
}

function stripDecorations(source: string): string {
  return source
    .split('\n')
    .filter(
      (line) => !line.startsWith('  linkStyle ') && !line.startsWith('  class ')
    )
    .map((line) => line.replace(/:::\w+$/, ''))
    .join('\n')
}

describe('buildReplayPlan', () => {
  it('replays a pure cycle: subtract, condemn, free', () => {
    const plan = okPlan(cycleReport())
    const captions = plan.steps.map((step) => step.caption)

    expect(plan.steps).toHaveLength(8)
    expect(plan.steps.map((step) => step.phase)).toEqual([
      'Start',
      'Trial deletion',
      'Trial deletion',
      'Trial deletion',
      'Scan',
      'Free cycles',
      'Free cycles',
      'Done',
    ])

    expect(captions[1]).toContain(
      'Trial deletion subtracts the fields["next"] reference Instance(Node)#20 → Instance(Node)#21: RC 1 → 0.'
    )
    expect(captions[1]).toContain('becomes a Candidate')
    expect(plan.steps[1].statuses.get(21)).toBe('candidate')
    expect(plan.steps[1].statuses.get(20)).toBe('pending')
    expect(plan.steps[1].boldEdgeIndices).toEqual([0])

    expect(plan.steps[2].statuses.get(20)).toBe('candidate')
    expect(plan.steps[2].dimmedEdgeIndices).toEqual([0])

    expect(captions[3]).toContain('Trial deletion complete: 2 Candidates')
    expect(captions[3]).toContain('no drawn object kept a positive RC')

    expect(captions[4]).toContain('Scan rescues nothing')
    expect(plan.steps[4].statuses.get(20)).toBe('garbage')
    expect(plan.steps[4].counts.garbage).toBe(2)

    expect(captions[5]).toContain('Free cycles frees Instance(Node)#20')
    expect(plan.steps[5].statuses.get(20)).toBe('freed')
    expect(plan.steps[5].statuses.get(21)).toBe('garbage')

    expect(captions[7]).toContain('2 objects freed')
    expect(plan.steps[7].counts).toEqual({
      candidates: 0,
      restored: 0,
      garbage: 0,
      freed: 2,
    })
  })

  it('replays a rooted chain with parents-first witness restores', () => {
    const plan = okPlan(rootedChainReport())
    const captions = plan.steps.map((step) => step.caption)

    expect(plan.steps).toHaveLength(9)
    expect(captions[3]).toContain(
      '2 Candidates at RC 0; 1 Trial survivor keeps a positive RC'
    )
    expect(captions[4]).toContain('Scan walks the Trial survivor')

    // Array#2 (depth 1) restores before Array#3 (depth 2).
    expect(captions[5]).toContain(
      'Scan restores Array#2: the witness path from Trial survivor Array#1 reaches it directly via items[0]'
    )
    expect(captions[6]).toContain(
      'Scan restores Array#3: the witness path from Trial survivor Array#1 reaches it through Array#2 via items[1]'
    )
    expect(plan.steps[6].boldEdgeIndices).toEqual([0, 1])

    expect(captions[7]).toContain('the candidate list is empty')
    expect(captions[8]).toContain('nothing was freed')
    expect(plan.steps[8].counts).toEqual({
      candidates: 0,
      restored: 2,
      garbage: 0,
      freed: 0,
    })
  })

  it('replays a survivor-only heap without a scan or free phase', () => {
    const plan = okPlan(
      makeReport({
        objects: [
          { id: 1, kind: 'array', label: 'Array#1' },
          { id: 5, kind: 'instance', label: 'Instance(Node)#5' },
        ],
        decisions: [
          survivor(1, 1),
          {
            objectId: 5,
            refCountBefore: 2,
            heapIncomingEdges: 1,
            trialRefCount: 1,
            decision: 'survivor',
            final: 'retained',
          },
        ],
        edges: [
          { fromId: 1, toId: 5, relation: { kind: 'arrayElement', index: 0 } },
        ],
      })
    )

    expect(plan.steps.map((step) => step.phase)).toEqual([
      'Start',
      'Trial deletion',
      'Trial deletion',
      'Done',
    ])
    expect(plan.steps[1].caption).toContain('RC 2 → 1')
    expect(plan.steps[1].caption).not.toContain('Candidate')
    expect(plan.steps[2].caption).toContain('there are no Candidates')
    expect(plan.steps[3].caption).toContain('nothing was freed')
  })

  it('emits identical node and edge statements for every frame', () => {
    for (const report of [cycleReport(), rootedChainReport()]) {
      const plan = okPlan(report)
      const sources = plan.steps.map((step) =>
        replayStepSource(plan.model, step)
      )
      const baseline = stripDecorations(sources[0])
      for (const source of sources) {
        expect(stripDecorations(source)).toBe(baseline)
      }
    }
  })

  it('styles frames through classes and linkStyle only', () => {
    const plan = okPlan(cycleReport())

    const firstTrial = replayStepSource(plan.model, plan.steps[1])
    expect(firstTrial).toContain('o21["Instance(Node)#35;21"]:::candidate')
    expect(firstTrial).toContain('  linkStyle 0 stroke-width:3.5px')
    expect(firstTrial).toContain('  class o20,o21 active')

    const secondTrial = replayStepSource(plan.model, plan.steps[2])
    expect(secondTrial).toContain('  linkStyle 0 opacity:0.35')
    expect(secondTrial).toContain('  linkStyle 1 stroke-width:3.5px')

    const done = replayStepSource(plan.model, plan.steps[7])
    expect(done).toContain('o20["Instance(Node)#35;20"]:::freed')
    expect(done).toContain('o21["Instance(Node)#35;21"]:::freed')
  })

  it('is unavailable when the topology graph is unavailable', () => {
    const plan = buildReplayPlan(cycleReport({ omittedEdgeDetails: 1 }))
    expect(plan.status).toBe('unavailable')
    if (plan.status === 'unavailable') {
      expect(plan.reason).toMatch(/topology card above explains why/)
    }
  })

  it('is unavailable when restoration witnesses were truncated', () => {
    const plan = buildReplayPlan(rootedChainReport({ omittedWitnesses: 1 }))
    expect(plan.status).toBe('unavailable')
    if (plan.status === 'unavailable') {
      expect(plan.reason).toMatch(/truncated restoration witnesses/)
    }
  })

  it('is unavailable when a restored object has no witness', () => {
    const plan = buildReplayPlan(rootedChainReport({ witnesses: [] }))
    expect(plan.status).toBe('unavailable')
    if (plan.status === 'unavailable') {
      expect(plan.reason).toMatch(/step-by-step replay/)
    }
  })
})
