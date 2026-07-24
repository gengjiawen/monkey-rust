import { describe, expect, it } from 'vitest'

import { buildDebuggerHeapGraph } from '../debuggerHeapGraph'
import type { DebuggerHeapObject } from '../debuggerReport'
import {
  emptyHeap,
  frame,
  hit,
  refValue,
  slot,
} from './debuggerFixtures'

function plainObject(id: number): DebuggerHeapObject {
  return { id, kind: 'array', label: 'Array', members: [] }
}

describe('buildDebuggerHeapGraph', () => {
  it('draws nodes with inlined members and typed edge labels', () => {
    const graph = buildDebuggerHeapGraph(
      hit(1, {
        globals: [slot('p', 0, refValue(5, '[[1], "x"]'))],
        heap: emptyHeap({
          objects: [
            {
              id: 5,
              kind: 'array',
              label: 'Array',
              members: [
                {
                  relation: { kind: 'arrayElement', index: 1 },
                  display: '"x"',
                },
              ],
            },
            { id: 9, kind: 'instance', label: 'Instance(Node)', members: [] },
          ],
          edges: [
            { from: 5, to: 9, relation: { kind: 'arrayElement', index: 0 } },
          ],
        }),
      }),
      null
    )

    if (graph.status !== 'ok') {
      throw new Error('expected a drawable graph')
    }
    expect(graph.source).toContain('flowchart LR')
    expect(graph.source).toContain(
      'o5["#35;5 Array<br/>items[1] = #quot;x#quot;"]'
    )
    expect(graph.source).toContain('o9["#35;9 Instance(Node)"]')
    expect(graph.source).toContain('o5 -- "items[0]" --> o9')
    expect(graph.source).not.toContain('highlighted')
    expect(graph.droppedGraphNodes).toBe(0)
    expect(graph.droppedGraphEdges).toBe(0)
  })

  it('tags the highlighted node with a class directive', () => {
    const snapshot = hit(1, {
      globals: [slot('p', 0, refValue(5, '[]'))],
      heap: emptyHeap({ objects: [plainObject(5), plainObject(9)] }),
    })

    const highlighted = buildDebuggerHeapGraph(snapshot, 9)
    if (highlighted.status !== 'ok') {
      throw new Error('expected a drawable graph')
    }
    expect(highlighted.source).toContain('class o9 highlighted')

    const unknown = buildDebuggerHeapGraph(snapshot, 123)
    if (unknown.status !== 'ok') {
      throw new Error('expected a drawable graph')
    }
    expect(unknown.source).not.toContain('highlighted')
  })

  it('reports an empty snapshot instead of drawing', () => {
    expect(buildDebuggerHeapGraph(hit(1), null)).toEqual({ status: 'empty' })
  })

  it('prefers root-reachable objects, then ascending ids, and counts drops', () => {
    const objects = Array.from({ length: 50 }, (_, id) => plainObject(id))
    const snapshot = hit(1, {
      frames: [
        frame({ locals: [slot('tail', 0, refValue(45, '[]'))] }),
      ],
      heap: emptyHeap({ objects }),
    })

    const graph = buildDebuggerHeapGraph(snapshot, null)
    if (graph.status !== 'ok') {
      throw new Error('expected a drawable graph')
    }
    // The root object is drawn even though 40 lower ids exist.
    expect(graph.source).toContain('o45[')
    expect(graph.source).toContain('o38[')
    expect(graph.source).not.toContain('o39[')
    expect(graph.droppedGraphNodes).toBe(10)
  })

  it('always draws the highlighted node and drops half-selected edges', () => {
    const objects = Array.from({ length: 41 }, (_, id) => plainObject(id))
    const snapshot = hit(1, {
      globals: [slot('root', 0, refValue(0, '[]'))],
      heap: emptyHeap({
        objects,
        edges: [
          { from: 0, to: 40, relation: { kind: 'arrayElement', index: 0 } },
          { from: 5, to: 39, relation: { kind: 'arrayElement', index: 0 } },
        ],
      }),
    })

    const graph = buildDebuggerHeapGraph(snapshot, 39)
    if (graph.status !== 'ok') {
      throw new Error('expected a drawable graph')
    }
    // Priority: highlighted 39, then BFS {0, 40}, then ids 1…37.
    expect(graph.source).toContain('o39[')
    expect(graph.source).toContain('o40[')
    expect(graph.source).toContain('class o39 highlighted')
    expect(graph.source).not.toContain('o38[')
    expect(graph.source).toContain('o0 -- "items[0]" --> o40')
    expect(graph.source).toContain('o5 -- "items[0]" --> o39')
    expect(graph.droppedGraphNodes).toBe(1)
    expect(graph.droppedGraphEdges).toBe(0)

    const unhighlighted = buildDebuggerHeapGraph(snapshot, null)
    if (unhighlighted.status !== 'ok') {
      throw new Error('expected a drawable graph')
    }
    // Without the highlight, 39 falls off the budget and its edge with it.
    expect(unhighlighted.source).not.toContain('o39[')
    expect(unhighlighted.droppedGraphNodes).toBe(1)
    expect(unhighlighted.droppedGraphEdges).toBe(1)
  })

  it('merges parallel edges into one labeled arrow', () => {
    const snapshot = hit(1, {
      globals: [slot('pair', 0, refValue(1, '[]'))],
      heap: emptyHeap({
        objects: [plainObject(1), plainObject(2)],
        edges: [
          { from: 1, to: 2, relation: { kind: 'arrayElement', index: 0 } },
          { from: 1, to: 2, relation: { kind: 'arrayElement', index: 1 } },
        ],
      }),
    })

    const graph = buildDebuggerHeapGraph(snapshot, null)
    if (graph.status !== 'ok') {
      throw new Error('expected a drawable graph')
    }
    expect(graph.source).toContain('o1 -- "items[0], items[1]" --> o2')
  })
})
