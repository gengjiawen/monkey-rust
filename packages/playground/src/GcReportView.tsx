'use client'

import { Fragment, useMemo, useState } from 'react'

import type {
  EdgeRelation,
  GcCollectionReport,
  GcObjectSummary,
  GcRunEnvelope,
  HeapSnapshot,
  ObjectDecision,
  RestorationWitness,
  SourceSpan,
  ValueKind,
  VisitedEdge,
} from './gcReport'
import {
  formatEdgeRelation,
  isCandidateRelatedEdge,
  rebuildWitnessPath,
  scanResultLabel,
  valueKinds,
} from './gcReport'
import { HeapGraphView } from './HeapGraphView'
import { PhaseReplayView } from './PhaseReplayView'

export type GcPanelState =
  | { status: 'idle' }
  | { status: 'running' }
  | GcRunEnvelope
  | { status: 'invalid'; message: string }

interface GcReportViewProps {
  state: GcPanelState
  onErrorSpanSelect?: (span: SourceSpan) => void
}

type DecisionFilter = 'candidates' | 'survivors' | 'all'
type EdgeFilter = 'candidate-related' | 'all'

const cardClass =
  'rounded-[10px] border border-(--gray-a5) bg-(--color-panel-solid) p-4 shadow-[0_1px_2px_var(--black-a3)]'

const mutedClass = 'text-xs text-(--gray-10)'

const footnoteClass = 'mx-0.5 mt-0 mb-2 text-xs leading-normal text-(--gray-10)'

const eyebrowClass =
  'block text-[11px] font-bold tracking-[0.08em] uppercase text-(--accent-11)'

// gc-section / collected-card <h2>: margins reset, 16px, near-black.
const sectionHeadingClass = 'm-0 mb-3 text-base text-(--gray-12)'

// <h3> inside cards keeps its user-agent size; only margins/color are set.
const cardSubheadingClass = 'm-0 mb-3 text-(--gray-12)'

const helperTextClass =
  'mt-0.5 block text-[11px] leading-[1.35] font-normal text-(--gray-10)'

const phaseIndexClass =
  'mb-2 inline-grid size-[22px] place-items-center rounded-full bg-(--accent-a4) text-xs font-bold text-(--accent-11)'

const dlTextClass =
  '[&_dt]:text-[11px] [&_dt]:text-(--gray-10) [&_dd]:m-0 [&_dd]:mt-0.5 [&_dd]:font-mono [&_dd]:font-bold [&_dd]:text-(--gray-12)'

const summaryBoxClass =
  '[&>div]:min-w-0 [&>div]:rounded-md [&>div]:bg-(--gray-a3) [&>div]:p-2'

const summaryListClass = `m-0 mb-3 grid grid-cols-2 gap-2 ${summaryBoxClass} ${dlTextClass}`

const phaseSummaryListClass = `m-0 grid grid-cols-1 gap-2 ${summaryBoxClass} ${dlTextClass}`

const kindListClass = `m-0 grid grid-cols-7 gap-2 max-[780px]:grid-cols-4 [&>div]:min-w-0 [&>div]:text-center ${dlTextClass}`

const kindTableClass = 'w-full border-collapse text-xs text-(--gray-11)'

const kindTableHeadClass =
  'border-t border-(--gray-a4) px-[3px] py-[5px] text-left font-medium'

const kindTableCellClass =
  'border-t border-(--gray-a4) px-[3px] py-[5px] text-right font-mono'

const filterGroupClass = 'm-0 mb-3 flex flex-wrap gap-2'

const filterLabelBaseClass =
  'inline-flex cursor-pointer items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-(--accent-8)'

const filterLabelIdleClass = `${filterLabelBaseClass} border-(--gray-a5) bg-(--gray-a2) text-(--gray-11)`

const filterLabelActiveClass = `${filterLabelBaseClass} border-(--accent-a7) bg-(--accent-a3) text-(--accent-11)`

const decisionCellClass =
  'border-t border-(--gray-a4) px-2.5 py-2 align-middle whitespace-nowrap'

const decisionHeadCellClass = `${decisionCellClass} text-left text-[11px] font-semibold text-(--gray-10)`

const decisionNumHeadCellClass = `${decisionCellClass} text-right font-mono text-[11px] font-semibold text-(--gray-10)`

const decisionRowHeadClass = `${decisionCellClass} text-left font-medium`

const decisionNumCellClass = `${decisionCellClass} text-right font-mono`

const decisionBadgeCellClass = `${decisionCellClass} text-left`

const decisionDetailCellClass =
  'border-t border-(--gray-a4) bg-(--gray-a2) px-2.5 pt-0 pb-3 whitespace-normal'

const expandButtonClass =
  'inline-flex cursor-pointer items-center gap-1.5 border-0 bg-transparent p-0 text-left text-inherit [font:inherit]'

