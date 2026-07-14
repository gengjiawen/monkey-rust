'use client'

import { Button, SegmentedControl } from '@radix-ui/themes'
import { useCallback } from 'react'

import type { SourceSpan } from './gcReport'
import {
  formatByteOffset,
  groupRegionsBySection,
  regionHex,
  snapshotSectionTitles,
  type SnapshotBuildEnvelope,
  type SnapshotBuildSuccess,
  type SnapshotRunEnvelope,
  type SnapshotSection,
} from './snapshot'

export type SnapshotBuildState =
  | { status: 'idle' }
  | SnapshotBuildEnvelope
  | { status: 'invalid'; message: string }

export type SnapshotRunState =
  | { status: 'idle' }
  | { status: 'running' }
  | SnapshotRunEnvelope
  | { status: 'invalid'; message: string }

interface SnapshotViewProps {
  build: SnapshotBuildState
  run: SnapshotRunState
  stripDebug: boolean
  onStripDebugChange: (stripDebug: boolean) => void
  onErrorSpanSelect?: (span: SourceSpan) => void
}

const cardClass =
  'rounded-[10px] border border-(--gray-a5) bg-(--color-panel-solid) p-4 shadow-[0_1px_2px_var(--black-a3)]'

const mutedClass = 'text-xs text-(--gray-10)'

const sectionHeadingClass = 'm-0 mb-3 text-base text-(--gray-12)'

const alertClass =
  'm-0 rounded-md border border-(--red-a6) bg-(--red-a3) px-3 py-2 text-sm text-(--red-11)'

const dlTextClass =
  '[&_dt]:text-[11px] [&_dt]:text-(--gray-10) [&_dd]:m-0 [&_dd]:mt-0.5 [&_dd]:font-mono [&_dd]:font-bold [&_dd]:text-(--gray-12)'

const summaryListClass = `m-0 mb-3 grid grid-cols-4 gap-2 max-[900px]:grid-cols-2 [&>div]:min-w-0 [&>div]:rounded-md [&>div]:bg-(--gray-a3) [&>div]:p-2 ${dlTextClass}`

const sectionDotTones: Record<SnapshotSection, string> = {
  header: 'bg-(--blue-9)',
  main: 'bg-(--green-9)',
  constants: 'bg-(--amber-9)',
  debug: 'bg-(--violet-9)',
}

const sectionHelp: Record<SnapshotSection, string> = {
  header: 'Fixed 10 bytes: magic, version, ABI fingerprint, flags.',
  main: 'Length-prefixed top-level instruction stream.',
  constants: 'Tagged constant pool: integers, strings, functions.',
  debug: 'Optional pc→source-span tables; gone with --strip.',
}

