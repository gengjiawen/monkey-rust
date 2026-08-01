'use client'

import { useMemo } from 'react'

import type { GcCollectionReport } from './gcReport'
import { buildHeapGraph } from './heapGraph'
import { MermaidGraphCanvas } from './MermaidGraphCanvas'

// Fate colors for heap topology nodes. The generator tags nodes with
// :::survivor / :::restored / :::freed / :::external but emits no classDef
// (mermaid classDef cannot express CSS variables), so the badge palette is
// applied here. `!` outweighs mermaid's own #id-prefixed node styles.
const fateCanvasClass = [
  '[&_.node.survivor_:is(rect,path,polygon)]:fill-(--green-a3)! [&_.node.survivor_:is(rect,path,polygon)]:stroke-(--green-a6)! [&_.node.survivor_.nodeLabel]:text-(--green-11)!',
  '[&_.node.restored_:is(rect,path,polygon)]:fill-(--blue-a3)! [&_.node.restored_:is(rect,path,polygon)]:stroke-(--blue-a6)! [&_.node.restored_.nodeLabel]:text-(--blue-11)!',
  '[&_.node.freed_:is(rect,path,polygon)]:fill-(--red-a3)! [&_.node.freed_:is(rect,path,polygon)]:stroke-(--red-a6)! [&_.node.freed_.nodeLabel]:text-(--red-11)!',
  '[&_.node.external_:is(rect,path,polygon)]:fill-(--gray-a2)! [&_.node.external_:is(rect,path,polygon)]:stroke-(--gray-a8)! [&_.node.external_:is(rect,path,polygon)]:[stroke-dasharray:4_3]! [&_.node.external_.nodeLabel]:text-(--gray-11)!',
].join(' ')

const mutedClass = 'text-xs text-(--gray-10)'

const footnoteClass = 'mx-0.5 mt-0 mb-2 text-xs leading-normal text-(--gray-10)'

export function HeapGraphView({ report }: { report: GcCollectionReport }) {
  const graph = useMemo(() => buildHeapGraph(report), [report])

  return (
    <MermaidGraphCanvas
      title="Heap topology"
      ariaLabel="Heap topology graph"
      idPrefix="gc-heap-graph"
      source={graph.status === 'ok' ? graph.source : null}
      emptyContent={graph.status === 'unavailable' ? graph.reason : null}
      canvasClassName={fateCanvasClass}
      footer={
        graph.status === 'ok' && graph.droppedIsolated > 0 ? (
          <p className={footnoteClass}>
            {graph.droppedIsolated} object
            {graph.droppedIsolated > 1 ? 's' : ''} with no visited heap edges
            (mostly VM bookkeeping values){' '}
            {graph.droppedIsolated > 1 ? 'are' : 'is'} not drawn.
          </p>
        ) : null
      }
    >
      <ul className={`${mutedClass} m-0 pl-5 [&_li+li]:mt-1`}>
        <li>Solid arrows show heap-to-heap references at collection start.</li>
        <li>
          Dotted arrows from External refs mark each trial survivor&apos;s
          remaining non-heap references (×N is its trial RC).
        </li>
        <li>
          The · Survivor / · Restored / · Freed suffix on each node is that
          object&apos;s fate after the collection; the arrows still show the
          topology from before it.
        </li>
      </ul>
    </MermaidGraphCanvas>
  )
}