const badgeBaseClass =
  'inline-block rounded-full border px-[7px] py-0.5 text-[11px] leading-[1.4] font-semibold'

const badgeTones = {
  amber: 'border-(--amber-a6) bg-(--amber-a3) text-(--amber-11)',
  green: 'border-(--green-a6) bg-(--green-a3) text-(--green-11)',
  blue: 'border-(--blue-a6) bg-(--blue-a3) text-(--blue-11)',
  red: 'border-(--red-a6) bg-(--red-a3) text-(--red-11)',
} as const

function badgeClass(tone: keyof typeof badgeTones): string {
  return `${badgeBaseClass} ${badgeTones[tone]}`
}

const scanBadgeTone: Record<
  ReturnType<typeof scanResultLabel>,
  keyof typeof badgeTones
> = {
  Garbage: 'amber',
  Restored: 'blue',
  'Scan root': 'green',
}

const globalChipBaseClass =
  'inline-block rounded-full border px-1.5 py-px text-[11px] leading-[1.4]'

const globalChipClass = `${globalChipBaseClass} border-(--accent-a6) bg-(--accent-a3) text-(--accent-11)`

const globalChipOverflowClass = `${globalChipBaseClass} border-(--gray-a6) bg-(--gray-a3) text-(--gray-11)`

// Paragraphs and headings inside an expanded decision row.
const detailTextClass = 'm-0 text-xs leading-[1.5] text-(--gray-11)'

const detailHeadingClass = 'm-0 mb-1.5 text-xs text-(--gray-11)'

const edgeListClass = 'm-0 flex list-none flex-col gap-1.5 p-0'

const edgeItemClass =
  'rounded-md border border-(--gray-a4) bg-(--color-panel-solid) px-2 py-1.5 text-xs leading-[1.45] text-(--gray-12) [&_code]:text-xs'

const noticeBaseClass =
  'm-0 mb-3 block rounded-md border px-2.5 py-2 text-xs leading-[1.45]'

const truncationNoticeClass = `${noticeBaseClass} border-(--amber-a6) bg-(--amber-a3) text-(--amber-11)`

const grayNoticeClass = `${noticeBaseClass} border-(--gray-a5) bg-(--gray-a3) text-(--gray-11)`

const emptyStateClass =
  'mx-auto my-16 block max-w-[520px] rounded-xl border border-(--gray-a5) bg-(--color-panel-solid) p-6 text-center'

const errorCardClass =
  'mx-auto my-16 block max-w-[520px] rounded-xl border border-(--red-a7) bg-(--color-panel-solid) p-6'

const stateHeadingClass = 'm-0 text-(--gray-12)'

const stateTextClass = 'm-0 mt-2.5'

const errorStageClass =
  'mb-1.5 block text-[11px] font-bold tracking-[0.08em] uppercase text-(--red-11)'

const spanButtonClass =
  'mt-3 cursor-pointer rounded-md border border-(--red-a7) bg-transparent px-2.5 py-1 text-xs leading-[inherit] text-(--red-11) [font-family:inherit] hover:bg-(--red-a3)'

const valueKindLabels: Record<ValueKind, string> = {
  class: 'Class',
  instance: 'Instance',
  boundMethod: 'Bound method',
  closure: 'Closure',
  array: 'Array',
  hash: 'Hash',
  integer: 'Integer',
  boolean: 'Boolean',
  string: 'String',
  null: 'Null',
  error: 'Error',
  compiledFunction: 'Compiled function',
  builtin: 'Builtin',
  other: 'Other runtime object',
}

const edgesVisitedDescription =
  'Count of heap-to-heap references visited during trial deletion. Different fields or array slots that point to the same object are counted separately. Constants, globals, and VM stack slots are not included.'

const candidatesDescription =
  'Objects whose reference count reached zero after subtracting every heap incoming edge. They are only temporary candidates; Scan may still restore some of them.'

