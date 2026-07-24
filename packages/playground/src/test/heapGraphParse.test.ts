// Parses generator output with the real mermaid parser (not a mock) so the
// emitted syntax — entities, edge labels, ::: class tags — stays valid.
import mermaid from 'mermaid'
import { describe, expect, it } from 'vitest'

import type {
  GcCollectionReport,
  ObjectDecision,
  ValueKindCounts,
} from '../gcReport'
import { buildHeapGraph } from '../heapGraph'
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

function report(): GcCollectionReport {
  const decisions: ObjectDecision[] = [
    {
      objectId: 1,
      refCountBefore: 2,
      heapIncomingEdges: 0,
      trialRefCount: 2,
      decision: 'survivor',
      final: 'retained',
    },
    {
      objectId: 12,
      refCountBefore: 1,
      heapIncomingEdges: 1,
      trialRefCount: 0,
      decision: 'candidate',
      final: 'retained',
    },
    {
      objectId: 13,
      refCountBefore: 1,
      heapIncomingEdges: 1,
      trialRefCount: 0,
      decision: 'candidate',
      final: 'freed',
    },
  ]
  const objects = [
    { id: 1, kind: 'array' as const, label: 'Array#1' },
    { id: 12, kind: 'instance' as const, label: 'Instance(Node)#12' },
    { id: 13, kind: 'hash' as const, label: 'Hash#13' },
  ]
  const snapshot = { objectCount: 3, trackedBytes: 0, byValueKind: counts() }
  return {
    before: snapshot,
    after: snapshot,
    objects,
    globalRoots: [
      { name: 'holder', objectId: 1 },
      { name: 'alias', objectId: 1 },
    ],
    omittedGlobalRoots: 0,
    phases: {
      trialDeletion: {
        edgesVisited: 4,
        candidates: 2,
        objectDecisions: decisions,
        visitedEdges: [
          { fromId: 1, toId: 12, relation: { kind: 'arrayElement', index: 0 } },
          {
            fromId: 12,
            toId: 13,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 13,
            toId: 12,
            relation: { kind: 'hashValue', keyKind: 'string', key: '<a & "b">' },
          },
          {
            fromId: 13,
            toId: 12,
            relation: { kind: 'hashValue', keyKind: 'integer', key: '42' },
          },
        ],
        omittedObjectDecisions: 0,
        omittedEdgeDetails: 0,
      },
      scan: {
        restored: 1,
        garbageCandidates: 1,
        restoredObjects: [objects[1]],
        garbageCandidateObjects: [objects[2]],
        restorationWitnesses: [],
        omittedWitnesses: 0,
      },
      freeCycles: { freed: 1 },
    },
    collectedByValueKind: counts(),
  }
}

// A replayable report: rooted witness chain (1 → 12) plus a garbage cycle
// (13 ↔ 14), so the frames exercise every status class, the active class
// list, and both linkStyle emphases (bold and dimmed).
function replayableReport(): GcCollectionReport {
  const decisions: ObjectDecision[] = [
    {
      objectId: 1,
      refCountBefore: 2,
      heapIncomingEdges: 0,
      trialRefCount: 2,
      decision: 'survivor',
      final: 'retained',
    },
    {
      objectId: 12,
      refCountBefore: 1,
      heapIncomingEdges: 1,
      trialRefCount: 0,
      decision: 'candidate',
      final: 'retained',
    },
    {
      objectId: 13,
      refCountBefore: 1,
      heapIncomingEdges: 1,
      trialRefCount: 0,
      decision: 'candidate',
      final: 'freed',
    },
    {
      objectId: 14,
      refCountBefore: 1,
      heapIncomingEdges: 1,
      trialRefCount: 0,
      decision: 'candidate',
      final: 'freed',
    },
  ]
  const objects = [
    { id: 1, kind: 'array' as const, label: 'Array#1' },
    { id: 12, kind: 'instance' as const, label: 'Instance(Node)#12' },
    { id: 13, kind: 'instance' as const, label: 'Instance(Node)#13' },
    { id: 14, kind: 'instance' as const, label: 'Instance(Node)#14' },
  ]
  const snapshot = { objectCount: 4, trackedBytes: 0, byValueKind: counts() }
  return {
    before: snapshot,
    after: snapshot,
    objects,
    globalRoots: [{ name: 'holder', objectId: 1 }],
    omittedGlobalRoots: 0,
    phases: {
      trialDeletion: {
        edgesVisited: 3,
        candidates: 3,
        objectDecisions: decisions,
        visitedEdges: [
          { fromId: 1, toId: 12, relation: { kind: 'arrayElement', index: 0 } },
          {
            fromId: 13,
            toId: 14,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 14,
            toId: 13,
            relation: { kind: 'instanceField', name: 'next' },
          },
        ],
        omittedObjectDecisions: 0,
        omittedEdgeDetails: 0,
      },
      scan: {
        restored: 1,
        garbageCandidates: 2,
        restoredObjects: [objects[1]],
        garbageCandidateObjects: [objects[2], objects[3]],
        restorationWitnesses: [
          {
            objectId: 12,
            rootId: 1,
            predecessorId: 1,
            relation: { kind: 'arrayElement', index: 0 },
          },
        ],
        omittedWitnesses: 0,
      },
      freeCycles: { freed: 2 },
    },
    collectedByValueKind: counts(),
  }
}

describe('heap graph mermaid syntax', () => {
  it('is accepted by the real mermaid parser', async () => {
    const graph = buildHeapGraph(report())
    if (graph.status !== 'ok') {
      throw new Error(`expected an ok graph, got: ${graph.reason}`)
    }

    mermaid.initialize({ startOnLoad: false, securityLevel: 'strict' })
    await expect(mermaid.parse(graph.source)).resolves.toBeTruthy()
  })

  it('accepts every phase replay frame', async () => {
    const plan = buildReplayPlan(replayableReport())
    if (plan.status !== 'ok') {
      throw new Error(`expected an ok plan, got: ${plan.reason}`)
    }

    mermaid.initialize({ startOnLoad: false, securityLevel: 'strict' })
    for (const step of plan.steps) {
      const source = replayStepSource(plan.model, step)
      await expect(mermaid.parse(source)).resolves.toBeTruthy()
    }
  })
})
