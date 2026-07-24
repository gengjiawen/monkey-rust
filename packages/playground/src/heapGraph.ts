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

// Written into the node text as well as the ::: class: color must not be the
// only channel carrying the post-collection fate.
const FATE_TEXT: Record<FateClass, string> = {
  survivor: 'Survivor',
  restored: 'Restored',
  freed: 'Freed',
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

// Hundreds of global slots may alias one object; an uncapped list would let a
// single node label grow past everything the 40-node budget protects.
function globalNamesLabel(names: string[]): string {
  const shown = names.slice(0, 2).map(escapeMermaidText).join(', ')
  return names.length <= 2 ? shown : `${shown} +${names.length - 2} more`
}

/** One drawable arrow: parallel visited edges merged, labels in visit order. */
export interface MergedHeapEdge {
  fromId: number
  toId: number
  labels: string[]
}

/**
 * The drawable slice of a collection report, shared by the static topology
 * graph and the phase replay so both emit the exact same nodes and arrows
 * (and therefore the exact same mermaid layout).
 */
export interface HeapGraphModel {
  /** Drawn object ids, ascending. */
  sortedIds: number[]
  labels: Map<number, string>
  decisions: Map<number, ObjectDecision>
  globalNames: Map<number, string[]>
  /** Sorted by (fromId, toId); array order matches emitted link order. */
  mergedEdges: MergedHeapEdge[]
  /** Trial survivors that get a dotted External refs arrow, ascending id. */
  externalTargets: ObjectDecision[]
  droppedIsolated: number
}

export type HeapGraphModelResult =
  | { status: 'ok'; model: HeapGraphModel }
  | { status: 'unavailable'; reason: string }

/**
 * Collect the drawable topology of the heap the collector actually visited.
 *
 * Only objects that participate in at least one reported edge, plus every
 * candidate, become nodes; isolated survivors (mostly VM bookkeeping values)
 * are dropped and counted in `droppedIsolated`. When the report truncated
 * edge or decision details the drawing would silently lie, so the model is
 * reported as unavailable instead.
 */
export function buildHeapGraphModel(
  report: GcCollectionReport
): HeapGraphModelResult {
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

  const mergedEdgeMap = new Map<string, MergedHeapEdge>()
  for (const edge of trial.visitedEdges) {
    const key = `${edge.fromId}->${edge.toId}`
    const label = formatEdgeRelation(edge.relation)
    const entry = mergedEdgeMap.get(key)
    if (entry) {
      entry.labels.push(label)
    } else {
      mergedEdgeMap.set(key, { fromId: edge.fromId, toId: edge.toId, labels: [label] })
    }
  }
  const mergedEdges = [...mergedEdgeMap.values()].sort(
    (left, right) => left.fromId - right.fromId || left.toId - right.toId
  )

  return {
    status: 'ok',
    model: {
      sortedIds,
      labels,
      decisions,
      globalNames,
      mergedEdges,
      externalTargets,
      droppedIsolated,
    },
  }
}

/** Per-node hooks for renderHeapGraphModel. */
export interface NodeDecoration {
  /** Extra text between the object label and its global-names line. */
  suffix?: string
  /** mermaid ::: class attached to the node. */
  className?: string
}

/**
 * Emit mermaid source for a model. Link order is deterministic: solid merged
 * edges first (so `linkStyle N` addresses `mergedEdges[N]`), then one dotted
 * External refs arrow per external target. `extraLines` (linkStyle / class
 * statements) never affect layout, only styling.
 */
export function renderHeapGraphModel(
  model: HeapGraphModel,
  decorate: (id: number) => NodeDecoration,
  extraLines: readonly string[] = []
): string {
  const lines: string[] = ['flowchart LR']
  if (model.externalTargets.length > 0) {
    lines.push(
      '  ext(["External refs<br/>constants · globals · stack"]):::external'
    )
  }
  for (const id of model.sortedIds) {
    const label = escapeMermaidText(model.labels.get(id) ?? `Object#${id}`)
    const names = model.globalNames.get(id)
    const nameLine = names
      ? `<br/><i>global${names.length > 1 ? 's' : ''}: ${globalNamesLabel(names)}</i>`
      : ''
    const { suffix = '', className } = decorate(id)
    const classTag = className ? `:::${className}` : ''
    lines.push(`  o${id}["${label}${suffix}${nameLine}"]${classTag}`)
  }
  for (const edge of model.mergedEdges) {
    const label = escapeMermaidText(mergedEdgeLabel(edge.labels))
    lines.push(`  o${edge.fromId} -- "${label}" --> o${edge.toId}`)
  }
  for (const decision of model.externalTargets) {
    lines.push(`  ext -. "×${decision.trialRefCount}" .-> o${decision.objectId}`)
  }
  lines.push(...extraLines)
  // No classDef lines: mermaid still adds the ::: class names to each node's
  // SVG class attribute, and the canvas Tailwind variants in HeapGraphView /
  // PhaseReplayView style them with the same Radix tokens as the walkthrough
  // badges (classDef cannot express CSS variables).
  return lines.join('\n')
}

/**
 * Build a mermaid flowchart of the heap topology the collector actually
 * visited: solid arrows are the reported heap-to-heap edges, and a single
 * "External refs" pseudo-node stands in for every non-heap reference
 * (constants, global slots, VM stack) with one dotted arrow per trial
 * survivor labeled by its trial reference count.
 */
export function buildHeapGraph(report: GcCollectionReport): HeapGraph {
  const result = buildHeapGraphModel(report)
  if (result.status === 'unavailable') {
    return result
  }
  const { model } = result
  const source = renderHeapGraphModel(model, (id) => {
    const decision = model.decisions.get(id)
    const fate = decision ? fateClass(decision) : null
    return fate ? { suffix: ` · ${FATE_TEXT[fate]}`, className: fate } : {}
  })
  return { status: 'ok', source, droppedIsolated: model.droppedIsolated }
}