function SnapshotCard({
  title,
  snapshot,
  kinds,
}: {
  title: string
  snapshot: HeapSnapshot
  kinds: readonly ValueKind[]
}) {
  return (
    <section className={cardClass} aria-label={`${title} heap snapshot`}>
      <h3 className={cardSubheadingClass}>{title}</h3>
      <dl className={summaryListClass}>
        <div>
          <dt>Objects</dt>
          <dd>{snapshot.objectCount}</dd>
        </div>
        <div>
          <dt>Tracked bytes</dt>
          <dd>{snapshot.trackedBytes}</dd>
        </div>
      </dl>
      <table className={kindTableClass}>
        <tbody>
          {kinds.map((kind) => (
            <tr key={kind}>
              <th scope="row" className={kindTableHeadClass}>
                {valueKindLabels[kind]}
              </th>
              <td className={kindTableCellClass}>
                {snapshot.byValueKind[kind]}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  )
}

function objectLabel(
  catalog: Map<number, GcObjectSummary>,
  id: number
): string {
  return catalog.get(id)?.label ?? `Object#${id}`
}

function globalNamesById(report: GcCollectionReport): Map<number, string[]> {
  const names = new Map<number, string[]>()
  for (const root of report.globalRoots) {
    const existing = names.get(root.objectId)
    if (existing) {
      existing.push(root.name)
    } else {
      names.set(root.objectId, [root.name])
    }
  }
  return names
}

// Hundreds of global slots may alias one object; lists of names are always
// cut off here so no row or sentence grows unbounded.
const MAX_GLOBAL_NAMES_SHOWN = 3

function GlobalNameChips({ names }: { names: string[] | undefined }) {
  if (!names || names.length === 0) {
    return null
  }
  const shown = names.slice(0, MAX_GLOBAL_NAMES_SHOWN)
  const hidden = names.length - shown.length
  return (
    <span className="ml-2 inline-flex flex-wrap gap-1 align-middle">
      {shown.map((name) => (
        <code key={name} className={globalChipClass}>
          {name}
        </code>
      ))}
      {hidden > 0 ? (
        <span
          className={globalChipOverflowClass}
          title={names.slice(MAX_GLOBAL_NAMES_SHOWN).join(', ')}
        >
          +{hidden} more
        </span>
      ) : null}
    </span>
  )
}

function trialLabel(decision: ObjectDecision['decision']): string {
  return decision === 'candidate' ? 'Candidate' : 'Survivor'
}

function finalLabel(fate: ObjectDecision['final']): string {
  return fate === 'freed' ? 'Freed' : 'Retained'
}

function sortDecisions(
  decisions: ObjectDecision[],
  involvedIds: Set<number>
): ObjectDecision[] {
  return [...decisions].sort((left, right) => {
    const leftInvolved = involvedIds.has(left.objectId) ? 0 : 1
    const rightInvolved = involvedIds.has(right.objectId) ? 0 : 1
    if (leftInvolved !== rightInvolved) {
      return leftInvolved - rightInvolved
    }
    return left.objectId - right.objectId
  })
}

function sortVisitedEdges(
  edges: VisitedEdge[],
  candidateIds: Set<number>
): VisitedEdge[] {
  const rank = (edge: VisitedEdge): number => {
    const fromCandidate = candidateIds.has(edge.fromId)
    const toCandidate = candidateIds.has(edge.toId)
    if (fromCandidate && toCandidate) {
      return 0
    }
    if (!fromCandidate && toCandidate) {
      return 1
    }
    if (fromCandidate && !toCandidate) {
      return 2
    }
    return 3
  }

  return [...edges].sort((left, right) => {
    const rankDiff = rank(left) - rank(right)
    if (rankDiff !== 0) {
      return rankDiff
    }
    if (left.fromId !== right.fromId) {
      return left.fromId - right.fromId
    }
    return left.toId - right.toId
  })
}

function RelationText({ relation }: { relation: EdgeRelation }) {
  return (
    <span className="font-mono text-(--accent-11)">
      {formatEdgeRelation(relation)}
    </span>
  )
}

function TruncationNotice({ message }: { message: string }) {
  return <output className={truncationNoticeClass}>{message}</output>
}

function WitnessPathView({
  steps,
  catalog,
}: {
  steps: { fromId: number; toId: number; relation: EdgeRelation }[]
  catalog: Map<number, GcObjectSummary>
}) {
  return (
    <div>
      <p className={detailTextClass}>
        Reachability witness: a deterministic reachability path, not the
        collector&apos;s actual event order.
      </p>
      <ol className={edgeListClass}>
        {steps.map((step, index) => (
          <li key={`${step.fromId}-${step.toId}-${index}`} className={edgeItemClass}>
            <code>{objectLabel(catalog, step.fromId)}</code>
            <span className="text-(--gray-10)">
              {' '}
              -- <RelationText relation={step.relation} /> --&gt;{' '}
            </span>
            <code>{objectLabel(catalog, step.toId)}</code>
          </li>
        ))}
      </ol>
    </div>
  )
}

function EdgeList({
  edges,
  catalog,
  emptyLabel,
}: {
  edges: VisitedEdge[]
  catalog: Map<number, GcObjectSummary>
  emptyLabel: string
}) {
  if (edges.length === 0) {
    return <p className={`m-0 ${mutedClass}`}>{emptyLabel}</p>
  }

  return (
    <ul className={edgeListClass}>
      {edges.map((edge, index) => (
        <li key={`${edge.fromId}-${edge.toId}-${index}`} className={edgeItemClass}>
          <code>{objectLabel(catalog, edge.fromId)}</code>
          <span className="text-(--gray-10)">
            {' '}
            -- <RelationText relation={edge.relation} /> --&gt;{' '}
          </span>
          <code>{objectLabel(catalog, edge.toId)}</code>
        </li>
      ))}
    </ul>
  )
}

function DecisionRowDetails({
  decision,
  report,
  catalog,
  visitedEdges,
  witnesses,
  restoredIds,
  omittedEdges,
  omittedWitnesses,
}: {
  decision: ObjectDecision
  report: GcCollectionReport
  catalog: Map<number, GcObjectSummary>
  visitedEdges: VisitedEdge[]
  witnesses: RestorationWitness[]
  restoredIds: Set<number>
  omittedEdges: number
  omittedWitnesses: number
}) {
  const label = objectLabel(catalog, decision.objectId)
  const incoming = visitedEdges.filter(
    (edge) => edge.toId === decision.objectId
  )
  const outgoing = visitedEdges.filter(
    (edge) => edge.fromId === decision.objectId
  )

  if (decision.decision === 'survivor') {
    const restoredFromHere = witnesses
      .filter((witness) => witness.rootId === decision.objectId)
      .map((witness) => objectLabel(catalog, witness.objectId))
    const globalNames = report.globalRoots
      .filter((root) => root.objectId === decision.objectId)
      .map((root) => root.name)

    return (
      <div className="flex flex-col gap-2.5 pt-2.5">
        <p className={detailTextClass}>
          Trial RC {decision.trialRefCount} is the count of direct non-heap
          references that remained after heap incoming edges were subtracted.
          This object is a Scan starting point and does not need to be restored.
        </p>
        {globalNames.length > 0 ? (
          <p className={detailTextClass}>
            Global variable{globalNames.length > 1 ? 's' : ''}{' '}
            {globalNames.slice(0, MAX_GLOBAL_NAMES_SHOWN).map((name, index) => (
              <Fragment key={name}>
                {index > 0 ? ', ' : null}
                <code>{name}</code>
              </Fragment>
            ))}
            {globalNames.length > MAX_GLOBAL_NAMES_SHOWN
              ? ` and ${globalNames.length - MAX_GLOBAL_NAMES_SHOWN} more`
              : null}{' '}
            currently reference{globalNames.length > 1 ? '' : 's'} this object;
            each named global slot is one of those non-heap references.
          </p>
        ) : null}
        <p className={detailTextClass}>
          Remaining non-heap references may come from the constants table,
          global slots, or VM stack slots. For Null-like bookkeeping objects,
          most of those references usually come from VM-prefilled stack and
          global slots.
        </p>
        {restoredFromHere.length > 0 ? (
          <div>
            <h4 className={detailHeadingClass}>
              Restored candidates from this survivor
            </h4>
            <ul className={edgeListClass}>
              {restoredFromHere.map((name) => (
                <li key={name} className={edgeItemClass}>
                  <code>{name}</code>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          <p className={detailTextClass}>
            No candidates were restored from this survivor.
          </p>
        )}
        {omittedWitnesses > 0 ? (
          <TruncationNotice
            message={`${omittedWitnesses} restoration witness details were omitted from this report.`}
          />
        ) : null}
      </div>
    )
  }

  const path = rebuildWitnessPath(witnesses, decision.objectId)
  const isRestored = restoredIds.has(decision.objectId)

  return (
    <div className="flex flex-col gap-2.5 pt-2.5">
      <div className="grid grid-cols-2 gap-3 max-[780px]:grid-cols-1">
        <div>
          <h4 className={detailHeadingClass}>Incoming reported edges</h4>
          <EdgeList
            edges={incoming}
            catalog={catalog}
            emptyLabel={`No reported edges point to ${label}.`}
          />
        </div>
        <div>
          <h4 className={detailHeadingClass}>Outgoing reported edges</h4>
          <EdgeList
            edges={outgoing}
            catalog={catalog}
            emptyLabel={`${label} has no reported outgoing edges.`}
          />
        </div>
      </div>
      {isRestored ? (
        path ? (
          <WitnessPathView steps={path} catalog={catalog} />
        ) : (
          <p className={detailTextClass}>
            This candidate was restored, but its reachability witness was
            omitted from the report.
          </p>
        )
      ) : (
        <p className={detailTextClass}>
          No path from any trial survivor. Remained in the temporary candidate
          list after Scan.
        </p>
      )}
      {omittedEdges > 0 || omittedWitnesses > 0 ? (
        <TruncationNotice
          message={[
            omittedEdges > 0
              ? `${omittedEdges} visited edge details were omitted.`
              : null,
            omittedWitnesses > 0
              ? `${omittedWitnesses} restoration witness details were omitted.`
              : null,
            `Some details for ${label} may be incomplete.`,
          ]
            .filter(Boolean)
            .join(' ')}
        />
      ) : null}
      {report.phases.trialDeletion.omittedObjectDecisions > 0 ? (
        <TruncationNotice
          message={`${report.phases.trialDeletion.omittedObjectDecisions} object decisions were omitted from this report.`}
        />
      ) : null}
    </div>
  )
}

function ObjectDecisionWalkthrough({ report }: { report: GcCollectionReport }) {
  const [filter, setFilter] = useState<DecisionFilter>('candidates')
  const [expandedId, setExpandedId] = useState<number | null>(null)

  const catalog = useMemo(
    () => new Map(report.objects.map((object) => [object.id, object])),
    [report.objects]
  )
  const namesById = useMemo(() => globalNamesById(report), [report])
  const restoredIds = useMemo(
    () =>
      new Set(report.phases.scan.restoredObjects.map((object) => object.id)),
    [report.phases.scan.restoredObjects]
  )
  const garbageIds = useMemo(
    () =>
      new Set(
        report.phases.scan.garbageCandidateObjects.map((object) => object.id)
      ),
    [report.phases.scan.garbageCandidateObjects]
  )
  const involvedIds = useMemo(() => {
    const ids = new Set<number>()
    for (const edge of report.phases.trialDeletion.visitedEdges) {
      ids.add(edge.fromId)
      ids.add(edge.toId)
    }
    for (const witness of report.phases.scan.restorationWitnesses) {
      ids.add(witness.objectId)
      ids.add(witness.rootId)
      ids.add(witness.predecessorId)
    }
    return ids
  }, [
    report.phases.trialDeletion.visitedEdges,
    report.phases.scan.restorationWitnesses,
  ])
  const candidateCount = report.phases.trialDeletion.candidates
  const reportedCandidateCount =
    report.phases.trialDeletion.objectDecisions.filter(
      (decision) => decision.decision === 'candidate'
    ).length
  const reportedSurvivorCount =
    report.phases.trialDeletion.objectDecisions.filter(
      (decision) => decision.decision === 'survivor'
    ).length
  const reportedAllCount = report.phases.trialDeletion.objectDecisions.length
  const omittedCount = report.phases.trialDeletion.omittedObjectDecisions
  const allCount = reportedAllCount + omittedCount
  const survivorCount = allCount - candidateCount
  const candidateCountLabel =
    omittedCount > 0
      ? `${reportedCandidateCount} of ${candidateCount} reported`
      : `${candidateCount}`
  const survivorCountLabel =
    omittedCount > 0
      ? `${reportedSurvivorCount} of ${survivorCount} reported`
      : `${survivorCount}`
  const allCountLabel =
    omittedCount > 0
      ? `${reportedAllCount} of ${allCount} reported`
      : `${allCount}`

  const filtered = useMemo(() => {
    const decisions = report.phases.trialDeletion.objectDecisions.filter(
      (decision) => {
        if (filter === 'candidates') {
          return decision.decision === 'candidate'
        }
        if (filter === 'survivors') {
          return decision.decision === 'survivor'
        }
        return true
      }
    )
    return sortDecisions(decisions, involvedIds)
  }, [filter, involvedIds, report.phases.trialDeletion.objectDecisions])

  const showEmptyCandidates = filter === 'candidates' && candidateCount === 0

  return (
    <section className={cardClass} aria-label="Object decision walkthrough">
      <h2 className="mb-3 text-base">Object decision walkthrough</h2>
      <p className={`mx-0 -mt-1 mb-3 ${mutedClass}`}>
        Synthetic labels distinguish runtime kinds; IDs are scoped to this
        report. Trial RC = RC before − heap incoming edges.
      </p>

      <div
        className={filterGroupClass}
        role="radiogroup"
        aria-label="Object decision filter"
      >
        <label
          className={
            filter === 'candidates' ? filterLabelActiveClass : filterLabelIdleClass
          }
        >
          <input
            type="radio"
            name="gc-decision-filter"
            value="candidates"
            className="sr-only"
            checked={filter === 'candidates'}
            aria-label={`Candidates ${candidateCountLabel}`}
            onChange={() => setFilter('candidates')}
          />
          Candidates {candidateCountLabel}
        </label>
        <label
          className={
            filter === 'survivors' ? filterLabelActiveClass : filterLabelIdleClass
          }
        >
          <input
            type="radio"
            name="gc-decision-filter"
            value="survivors"
            className="sr-only"
            checked={filter === 'survivors'}
            aria-label={`Trial survivors ${survivorCountLabel}`}
            onChange={() => setFilter('survivors')}
          />
          Trial survivors {survivorCountLabel}
        </label>
        <label
          className={
            filter === 'all' ? filterLabelActiveClass : filterLabelIdleClass
          }
        >
          <input
            type="radio"
            name="gc-decision-filter"
            value="all"
            className="sr-only"
            checked={filter === 'all'}
            aria-label={`All graph objects ${allCountLabel}`}
            onChange={() => setFilter('all')}
          />
          All graph objects {allCountLabel}
        </label>
      </div>

      {report.phases.trialDeletion.omittedObjectDecisions > 0 ? (
        <TruncationNotice
          message={`${report.phases.trialDeletion.omittedObjectDecisions} object decisions were omitted from this report.`}
        />
      ) : null}
      {report.omittedGlobalRoots > 0 ? (
        <TruncationNotice
          message={`${report.omittedGlobalRoots} named global roots were omitted from this report, so some global name chips may be missing.`}
        />
      ) : null}

      {showEmptyCandidates ? (
        <output className={grayNoticeClass}>
          No candidates in this collection. Every live object kept a positive
          trial reference count after heap incoming edges were subtracted.
        </output>
      ) : (
        <div className="overflow-x-auto [-webkit-overflow-scrolling:touch]">
          <table className="w-full min-w-[720px] border-collapse text-xs">
            <thead>
              <tr>
                <th scope="col" className={decisionHeadCellClass}>
                  Object
                </th>
                <th scope="col" className={decisionNumHeadCellClass}>
                  RC before
                </th>
                <th scope="col" className={decisionNumHeadCellClass}>
                  Heap in-edges
                </th>
                <th scope="col" className={decisionNumHeadCellClass}>
                  Trial RC
                </th>
                <th scope="col" className={decisionHeadCellClass}>
                  Trial
                </th>
                <th scope="col" className={decisionHeadCellClass}>
                  Scan
                </th>
                <th scope="col" className={decisionHeadCellClass}>
                  Final
                </th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((decision) => {
                const label = objectLabel(catalog, decision.objectId)
                const scan = scanResultLabel(decision, restoredIds, garbageIds)
                const expanded = expandedId === decision.objectId
                return (
                  <Fragment key={decision.objectId}>
                    <tr
                      className={
                        expanded ? '[&>:is(th,td)]:bg-(--accent-a2)' : undefined
                      }
                    >
                      <th scope="row" className={decisionRowHeadClass}>
                        <button
                          type="button"
                          className={expandButtonClass}
                          aria-expanded={expanded}
                          aria-label={
                            expanded
                              ? `Collapse details for ${label}`
                              : `Expand details for ${label}`
                          }
                          onClick={() =>
                            setExpandedId(expanded ? null : decision.objectId)
                          }
                        >
                          <span aria-hidden="true">{expanded ? '▾' : '▸'}</span>
                          <code className="text-xs text-(--gray-12)">
                            {label}
                          </code>
                        </button>
                        <GlobalNameChips
                          names={namesById.get(decision.objectId)}
                        />
                      </th>
                      <td className={decisionNumCellClass}>
                        {decision.refCountBefore}
                      </td>
                      <td className={decisionNumCellClass}>
                        {decision.heapIncomingEdges}
                      </td>
                      <td className={decisionNumCellClass}>
                        {decision.trialRefCount}
                      </td>
                      <td className={decisionBadgeCellClass}>
                        <span
                          className={badgeClass(
                            decision.decision === 'candidate'
                              ? 'amber'
                              : 'green'
                          )}
                        >
                          {trialLabel(decision.decision)}
                        </span>
                      </td>
                      <td className={decisionBadgeCellClass}>
                        <span className={badgeClass(scanBadgeTone[scan])}>
                          {scan}
                        </span>
                      </td>
                      <td className={decisionBadgeCellClass}>
                        <span
                          className={badgeClass(
                            decision.final === 'freed' ? 'red' : 'green'
                          )}
                        >
                          {finalLabel(decision.final)}
                        </span>
                      </td>
                    </tr>
                    {expanded ? (
                      <tr>
                        <td colSpan={7} className={decisionDetailCellClass}>
                          <DecisionRowDetails
                            decision={decision}
                            report={report}
                            catalog={catalog}
                            visitedEdges={
                              report.phases.trialDeletion.visitedEdges
                            }
                            witnesses={report.phases.scan.restorationWitnesses}
                            restoredIds={restoredIds}
                            omittedEdges={
                              report.phases.trialDeletion.omittedEdgeDetails
                            }
                            omittedWitnesses={
                              report.phases.scan.omittedWitnesses
                            }
                          />
                        </td>
                      </tr>
                    ) : null}
                  </Fragment>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}

function VisitedHeapEdges({ report }: { report: GcCollectionReport }) {
  const [edgeFilter, setEdgeFilter] = useState<EdgeFilter>('candidate-related')
  const catalog = useMemo(
    () => new Map(report.objects.map((object) => [object.id, object])),
    [report.objects]
  )
  const candidateIds = useMemo(
    () =>
      new Set(
        [
          ...report.phases.scan.restoredObjects,
          ...report.phases.scan.garbageCandidateObjects,
        ].map((object) => object.id)
      ),
    [
      report.phases.scan.restoredObjects,
      report.phases.scan.garbageCandidateObjects,
    ]
  )

  const allEdges = report.phases.trialDeletion.visitedEdges
  const candidateRelated = allEdges.filter((edge) =>
    isCandidateRelatedEdge(edge, candidateIds)
  )
  const visible =
    edgeFilter === 'candidate-related' ? candidateRelated : allEdges
  const sorted = sortVisitedEdges(visible, candidateIds)
  const visitedTotal = report.phases.trialDeletion.edgesVisited

  return (
    <details className={`group ${cardClass}`}>
      <summary className="cursor-pointer list-none group-open:mb-3 [&::-webkit-details-marker]:hidden">
        <h2 className="mb-0 inline-flex items-center gap-2 text-base before:text-xs before:text-(--gray-10) before:content-['▸'] group-open:before:content-['▾']">
          Visited heap edges
        </h2>
      </summary>
      <output className={grayNoticeClass}>
        {edgeFilter === 'candidate-related'
          ? `Showing ${candidateRelated.length} candidate-related edges of ${visitedTotal} visited`
          : `Showing ${allEdges.length} reported edges of ${visitedTotal} visited`}
      </output>

      <div
        className={filterGroupClass}
        role="radiogroup"
        aria-label="Visited heap edges filter"
      >
        <label
          className={
            edgeFilter === 'candidate-related'
              ? filterLabelActiveClass
              : filterLabelIdleClass
          }
        >
          <input
            type="radio"
            name="gc-edge-filter"
            value="candidate-related"
            className="sr-only"
            checked={edgeFilter === 'candidate-related'}
            aria-label="Candidate-related"
            onChange={() => setEdgeFilter('candidate-related')}
          />
          Candidate-related
        </label>
        <label
          className={
            edgeFilter === 'all' ? filterLabelActiveClass : filterLabelIdleClass
          }
        >
          <input
            type="radio"
            name="gc-edge-filter"
            value="all"
            className="sr-only"
            checked={edgeFilter === 'all'}
            aria-label="All visited edges"
            onChange={() => setEdgeFilter('all')}
          />
          All visited edges
        </label>
      </div>

      {report.phases.trialDeletion.omittedEdgeDetails > 0 ? (
        <TruncationNotice
          message={`${report.phases.trialDeletion.omittedEdgeDetails} visited edge details were omitted from this report.`}
        />
      ) : null}

      <EdgeList
        edges={sorted}
        catalog={catalog}
        emptyLabel="No visited heap edges were reported for this filter."
      />
    </details>
  )
}

export function GcReportView({ state, onErrorSpanSelect }: GcReportViewProps) {
  if (state.status === 'idle') {
    return (
      <div className={emptyStateClass}>
        <h2 className={stateHeadingClass}>Cycle collector</h2>
        <p className={stateTextClass}>
          Run the current source to collect unreachable Monkey object cycles.
        </p>
        <p className={`${stateTextClass} ${mutedClass}`}>
          Editing never executes the program automatically.
        </p>
      </div>
    )
  }

  if (state.status === 'running') {
    return (
      <output className={emptyStateClass} aria-live="polite">
        <h2 className={stateHeadingClass}>Running GC…</h2>
        <p className={stateTextClass}>
          The program is executing with a fixed instruction budget.
        </p>
      </output>
    )
  }

  if (state.status === 'invalid') {
    return (
      <section className={errorCardClass} role="alert">
        <span className={errorStageClass}>response error</span>
        <h2 className={stateHeadingClass}>Invalid GC response</h2>
        <pre className="whitespace-pre-wrap wrap-anywhere">{state.message}</pre>
      </section>
    )
  }

  if (state.status === 'error') {
    const { span } = state
    return (
      <section className={errorCardClass} role="alert">
        <span className={errorStageClass}>{state.stage} error</span>
        <h2 className={stateHeadingClass}>Program could not be collected</h2>
        <pre className="whitespace-pre-wrap wrap-anywhere">{state.message}</pre>
        {span !== null ? (
          onErrorSpanSelect ? (
            <button
              type="button"
              className={spanButtonClass}
              onClick={() => onErrorSpanSelect?.(span)}
            >
              Show in editor ({span.start}–{span.end})
            </button>
          ) : (
            <p className={`${stateTextClass} ${mutedClass}`}>
              Source span: {span.start}–{span.end}
            </p>
          )
        ) : null}
      </section>
    )
  }

  const { report } = state
  const collectedObjects = report.before.objectCount - report.after.objectCount
  const snapshotValueKinds = valueKinds.filter(
    (kind) =>
      report.before.byValueKind[kind] > 0 || report.after.byValueKind[kind] > 0
  )
  const collectedValueKinds = valueKinds.filter(
    (kind) => report.collectedByValueKind[kind] > 0
  )

  return (
    <div className="mx-auto flex max-w-[920px] flex-col gap-3.5">
      <section className={`${cardClass} flex items-center justify-between gap-4`}>
        <span className={eyebrowClass}>Program result</span>
        <code className="overflow-hidden text-sm text-ellipsis whitespace-nowrap text-(--green-11)">
          {state.result}
        </code>
      </section>

      <section
        className="flex items-end justify-between gap-5 rounded-[10px] border border-(--green-a7) bg-[linear-gradient(120deg,var(--green-a3),var(--color-panel-solid))] p-4.5 shadow-[0_1px_2px_var(--black-a3)] max-[640px]:flex-col max-[640px]:items-start"
        aria-label="Cycle collection summary"
      >
        <div>
          <span className={`${eyebrowClass} mb-1.5`}>Heap objects</span>
          <strong
            className="block text-[28px] leading-none text-(--green-11)"
            aria-label="Heap object count before and after collection"
          >
            {report.before.objectCount} → {report.after.objectCount}
          </strong>
        </div>
        <p className="m-0 text-right text-(--gray-11) max-[640px]:text-left">
          Collected by cycle GC:{' '}
          <strong aria-label="Collected object count">
            {collectedObjects}
          </strong>
        </p>
      </section>

      <div className="grid grid-cols-2 gap-3.5 max-[640px]:grid-cols-1">
        <SnapshotCard
          title="Before"
          snapshot={report.before}
          kinds={snapshotValueKinds}
        />
        <SnapshotCard
          title="After"
          snapshot={report.after}
          kinds={snapshotValueKinds}
        />
      </div>
      <p className={footnoteClass}>
        Heap snapshots include source values, compiled functions, constants, and
        VM bookkeeping values. Only kinds present before or after this
        collection are listed.
      </p>

      <section aria-label="Collector phase statistics">
        <h2 className={sectionHeadingClass}>Collector phases</h2>
        <div className="grid grid-cols-3 gap-3.5 max-[780px]:grid-cols-1">
          <article className={cardClass}>
            <span className={phaseIndexClass}>1</span>
            <h3 className={cardSubheadingClass}>Trial deletion</h3>
            <dl className={phaseSummaryListClass}>
              <div>
                <dt>
                  Edges visited
                  <span className={helperTextClass}>
                    Heap-to-heap references temporarily subtracted
                  </span>
                </dt>
                <dd>
                  <span
                    title={edgesVisitedDescription}
                    aria-description={edgesVisitedDescription}
                  >
                    {report.phases.trialDeletion.edgesVisited}
                  </span>
                </dd>
              </div>
              <div>
                <dt>
                  Candidates
                  <span className={helperTextClass}>
                    Trial reference count reached zero
                  </span>
                </dt>
                <dd>
                  <span
                    title={candidatesDescription}
                    aria-description={candidatesDescription}
                  >
                    {report.phases.trialDeletion.candidates}
                  </span>
                </dd>
              </div>
            </dl>
          </article>
          <article className={cardClass}>
            <span className={phaseIndexClass}>2</span>
            <h3 className={cardSubheadingClass}>Scan</h3>
            <dl className={phaseSummaryListClass}>
              <div>
                <dt>Restored</dt>
                <dd>{report.phases.scan.restored}</dd>
              </div>
              <div>
                <dt>Garbage candidates</dt>
                <dd>{report.phases.scan.garbageCandidates}</dd>
              </div>
            </dl>
          </article>
          <article className={cardClass}>
            <span className={phaseIndexClass}>3</span>
            <h3 className={cardSubheadingClass}>Free cycles</h3>
            <dl className={phaseSummaryListClass}>
              <div>
                <dt>Objects freed</dt>
                <dd>{report.phases.freeCycles.freed}</dd>
              </div>
            </dl>
          </article>
        </div>
      </section>

      <HeapGraphView report={report} />
      <PhaseReplayView report={report} />
      <ObjectDecisionWalkthrough report={report} />
      <VisitedHeapEdges report={report} />

      <section className={cardClass}>
        <h2 className={sectionHeadingClass}>Collected by value kind</h2>
        {collectedValueKinds.length > 0 ? (
          <dl className={kindListClass}>
            {collectedValueKinds.map((kind) => (
              <div key={kind}>
                <dt>{valueKindLabels[kind]}</dt>
                <dd>{report.collectedByValueKind[kind]}</dd>
              </div>
            ))}
          </dl>
        ) : (
          <p className={mutedClass}>No heap values were collected.</p>
        )}
      </section>

      <p className={footnoteClass}>
        Tracked bytes are collector accounting, not browser memory. Collection
        reclaims Monkey heap objects; WebAssembly linear memory may stay
        allocated.
      </p>
    </div>
  )
}
