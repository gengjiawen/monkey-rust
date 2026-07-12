import { describe, expect, it } from 'vitest'

import type {
  GcCollectionReport,
  GcObjectSummary,
  GlobalRoot,
  ObjectDecision,
  ValueKindCounts,
  VisitedEdge,
} from '../gcReport'
import { buildHeapGraph, MAX_GRAPH_NODES } from '../heapGraph'

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
  omittedEdgeDetails?: number
  omittedObjectDecisions?: number
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
        restorationWitnesses: [],
        omittedWitnesses: 0,
      },
      freeCycles: { freed: garbageObjects.length },
    },
    collectedByValueKind: counts(),
  }
}

describe('buildHeapGraph', () => {
  it('draws edge participants, candidates, and one external refs root', () => {
    const graph = buildHeapGraph(
      makeReport({
        objects: [
          { id: 1, kind: 'array', label: 'Array#1' },
          { id: 5, kind: 'null', label: 'Null#5' },
          { id: 12, kind: 'instance', label: 'Instance(Node)#12' },
          { id: 13, kind: 'instance', label: 'Instance(Node)#13' },
          { id: 20, kind: 'instance', label: 'Instance(Node)#20' },
          { id: 21, kind: 'instance', label: 'Instance(Node)#21' },
        ],
        globalRoots: [{ name: 'holder', objectId: 1 }],
        decisions: [
          survivor(1, 2),
          survivor(5, 7),
          candidate(12, 'retained'),
          candidate(13, 'retained'),
          candidate(20, 'freed'),
          candidate(21, 'freed'),
        ],
        edges: [
          { fromId: 1, toId: 12, relation: { kind: 'arrayElement', index: 0 } },
          {
            fromId: 12,
            toId: 13,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 13,
            toId: 12,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 20,
            toId: 21,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 21,
            toId: 20,
            relation: { kind: 'instanceField', name: 'next' },
          },
        ],
      })
    )

    if (graph.status !== 'ok') {
      throw new Error(`expected an ok graph, got: ${graph.reason}`)
    }
    const lines = graph.source.split('\n')
    expect(lines[0]).toBe('flowchart LR')
    expect(lines).toContain(
      '  o1["Array#35;1 · Survivor<br/><i>global: holder</i>"]:::survivor'
    )
    expect(lines).toContain('  o12["Instance(Node)#35;12 · Restored"]:::restored')
    expect(lines).toContain('  o20["Instance(Node)#35;20 · Freed"]:::freed')
    expect(lines).toContain('  o1 -- "items[0]" --> o12')
    expect(lines).toContain('  o12 -- "fields[#quot;next#quot;]" --> o13')
    expect(lines).toContain(
      '  ext(["External refs<br/>constants · globals · stack"]):::external'
    )
    expect(lines).toContain('  ext -. "×2" .-> o1')
    // The isolated Null survivor is dropped, and candidates get no ext arrow.
    expect(graph.source).not.toContain('o5')
    expect(graph.droppedIsolated).toBe(1)
    expect(lines.filter((line) => line.includes('ext -.'))).toHaveLength(1)
  })

  it('omits the external refs node when no survivor is drawn', () => {
    const graph = buildHeapGraph(
      makeReport({
        objects: [
          { id: 20, kind: 'instance', label: 'Instance(Node)#20' },
          { id: 21, kind: 'instance', label: 'Instance(Node)#21' },
        ],
        decisions: [candidate(20, 'freed'), candidate(21, 'freed')],
        edges: [
          {
            fromId: 20,
            toId: 21,
            relation: { kind: 'instanceField', name: 'next' },
          },
          {
            fromId: 21,
            toId: 20,
            relation: { kind: 'instanceField', name: 'next' },
          },
        ],
      })
    )

    if (graph.status !== 'ok') {
      throw new Error(`expected an ok graph, got: ${graph.reason}`)
    }
    expect(graph.source).not.toContain('ext([')
    expect(graph.source).not.toContain('ext -.')
    expect(graph.droppedIsolated).toBe(0)
  })

  it('merges parallel edges and truncates long relation lists', () => {
    const graph = buildHeapGraph(
      makeReport({
        objects: [
          { id: 1, kind: 'array', label: 'Array#1' },
          { id: 2, kind: 'instance', label: 'Instance(Node)#2' },
        ],
        decisions: [survivor(1, 1), candidate(2, 'retained')],
        edges: [
          { fromId: 1, toId: 2, relation: { kind: 'arrayElement', index: 0 } },
          { fromId: 1, toId: 2, relation: { kind: 'arrayElement', index: 1 } },
          { fromId: 1, toId: 2, relation: { kind: 'arrayElement', index: 2 } },
        ],
      })
    )

    if (graph.status !== 'ok') {
      throw new Error(`expected an ok graph, got: ${graph.reason}`)
    }
    expect(graph.source).toContain(
      '  o1 -- "items[0], items[1] +1 more" --> o2'
    )
  })

  it('caps the alias list in node labels', () => {
    const graph = buildHeapGraph(
      makeReport({
        objects: [
          { id: 1, kind: 'array', label: 'Array#1' },
          { id: 2, kind: 'instance', label: 'Instance(Node)#2' },
        ],
        globalRoots: ['a', 'b', 'c', 'd'].map((name) => ({
          name,
          objectId: 1,
        })),
        decisions: [survivor(1, 4), candidate(2, 'retained')],
        edges: [
          { fromId: 1, toId: 2, relation: { kind: 'arrayElement', index: 0 } },
        ],
      })
    )

    if (graph.status !== 'ok') {
      throw new Error(`expected an ok graph, got: ${graph.reason}`)
    }
    expect(graph.source).toContain(
      '  o1["Array#35;1 · Survivor<br/><i>globals: a, b +2 more</i>"]:::survivor'
    )
  })

  it('escapes mermaid-sensitive characters in labels', () => {
    const graph = buildHeapGraph(
      makeReport({
        objects: [
          { id: 1, kind: 'hash', label: 'Hash#1' },
          { id: 2, kind: 'string', label: 'String("<a & b>")#2' },
        ],
        decisions: [survivor(1, 1), candidate(2, 'retained')],
        edges: [
          {
            fromId: 1,
            toId: 2,
            relation: { kind: 'hashValue', keyKind: 'string', key: '"k"' },
          },
        ],
      })
    )

    if (graph.status !== 'ok') {
      throw new Error(`expected an ok graph, got: ${graph.reason}`)
    }
    expect(graph.source).toContain(
      '  o2["String(#quot;#lt;a #amp; b#gt;#quot;)#35;2 · Restored"]:::restored'
    )
    expect(graph.source).toContain(
      '  o1 -- "values[#quot;#quot;k#quot;#quot;]" --> o2'
    )
  })

  it('is unavailable when the report truncated details', () => {
    const base = {
      objects: [
        { id: 1, kind: 'array', label: 'Array#1' } as GcObjectSummary,
        { id: 2, kind: 'instance', label: 'Instance(Node)#2' } as GcObjectSummary,
      ],
      decisions: [survivor(1, 1), candidate(2, 'retained')],
      edges: [
        {
          fromId: 1,
          toId: 2,
          relation: { kind: 'arrayElement', index: 0 },
        } as VisitedEdge,
      ],
    }

    const truncatedEdges = buildHeapGraph(
      makeReport({ ...base, omittedEdgeDetails: 1 })
    )
    expect(truncatedEdges.status).toBe('unavailable')
    if (truncatedEdges.status === 'unavailable') {
      expect(truncatedEdges.reason).toMatch(/truncated/)
    }

    const truncatedDecisions = buildHeapGraph(
      makeReport({ ...base, omittedObjectDecisions: 1 })
    )
    expect(truncatedDecisions.status).toBe('unavailable')
  })

  it('is unavailable when there is nothing to draw', () => {
    const graph = buildHeapGraph(
      makeReport({
        objects: [{ id: 1, kind: 'null', label: 'Null#1' }],
        decisions: [survivor(1, 7)],
      })
    )
    expect(graph.status).toBe('unavailable')
    if (graph.status === 'unavailable') {
      expect(graph.reason).toMatch(/no heap-to-heap references/)
    }
  })

  it('is unavailable when the graph would exceed the node cap', () => {
    const ids = Array.from({ length: MAX_GRAPH_NODES + 1 }, (_, index) => index)
    const graph = buildHeapGraph(
      makeReport({
        objects: ids.map((id) => ({
          id,
          kind: 'instance',
          label: `Instance(Node)#${id}`,
        })),
        decisions: ids.map((id) => candidate(id, 'freed')),
      })
    )
    expect(graph.status).toBe('unavailable')
    if (graph.status === 'unavailable') {
      expect(graph.reason).toContain(`${MAX_GRAPH_NODES + 1}`)
    }
  })
})
