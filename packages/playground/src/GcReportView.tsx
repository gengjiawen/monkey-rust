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
    <section
      className="gc-card gc-snapshot"
      aria-label={`${title} heap snapshot`}
    >
      <h3>{title}</h3>
      <dl className="gc-summary-list">
        <div>
          <dt>Objects</dt>
          <dd>{snapshot.objectCount}</dd>
        </div>
        <div>
          <dt>Tracked bytes</dt>
          <dd>{snapshot.trackedBytes}</dd>
        </div>
      </dl>
      <table className="gc-kind-table">
        <tbody>
          {kinds.map((kind) => (
            <tr key={kind}>
              <th scope="row">{valueKindLabels[kind]}</th>
              <td>{snapshot.byValueKind[kind]}</td>
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

function GlobalNameChips({ names }: { names: string[] | undefined }) {
  if (!names || names.length === 0) {
    return null
  }
  return (
    <span className="gc-global-names">
      {names.map((name) => (
        <code key={name} className="gc-global-name">
          {name}
        </code>
      ))}
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
    <span className="gc-edge-relation">{formatEdgeRelation(relation)}</span>
  )
}

function TruncationNotice({ message }: { message: string }) {
  return <output className="gc-truncation-notice">{message}</output>
}

function WitnessPathView({
  steps,
  catalog,
}: {
  steps: { fromId: number; toId: number; relation: EdgeRelation }[]
  catalog: Map<number, GcObjectSummary>
}) {
  return (
    <div className="gc-witness-path">
      <p className="gc-muted">
        Reachability witness: a deterministic reachability path, not the
        collector&apos;s actual event order.
      </p>
      <ol className="gc-edge-list">
        {steps.map((step, index) => (
          <li key={`${step.fromId}-${step.toId}-${index}`}>
            <code>{objectLabel(catalog, step.fromId)}</code>
            <span className="gc-edge-arrow">
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
    return <p className="gc-muted">{emptyLabel}</p>
  }

  return (
    <ul className="gc-edge-list">
      {edges.map((edge, index) => (
        <li key={`${edge.fromId}-${edge.toId}-${index}`}>
          <code>{objectLabel(catalog, edge.fromId)}</code>
          <span className="gc-edge-arrow">
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
      <div className="gc-decision-details">
        <p>
          Trial RC {decision.trialRefCount} is the count of direct non-heap
          references that remained after heap incoming edges were subtracted.
          This object is a Scan starting point and does not need to be restored.
        </p>
        {globalNames.length > 0 ? (
          <p>
            Global variable{globalNames.length > 1 ? 's' : ''}{' '}
            {globalNames.map((name, index) => (
              <Fragment key={name}>
                {index > 0 ? ', ' : null}
                <code>{name}</code>
              </Fragment>
            ))}{' '}
            currently reference{globalNames.length > 1 ? '' : 's'} this object;
            each named global slot is one of those non-heap references.
          </p>
        ) : null}
        <p className="gc-muted">
          Remaining non-heap references may come from the constants table,
          global slots, or VM stack slots. For Null-like bookkeeping objects,
          most of those references usually come from VM-prefilled stack and
          global slots.
        </p>
        {restoredFromHere.length > 0 ? (
          <div>
            <h4>Restored candidates from this survivor</h4>
            <ul className="gc-detail-list">
              {restoredFromHere.map((name) => (
                <li key={name}>
                  <code>{name}</code>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          <p className="gc-muted">
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
    <div className="gc-decision-details">
      <div className="gc-detail-columns">
        <div>
          <h4>Incoming reported edges</h4>
          <EdgeList
            edges={incoming}
            catalog={catalog}
            emptyLabel={`No reported edges point to ${label}.`}
          />
        </div>
        <div>
          <h4>Outgoing reported edges</h4>
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
          <p className="gc-muted">
            This candidate was restored, but its reachability witness was
            omitted from the report.
          </p>
        )
      ) : (
        <p>
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
    <section
      className="gc-card gc-walkthrough-card"
      aria-label="Object decision walkthrough"
    >
      <h2>Object decision walkthrough</h2>
      <p className="gc-muted">
        Synthetic labels distinguish runtime kinds; IDs are scoped to this
        report. Trial RC = RC before − heap incoming edges.
      </p>

      <div
        className="gc-filter-group"
        role="radiogroup"
        aria-label="Object decision filter"
      >
        <label className={filter === 'candidates' ? 'is-active' : undefined}>
          <input
            type="radio"
            name="gc-decision-filter"
            value="candidates"
            checked={filter === 'candidates'}
            aria-label={`Candidates ${candidateCountLabel}`}
            onChange={() => setFilter('candidates')}
          />
          Candidates {candidateCountLabel}
        </label>
        <label className={filter === 'survivors' ? 'is-active' : undefined}>
          <input
            type="radio"
            name="gc-decision-filter"
            value="survivors"
            checked={filter === 'survivors'}
            aria-label={`Trial survivors ${survivorCountLabel}`}
            onChange={() => setFilter('survivors')}
          />
          Trial survivors {survivorCountLabel}
        </label>
        <label className={filter === 'all' ? 'is-active' : undefined}>
          <input
            type="radio"
            name="gc-decision-filter"
            value="all"
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

      {showEmptyCandidates ? (
        <output className="gc-empty-details">
          No candidates in this collection. Every live object kept a positive
          trial reference count after heap incoming edges were subtracted.
        </output>
      ) : (
        <div className="gc-decision-table-wrap">
          <table className="gc-decision-table">
            <thead>
              <tr>
                <th scope="col">Object</th>
                <th scope="col">RC before</th>
                <th scope="col">Heap in-edges</th>
                <th scope="col">Trial RC</th>
                <th scope="col">Trial</th>
                <th scope="col">Scan</th>
                <th scope="col">Final</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((decision) => {
                const label = objectLabel(catalog, decision.objectId)
                const scan = scanResultLabel(decision, restoredIds, garbageIds)
                const expanded = expandedId === decision.objectId
                return (
                  <Fragment key={decision.objectId}>
                    <tr className={expanded ? 'is-expanded' : undefined}>
                      <th scope="row">
                        <button
                          type="button"
                          className="gc-expand-button"
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
                          <code>{label}</code>
                        </button>
                        <GlobalNameChips
                          names={namesById.get(decision.objectId)}
                        />
                      </th>
                      <td>{decision.refCountBefore}</td>
                      <td>{decision.heapIncomingEdges}</td>
                      <td>{decision.trialRefCount}</td>
                      <td>
                        <span
                          className={`gc-badge gc-badge-${decision.decision}`}
                        >
                          {trialLabel(decision.decision)}
                        </span>
                      </td>
                      <td>
                        <span
                          className={`gc-badge gc-badge-scan-${scan
                            .toLowerCase()
                            .replace(' ', '-')}`}
                        >
                          {scan}
                        </span>
                      </td>
                      <td>
                        <span className={`gc-badge gc-badge-${decision.final}`}>
                          {finalLabel(decision.final)}
                        </span>
                      </td>
                    </tr>
                    {expanded ? (
                      <tr className="gc-decision-detail-row">
                        <td colSpan={7}>
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
    <details className="gc-card gc-edges-card">
      <summary>
        <h2>Visited heap edges</h2>
      </summary>
      <output className="gc-edges-summary">
        {edgeFilter === 'candidate-related'
          ? `Showing ${candidateRelated.length} candidate-related edges of ${visitedTotal} visited`
          : `Showing ${allEdges.length} reported edges of ${visitedTotal} visited`}
      </output>

      <div
        className="gc-filter-group"
        role="radiogroup"
        aria-label="Visited heap edges filter"
      >
        <label
          className={
            edgeFilter === 'candidate-related' ? 'is-active' : undefined
          }
        >
          <input
            type="radio"
            name="gc-edge-filter"
            value="candidate-related"
            checked={edgeFilter === 'candidate-related'}
            aria-label="Candidate-related"
            onChange={() => setEdgeFilter('candidate-related')}
          />
          Candidate-related
        </label>
        <label className={edgeFilter === 'all' ? 'is-active' : undefined}>
          <input
            type="radio"
            name="gc-edge-filter"
            value="all"
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
      <div className="gc-empty-state">
        <h2>Cycle collector</h2>
        <p>
          Run the current source to collect unreachable Monkey object cycles.
        </p>
        <p className="gc-muted">
          Editing never executes the program automatically.
        </p>
      </div>
    )
  }

  if (state.status === 'running') {
    return (
      <output className="gc-empty-state" aria-live="polite">
        <h2>Running GC…</h2>
        <p>The program is executing with a fixed instruction budget.</p>
      </output>
    )
  }

  if (state.status === 'invalid') {
    return (
      <section className="gc-error" role="alert">
        <span className="gc-stage">response error</span>
        <h2>Invalid GC response</h2>
        <pre>{state.message}</pre>
      </section>
    )
  }

  if (state.status === 'error') {
    const { span } = state
    return (
      <section className="gc-error" role="alert">
        <span className="gc-stage">{state.stage} error</span>
        <h2>Program could not be collected</h2>
        <pre>{state.message}</pre>
        {span !== null ? (
          onErrorSpanSelect ? (
            <button
              type="button"
              className="gc-span-button"
              onClick={() => onErrorSpanSelect?.(span)}
            >
              Show in editor ({span.start}–{span.end})
            </button>
          ) : (
            <p className="gc-muted">
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
    <div className="gc-report">
      <section className="gc-card gc-result-card">
        <span className="gc-eyebrow">Program result</span>
        <code>{state.result}</code>
      </section>

      <section
        className="gc-cycle-highlight"
        aria-label="Cycle collection summary"
      >
        <div>
          <span className="gc-eyebrow">Heap objects</span>
          <strong aria-label="Heap object count before and after collection">
            {report.before.objectCount} → {report.after.objectCount}
          </strong>
        </div>
        <p>
          Collected by cycle GC:{' '}
          <strong aria-label="Collected object count">
            {collectedObjects}
          </strong>
        </p>
      </section>

      <div className="gc-snapshot-grid">
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
      <p className="gc-footnote">
        Heap snapshots include source values, compiled functions, constants, and
        VM bookkeeping values. Only kinds present before or after this
        collection are listed.
      </p>

      <section className="gc-section" aria-label="Collector phase statistics">
        <h2>Collector phases</h2>
        <div className="gc-phase-grid">
          <article className="gc-card">
            <span className="gc-phase-index">1</span>
            <h3>Trial deletion</h3>
            <dl className="gc-summary-list">
              <div>
                <dt>
                  Edges visited
                  <span className="gc-helper-text">
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
                  <span className="gc-helper-text">
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
          <article className="gc-card">
            <span className="gc-phase-index">2</span>
            <h3>Scan</h3>
            <dl className="gc-summary-list">
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
          <article className="gc-card">
            <span className="gc-phase-index">3</span>
            <h3>Free cycles</h3>
            <dl className="gc-summary-list">
              <div>
                <dt>Objects freed</dt>
                <dd>{report.phases.freeCycles.freed}</dd>
              </div>
            </dl>
          </article>
        </div>
      </section>

      <HeapGraphView report={report} />
      <ObjectDecisionWalkthrough report={report} />
      <VisitedHeapEdges report={report} />

      <section className="gc-card gc-collected-card">
        <h2>Collected by value kind</h2>
        {collectedValueKinds.length > 0 ? (
          <dl className="gc-kind-list">
            {collectedValueKinds.map((kind) => (
              <div key={kind}>
                <dt>{valueKindLabels[kind]}</dt>
                <dd>{report.collectedByValueKind[kind]}</dd>
              </div>
            ))}
          </dl>
        ) : (
          <p className="gc-muted">No heap values were collected.</p>
        )}
      </section>

      <p className="gc-footnote">
        Tracked bytes are collector accounting, not browser memory. Collection
        reclaims Monkey heap objects; WebAssembly linear memory may stay
        allocated.
      </p>
    </div>
  )
}