function downloadSnapshot(bytes: Uint8Array<ArrayBuffer>) {
  const blob = new Blob([bytes], { type: 'application/octet-stream' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = 'program.mbc'
  anchor.click()
  URL.revokeObjectURL(url)
}

function SnapshotSummary({
  build,
  stripDebug,
  onStripDebugChange,
}: {
  build: SnapshotBuildSuccess
  stripDebug: boolean
  onStripDebugChange: (stripDebug: boolean) => void
}) {
  const handleDownload = useCallback(() => {
    downloadSnapshot(build.bytes)
  }, [build.bytes])

  return (
    <section className={cardClass}>
      <h2 className={sectionHeadingClass}>Snapshot (.mbc)</h2>
      <dl className={summaryListClass}>
        <div>
          <dt>Size</dt>
          <dd aria-label="Snapshot size">{build.layout.byteLength} bytes</dd>
        </div>
        <div>
          <dt>Format version</dt>
          <dd aria-label="Snapshot format version">
            {build.layout.formatVersion}
          </dd>
        </div>
        <div>
          <dt>ABI fingerprint</dt>
          <dd aria-label="Snapshot ABI fingerprint">
            {build.layout.abiFingerprint}
          </dd>
        </div>
        <div>
          <dt>Debug info</dt>
          <dd aria-label="Snapshot debug info">
            {build.layout.hasDebugInfo ? 'included' : 'stripped'}
          </dd>
        </div>
      </dl>
      <div className="flex flex-wrap items-center gap-3">
        <SegmentedControl.Root
          size="1"
          value={stripDebug ? 'stripped' : 'debug'}
          onValueChange={(value) => onStripDebugChange(value === 'stripped')}
        >
          <SegmentedControl.Item value="debug">
            Keep debug info
          </SegmentedControl.Item>
          <SegmentedControl.Item value="stripped">
            Stripped
          </SegmentedControl.Item>
        </SegmentedControl.Root>
        <Button size="1" variant="soft" onClick={handleDownload}>
          Download .mbc
        </Button>
      </div>
      <p className={`${mutedClass} mx-0 mt-3 mb-0`}>
        These are the exact bytes <code>monkey-gc compile</code> writes.
        Download the file and run <code>monkey-gc run program.mbc</code>{' '}
        locally — no parser or compiler involved.
      </p>
    </section>
  )
}

function SnapshotHexdump({ build }: { build: SnapshotBuildSuccess }) {
  const groups = groupRegionsBySection(build.layout.regions)

  return (
    <section className={cardClass} aria-label="Annotated snapshot bytes">
      <h2 className={sectionHeadingClass}>Byte layout</h2>
      {groups.map((group, groupIndex) => (
        <div key={`${group.section}-${groupIndex}`} className="mb-4 last:mb-0">
          <h3 className="m-0 mb-0.5 flex items-center gap-2 text-sm text-(--gray-12)">
            <span
              aria-hidden
              className={`inline-block size-2 rounded-full ${sectionDotTones[group.section]}`}
            />
            {snapshotSectionTitles[group.section]}
          </h3>
          <p className={`${mutedClass} mx-0 mt-0 mb-2`}>
            {sectionHelp[group.section]}
          </p>
          <div className="font-mono text-xs leading-relaxed">
            {group.regions.map((region) => (
              <div
                key={region.offset}
                className="grid grid-cols-[3.25rem_minmax(6rem,11rem)_1fr] items-baseline gap-x-3 border-t border-(--gray-a3) py-1 first:border-t-0"
              >
                <span className="text-(--gray-9)">
                  {formatByteOffset(region.offset)}
                </span>
                <span className="break-words text-(--gray-11)">
                  {regionHex(build.bytes, region)}
                </span>
                <span className="font-sans">
                  <span className="font-semibold text-(--gray-12)">
                    {region.label}
                  </span>{' '}
                  <span className={mutedClass}>{region.detail}</span>
                </span>
              </div>
            ))}
          </div>
        </div>
      ))}
    </section>
  )
}

function SnapshotRunResult({
  build,
  run,
  onErrorSpanSelect,
}: {
  build: SnapshotBuildSuccess
  run: SnapshotRunState
  onErrorSpanSelect?: (span: SourceSpan) => void
}) {
  return (
    <section className={cardClass}>
      <h2 className={sectionHeadingClass}>Run from snapshot</h2>
      {run.status === 'idle' ? (
        <p className={`${mutedClass} m-0`}>
          Run snapshot executes the bytes above on the GC VM — the same path
          as <code>monkey-gc run program.mbc</code>. The source editor is not
          consulted.
        </p>
      ) : null}
      {run.status === 'running' ? (
        <p className={`${mutedClass} m-0`}>Running…</p>
      ) : null}
      {run.status === 'ok' ? (
        <output
          aria-label="Snapshot run result"
          className="block rounded-md bg-(--gray-a3) p-2 font-mono text-sm text-(--gray-12)"
        >
          {run.result}
        </output>
      ) : null}
      {run.status === 'error' ? (
        <div>
          <p role="alert" className={alertClass}>
            {run.stage === 'snapshot' ? 'snapshot rejected' : 'runtime error'}:{' '}
            {run.message}
          </p>
          {run.span !== null ? (
            <Button
              size="1"
              variant="soft"
              className="mt-2"
              onClick={() => {
                const { span } = run
                if (span !== null) {
                  onErrorSpanSelect?.(span)
                }
              }}
            >
              Show in editor ({run.span.start}–{run.span.end})
            </Button>
          ) : null}
          {run.stage === 'runtime' &&
          run.span === null &&
          !build.layout.hasDebugInfo ? (
            <p className={`${mutedClass} mx-0 mt-2 mb-0`}>
              Stripped snapshots drop the pc→span table, so this error has no
              source location. Switch to “Keep debug info” to get one.
            </p>
          ) : null}
        </div>
      ) : null}
      {run.status === 'invalid' ? (
        <p role="alert" className={alertClass}>
          {run.message}
        </p>
      ) : null}
    </section>
  )
}

export function SnapshotView({
  build,
  run,
  stripDebug,
  onStripDebugChange,
  onErrorSpanSelect,
}: SnapshotViewProps) {
  if (build.status === 'idle') {
    return <p className={`${mutedClass} m-0`}>Compiling snapshot…</p>
  }

  if (build.status === 'error' || build.status === 'invalid') {
    const heading =
      build.status === 'error' ? `${build.stage} error` : 'unexpected response'
    return (
      <p role="alert" className={alertClass}>
        {heading}: {build.message}
      </p>
    )
  }

  return (
    <div className="grid gap-4">
      <SnapshotSummary
        build={build}
        stripDebug={stripDebug}
        onStripDebugChange={onStripDebugChange}
      />
      <SnapshotRunResult
        build={build}
        run={run}
        onErrorSpanSelect={onErrorSpanSelect}
      />
      <SnapshotHexdump build={build} />
    </div>
  )
}
