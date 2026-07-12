import type { GcCollectionReport, ObjectDecision } from './gcReport'
import { formatEdgeRelation } from './gcReport'

/** Node budget keeping the rendered flowchart readable on one screen. */
export const MAX_GRAPH_NODES = 40

export type HeapGraph =
  | { status: 'ok'; source: string; droppedIsolated: number }
  | { status: 'unavailable'; reason: string }

type FateClass = 'survivor' | 'restored' | 'freed'

function fateClass(decision: ObjectDecision): FateClass {
  if (decision.decision === 'survivor') {
    return 'survivor'
  }
  return decision.final === 'freed' ? 'freed' : 'restored'
}

/**
 * Escape text for use inside a quoted mermaid label. `#` must go first so the
 * `#name;` entities produced by later replacements survive untouched.
 */
function escapeMermaidText(text: string): string {
  return text
    .replace(/#/g, '#35;')
    .replace(/&/g, '#amp;')
    .replace(/"/g, '#quot;')
    .replace(/</g, '#lt;')
    .replace(/>/g, '#gt;')
}

function mergedEdgeLabel(labels: string[]): string {
  if (labels.length <= 2) {
    return labels.join(', ')
  }
  return `${labels[0]}, ${labels[1]} +${labels.length - 2} more`
}

/**
 * Build a mermaid flowchart of the heap topology the collector actually
 * visited: solid arrows are the reported heap-to-heap edges, and a single
 * "External refs" pseudo-node stands in for every non-heap reference
 * (constants, global slots, VM stack) with one dotted arrow per trial
 * survivor labeled by its trial reference count.
 *
 * Only objects that participate in at least one reported edge, plus every
 * candidate, become nodes; isolated survivors (mostly VM bookkeeping values)
 * are dropped and counted in `droppedIsolated`. When the report truncated
 * edge or decision details the graph would silently lie, so it is reported
 * as unavailable instead.
 */
export function buildHeapGraph(report: GcCollectionReport): HeapGraph {
  const trial = report.phases.trialDeletion
  if (trial.omittedObjectDecisions > 0 || trial.omittedEdgeDetails > 0) {
    return {
      status: 'unavailable',
      reason:
        'This report truncated edge or decision details, so a drawn graph would be missing references. Use the walkthrough table instead.',
    }
  }

  const includedIds = new Set<number>()
  for (const edge of trial.visitedEdges) {
    includedIds.add(edge.fromId)
    includedIds.add(edge.toId)
  }
  for (const decision of trial.objectDecisions) {
    if (decision.decision === 'candidate') {
      includedIds.add(decision.objectId)
    }
  }

  if (includedIds.size === 0) {
    return {
      status: 'unavailable',
      reason:
        'This collection visited no heap-to-heap references, so there is no topology to draw.',
    }
  }
  if (includedIds.size > MAX_GRAPH_NODES) {
    return {
      status: 'unavailable',
      reason: `This collection connects ${includedIds.size} objects; graphs are only drawn for ${MAX_GRAPH_NODES} or fewer to stay readable.`,
    }
  }

  let droppedIsolated = 0
  for (const decision of trial.objectDecisions) {
    if (!includedIds.has(decision.objectId)) {
      droppedIsolated += 1
    }
  }

  const labels = new Map(report.objects.map((object) => [object.id, object.label]))
  const decisions = new Map(
    trial.objectDecisions.map((decision) => [decision.objectId, decision])
  )
  const globalNames = new Map<number, string[]>()
  for (const root of report.globalRoots) {
    const names = globalNames.get(root.objectId)
    if (names) {
      names.push(root.name)
    } else {
      globalNames.set(root.objectId, [root.name])
    }
  }

  const sortedIds = [...includedIds].sort((left, right) => left - right)
  const externalTargets = sortedIds
    .map((id) => decisions.get(id))
    .filter(
      (decision): decision is ObjectDecision =>
        decision !== undefined &&
        decision.decision === 'survivor' &&
        decision.trialRefCount > 0
    )

  const lines: string[] = ['flowchart LR']
  if (externalTargets.length > 0) {
    lines.push(
      '  ext(["External refs<br/>constants · globals · stack"]):::external'
    )
  }
  for (const id of sortedIds) {
    const label = escapeMermaidText(labels.get(id) ?? `Object#${id}`)
    const names = globalNames.get(id)
    const nameLine = names
      ? `<br/><i>global${names.length > 1 ? 's' : ''}: ${names
          .map(escapeMermaidText)
          .join(', ')}</i>`
      : ''
    const decision = decisions.get(id)
    const fate = decision ? `:::${fateClass(decision)}` : ''
    lines.push(`  o${id}["${label}${nameLine}"]${fate}`)
  }

  const mergedEdges = new Map<
    string,
    { fromId: number; toId: number; labels: string[] }
  >()
  for (const edge of trial.visitedEdges) {
    const key = `${edge.fromId}->${edge.toId}`
    const label = formatEdgeRelation(edge.relation)
    const entry = mergedEdges.get(key)
    if (entry) {
      entry.labels.push(label)
    } else {
      mergedEdges.set(key, { fromId: edge.fromId, toId: edge.toId, labels: [label] })
    }
  }
  const sortedEdges = [...mergedEdges.values()].sort(
    (left, right) => left.fromId - right.fromId || left.toId - right.toId
  )
  for (const edge of sortedEdges) {
    const label = escapeMermaidText(mergedEdgeLabel(edge.labels))
    lines.push(`  o${edge.fromId} -- "${label}" --> o${edge.toId}`)
  }
  for (const decision of externalTargets) {
    lines.push(`  ext -. "×${decision.trialRefCount}" .-> o${decision.objectId}`)
  }

  // No classDef lines: mermaid still adds the ::: class names to each node's
  // SVG class attribute, and globals.css styles them with the same Radix
  // tokens as the walkthrough badges (classDef cannot express CSS variables).
  return { status: 'ok', source: lines.join('\n'), droppedIsolated }
}
