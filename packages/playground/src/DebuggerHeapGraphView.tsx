'use client'

import { useMemo } from 'react'

import { buildDebuggerHeapGraph } from './debuggerHeapGraph'
import type { DebuggerHit } from './debuggerReport'
import { MermaidGraphCanvas } from './MermaidGraphCanvas'

// The graph builder tags the pinned/hovered node with `class oN highlighted`;
// mermaid classDef cannot express CSS variables, so the color lives here.
const highlightCanvasClass =
  '[&_.node.highlighted_:is(rect,path,polygon)]:fill-(--amber-a4)! [&_.node.highlighted_:is(rect,path,polygon)]:stroke-(--amber-a8)! [&_.node.highlighted_.nodeLabel]:text-(--amber-11)!'

const mutedClass = 'text-xs text-(--gray-10)'

const footnoteClass = 'mx-0.5 mt-0 mb-2 text-xs leading-normal text-(--gray-10)'

export function DebuggerHeapGraphView({
  hit,
  highlightedHeapId,
}: {
  hit: DebuggerHit
  highlightedHeapId: number | null
}) {
  const graph = useMemo(
    () => buildDebuggerHeapGraph(hit, highlightedHeapId),
    [hit, highlightedHeapId]
  )
  const { omittedObjects, omittedEdges } = hit.heap

  const snapshotNote =
    omittedObjects > 0 || omittedEdges > 0
      ? `The snapshot budget omitted ${omittedObjects} object${
          omittedObjects === 1 ? '' : 's'
        } and ${omittedEdges} reference${
          omittedEdges === 1 ? '' : 's'
        } at record time.`
      : null
  const projectionNote =
    graph.status === 'ok' &&
    (graph.droppedGraphNodes > 0 || graph.droppedGraphEdges > 0)
      ? `For readability this drawing leaves out ${graph.droppedGraphNodes} recorded object${
          graph.droppedGraphNodes === 1 ? '' : 's'
        } and ${graph.droppedGraphEdges} recorded reference${
          graph.droppedGraphEdges === 1 ? '' : 's'
        }.`
      : null

  return (
    <MermaidGraphCanvas
      title="Heap objects"
      ariaLabel="Debugger heap graph"
      idPrefix="debugger-heap-graph"
      source={graph.status === 'ok' ? graph.source : null}
      emptyContent={
        graph.status === 'empty' ? (
          <>
            No user-visible heap objects are referenced at this hit; every
            value on the stack is an inlined scalar.
            {snapshotNote ? ` ${snapshotNote}` : ''}
          </>
        ) : null
      }
      canvasClassName={highlightCanvasClass}
      footer={
        snapshotNote || projectionNote ? (
          <p className={footnoteClass}>
            {snapshotNote}
            {snapshotNote && projectionNote ? ' ' : ''}
            {projectionNote}
          </p>
        ) : null
      }
    >
      <ul className={`${mutedClass} m-0 pl-5 [&_li+li]:mt-1`}>
        <li>
          Each node is one user-visible heap object; scalar members are
          inlined into its label.
        </li>
        <li>
          Arrows are heap-to-heap references. Hover or pin a{' '}
          <span className="font-mono">ref #n</span> chip on the left to
          highlight its node.
        </li>
      </ul>
    </MermaidGraphCanvas>
  )
}
