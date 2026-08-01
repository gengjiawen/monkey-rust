import type { DebuggerHit, DebuggerValue } from './debuggerReport'
import { formatEdgeRelation } from './gcReport'
import { escapeMermaidText, MAX_GRAPH_NODES, mergedEdgeLabel } from './heapGraph'

export type DebuggerHeapGraph =
  | {
      status: 'ok'
      source: string
      /** Recorded objects not drawn because of the node budget. */
      droppedGraphNodes: number
      /** Recorded edges not drawn because an endpoint is not drawn. */
      droppedGraphEdges: number
    }
  | { status: 'empty' }

function rootIds(hit: DebuggerHit): number[] {
  const roots: number[] = []
  const push = (value: DebuggerValue | null) => {
    if (value !== null && value.heapId !== null) {
      roots.push(value.heapId)
    }
  }
  for (const slot of hit.globals) {
    push(slot.value)
  }
  for (const frame of hit.frames) {
    push(frame.callee)
    for (const slot of frame.locals) {
      push(slot.value)
    }
    for (const capture of frame.captures) {
      push(capture.value)
    }
    for (const temporary of frame.temporaries) {
      push(temporary.value)
    }
  }
  return roots
}

/**
 * Project one hit's heap snapshot onto a mermaid flowchart of at most
 * `MAX_GRAPH_NODES` nodes. Node priority: the highlighted object (if the
 * snapshot recorded it), then a BFS from the objects the visible slots
 * reference, then the remaining recorded objects by ascending id. Only edges
 * with both endpoints drawn are emitted; everything else is counted, not
 * silently dropped. This is an explicitly labeled readability projection on
 * top of the snapshot's own object/edge budget, not the full topology.
 */
export function buildDebuggerHeapGraph(
  hit: DebuggerHit,
  highlightedHeapId: number | null
): DebuggerHeapGraph {
  const { heap } = hit
  if (heap.objects.length === 0) {
    return { status: 'empty' }
  }

  const byId = new Map(heap.objects.map((object) => [object.id, object]))
  const adjacency = new Map<number, number[]>()
  for (const edge of heap.edges) {
    const targets = adjacency.get(edge.from)
    if (targets) {
      targets.push(edge.to)
    } else {
      adjacency.set(edge.from, [edge.to])
    }
  }

  // Full BFS order from the roots; selection consumes it up to the budget.
  const bfsOrder: number[] = []
  const seen = new Set<number>()
  const discover = (id: number) => {
    if (!byId.has(id) || seen.has(id)) {
      return
    }
    seen.add(id)
    bfsOrder.push(id)
  }
  for (const root of rootIds(hit)) {
    discover(root)
  }
  for (let cursor = 0; cursor < bfsOrder.length; cursor += 1) {
    for (const target of adjacency.get(bfsOrder[cursor]) ?? []) {
      discover(target)
    }
  }

  const selected = new Set<number>()
  if (highlightedHeapId !== null && byId.has(highlightedHeapId)) {
    selected.add(highlightedHeapId)
  }
  for (const id of bfsOrder) {
    if (selected.size >= MAX_GRAPH_NODES) {
      break
    }
    selected.add(id)
  }
  if (selected.size < MAX_GRAPH_NODES) {
    const remaining = heap.objects
      .map((object) => object.id)
      .filter((id) => !selected.has(id))
      .sort((left, right) => left - right)
    for (const id of remaining) {
      if (selected.size >= MAX_GRAPH_NODES) {
        break
      }
      selected.add(id)
    }
  }

  const lines: string[] = ['flowchart LR']
  const sortedIds = [...selected].sort((left, right) => left - right)
  for (const id of sortedIds) {
    const object = byId.get(id)
    if (!object) {
      continue
    }
    const memberLines = object.members.map(
      (member) =>
        `<br/>${escapeMermaidText(
          `${formatEdgeRelation(member.relation)} = ${member.display}`
        )}`
    )
    const label = escapeMermaidText(`#${id} ${object.label}`)
    lines.push(`  o${id}["${label}${memberLines.join('')}"]`)
  }

  const mergedEdges = new Map<
    string,
    { from: number; to: number; labels: string[] }
  >()
  let droppedGraphEdges = 0
  for (const edge of heap.edges) {
    if (!selected.has(edge.from) || !selected.has(edge.to)) {
      droppedGraphEdges += 1
      continue
    }
    const key = `${edge.from}->${edge.to}`
    const label = formatEdgeRelation(edge.relation)
    const entry = mergedEdges.get(key)
    if (entry) {
      entry.labels.push(label)
    } else {
      mergedEdges.set(key, { from: edge.from, to: edge.to, labels: [label] })
    }
  }
  const sortedEdges = [...mergedEdges.values()].sort(
    (left, right) => left.from - right.from || left.to - right.to
  )
  for (const edge of sortedEdges) {
    const label = escapeMermaidText(mergedEdgeLabel(edge.labels))
    lines.push(`  o${edge.from} -- "${label}" --> o${edge.to}`)
  }

  // Highlighting regenerates the source with a `class` directive instead of
  // querying mermaid's generated DOM ids; the canvas styles `.highlighted`.
  if (highlightedHeapId !== null && selected.has(highlightedHeapId)) {
    lines.push(`  class o${highlightedHeapId} highlighted`)
  }

  return {
    status: 'ok',
    source: lines.join('\n'),
    droppedGraphNodes: heap.objects.length - selected.size,
    droppedGraphEdges,
  }
}
