import type { GcRunEnvelope, HeapSnapshot, ValueKind } from './gcReport'
import { valueKinds } from './gcReport'

export type GcPanelState =
  | { status: 'idle' }
  | { status: 'running' }
  | GcRunEnvelope
  | { status: 'invalid'; message: string }

interface GcReportViewProps {
  state: GcPanelState
}

const valueKindLabels: Record<ValueKind, string> = {
  class: 'Class',
  instance: 'Instance',
  boundMethod: 'Bound method',
  closure: 'Closure',
  array: 'Array',
  hash: 'Hash',
  other: 'Other',
}

function SnapshotCard({
  title,
  snapshot,
}: {
  title: string
  snapshot: HeapSnapshot
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
          {valueKinds.map((kind) => (
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

export function GcReportView({ state }: GcReportViewProps) {
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
    return (
      <section className="gc-error" role="alert">
        <span className="gc-stage">{state.stage} error</span>
        <h2>Program could not be collected</h2>
        <pre>{state.message}</pre>
        {state.span ? (
          <p className="gc-muted">
            Source span: {state.span.start}–{state.span.end}
          </p>
        ) : null}
      </section>
    )
  }

  const { report } = state
  const beforeInstances = report.before.byValueKind.instance
  const afterInstances = report.after.byValueKind.instance
  const collectedInstances = report.collectedByValueKind.instance

  return (
    <div className="gc-report">
      <section className="gc-card gc-result-card">
        <span className="gc-eyebrow">Program result</span>
        <code>{state.result}</code>
      </section>

      <section
        className="gc-cycle-highlight"
        aria-label="Instance cycle summary"
      >
        <div>
          <span className="gc-eyebrow">Instance</span>
          <strong aria-label="Instance count before and after collection">
            {beforeInstances} → {afterInstances}
          </strong>
        </div>
        <p>
          Collected by cycle GC:{' '}
          <strong aria-label="Collected instance count">
            {collectedInstances}
          </strong>
        </p>
      </section>

      <div className="gc-snapshot-grid">
        <SnapshotCard title="Before" snapshot={report.before} />
        <SnapshotCard title="After" snapshot={report.after} />
      </div>

      <section className="gc-section" aria-label="Collector phase statistics">
        <h2>Collector phases</h2>
        <div className="gc-phase-grid">
          <article className="gc-card">
            <span className="gc-phase-index">1</span>
            <h3>Trial deletion</h3>
            <dl className="gc-summary-list">
              <div>
                <dt>Edges visited</dt>
                <dd>{report.phases.trialDeletion.edgesVisited}</dd>
              </div>
              <div>
                <dt>Candidates</dt>
                <dd>{report.phases.trialDeletion.candidates}</dd>
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

      <section className="gc-card gc-collected-card">
        <h2>Collected by value kind</h2>
        <dl className="gc-kind-list">
          {valueKinds.map((kind) => (
            <div key={kind}>
              <dt>{valueKindLabels[kind]}</dt>
              <dd>{report.collectedByValueKind[kind]}</dd>
            </div>
          ))}
        </dl>
      </section>

      <p className="gc-footnote">
        Tracked bytes are collector accounting, not browser memory. Collection
        reclaims Monkey heap objects; WebAssembly linear memory may stay
        allocated.
      </p>
    </div>
  )
}
