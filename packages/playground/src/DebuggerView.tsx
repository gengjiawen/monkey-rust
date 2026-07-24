'use client'

import { useEffect, useMemo, useState } from 'react'

import { DebuggerHeapGraphView } from './DebuggerHeapGraphView'
import type {
  DebuggerFrame,
  DebuggerHit,
  DebuggerRunEnvelope,
  DebuggerSlot,
  DebuggerValue,
} from './debuggerReport'
import type { SourceSpan } from './gcReport'

export type DebuggerPanelState =
  | { status: 'idle' }
  | { status: 'running' }
  | DebuggerRunEnvelope
  | { status: 'invalid'; message: string }

interface DebuggerViewProps {
  state: DebuggerPanelState
  onSpanSelect?: (span: SourceSpan | null) => void
}

const cardClass =
  'rounded-[10px] border border-(--gray-a5) bg-(--color-panel-solid) p-4 shadow-[0_1px_2px_var(--black-a3)]'

const mutedClass = 'text-xs text-(--gray-10)'

const emptyStateClass =
  'mx-auto my-16 block max-w-[520px] rounded-xl border border-(--gray-a5) bg-(--color-panel-solid) p-6 text-center'

const standaloneErrorCardClass =
  'mx-auto my-16 block max-w-[520px] rounded-xl border border-(--red-a7) bg-(--color-panel-solid) p-6'

const inlineErrorCardClass =
  'rounded-[10px] border border-(--red-a7) bg-(--color-panel-solid) p-4'

const stateHeadingClass = 'm-0 text-(--gray-12)'

const stateTextClass = 'm-0 mt-2.5'

const errorStageClass =
  'mb-1.5 block text-[11px] font-bold tracking-[0.08em] uppercase text-(--red-11)'

const spanButtonClass =
  'mt-3 cursor-pointer rounded-md border border-(--red-a7) bg-transparent px-2.5 py-1 text-xs leading-[inherit] text-(--red-11) [font-family:inherit] hover:bg-(--red-a3)'

const truncationNoticeClass =
  'm-0 block rounded-md border border-(--amber-a6) bg-(--amber-a3) px-2.5 py-2 text-xs leading-[1.45] text-(--amber-11)'

const grayNoticeClass =
  'm-0 block rounded-md border border-(--gray-a5) bg-(--gray-a3) px-2.5 py-2 text-xs leading-[1.45] text-(--gray-11)'

const sectionHeadingClass = 'm-0 mb-3 text-base text-(--gray-12)'

const subheadingClass =
  'm-0 mt-3 mb-1.5 text-[11px] font-bold tracking-[0.08em] uppercase text-(--gray-10)'

const pagerButtonClass =
  'cursor-pointer rounded-md border border-(--gray-a6) bg-transparent px-2.5 py-1 text-xs leading-[inherit] text-(--gray-11) [font-family:inherit] hover:bg-(--gray-a3) disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent'

const frameButtonClass =
  'flex w-full cursor-pointer items-baseline gap-2 rounded-md border-0 bg-transparent p-0 text-left [font-family:inherit] disabled:cursor-default'

const stackTubeClass =
  'overflow-hidden rounded-lg border border-(--gray-a6) divide-y divide-(--gray-a6)'

const stackCaptionClass =
  'm-0 font-mono text-[10px] tracking-[0.08em] uppercase text-(--gray-9)'

const frameDepthBadgeClass =
  'shrink-0 rounded bg-(--gray-a3) px-1.5 py-0.5 font-mono text-[10px] text-(--gray-10)'

const chipBaseClass =
  'rounded-full border px-2 py-0.5 font-mono text-[11px] leading-[1.4]'

const chipClass = `${chipBaseClass} cursor-pointer border-(--amber-a6) bg-(--amber-a3) text-(--amber-11) hover:bg-(--amber-a4)`

const chipPinnedClass = `${chipBaseClass} cursor-pointer border-(--amber-a8) bg-(--amber-a5) font-bold text-(--amber-11)`

const chipDisabledClass = `${chipBaseClass} cursor-not-allowed border-(--gray-a5) bg-(--gray-a3) text-(--gray-9)`

const slotBadgeClass =
  'ml-auto shrink-0 rounded bg-(--gray-a3) px-1.5 py-0.5 font-mono text-[10px] text-(--gray-10)'

const slotListClass = 'm-0 flex list-none flex-col gap-1.5 p-0'

const displayTextClass = 'font-mono text-xs break-all text-(--gray-12)'

interface RefControls {
  /** Ids the snapshot actually recorded in `heap.objects`. */
  includedIds: Set<number>
  pinnedHeapId: number | null
  onHover: (heapId: number | null) => void
  onPin: (heapId: number) => void
}

function HeapRefChip({ heapId, refs }: { heapId: number; refs: RefControls }) {
  if (!refs.includedIds.has(heapId)) {
    return (
      <button
        type="button"
        disabled
        className={chipDisabledClass}
        title="This object was omitted by the snapshot budget, so it cannot be highlighted."
      >
        ref #{heapId}
      </button>
    )
  }
  const pinned = refs.pinnedHeapId === heapId
  return (
    <button
      type="button"
      className={pinned ? chipPinnedClass : chipClass}
      aria-pressed={pinned}
      title={
        pinned
          ? 'Unpin this object in the heap graph'
          : 'Pin this object in the heap graph'
      }
      onMouseEnter={() => refs.onHover(heapId)}
      onMouseLeave={() => refs.onHover(null)}
      onFocus={() => refs.onHover(heapId)}
      onBlur={() => refs.onHover(null)}
      onClick={() => refs.onPin(heapId)}
    >
      ref #{heapId}
    </button>
  )
}

function ValueDisplay({
  value,
  refs,
}: {
  value: DebuggerValue
  refs: RefControls
}) {
  return (
    <span className="inline-flex min-w-0 flex-wrap items-baseline gap-1.5">
      <code className={displayTextClass}>{value.display}</code>
      {value.heapId !== null ? (
        <HeapRefChip heapId={value.heapId} refs={refs} />
      ) : null}
    </span>
  )
}

function SlotRow({ slot, refs }: { slot: DebuggerSlot; refs: RefControls }) {
  return (
    <li className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
      <span className="shrink-0 font-mono text-xs font-bold text-(--gray-12)">
        {slot.name}
      </span>
      {slot.value !== null ? (
        <ValueDisplay value={slot.value} refs={refs} />
      ) : (
        <em className={mutedClass}>&lt;uninitialized&gt;</em>
      )}
      <span className={slotBadgeClass}>slot {slot.slot}</span>
    </li>
  )
}

function FrameSegment({
  frame,
  depth,
  isCurrent,
  refs,
  onSpanSelect,
}: {
  frame: DebuggerFrame
  /** How many frames sit below this one; main is 0. */
  depth: number
  isCurrent: boolean
  refs: RefControls
  onSpanSelect?: (span: SourceSpan) => void
}) {
  const { currentSpan } = frame
  const linkable = currentSpan !== null && onSpanSelect !== undefined
  return (
    <section className={isCurrent ? 'bg-(--accent-a2) p-3' : 'p-3'}>
      <button
        type="button"
        className={frameButtonClass}
        disabled={!linkable}
        title={
          linkable
            ? 'Highlight where this frame is paused in the editor'
            : undefined
        }
        onClick={() => {
          if (currentSpan !== null) {
            onSpanSelect?.(currentSpan)
          }
        }}
      >
        <span className={frameDepthBadgeClass}>frame {depth}</span>
        <span className="font-mono text-sm font-bold text-(--gray-12)">
          {frame.name}
        </span>
        {isCurrent ? (
          <span className="rounded bg-(--accent-a4) px-1.5 py-0.5 text-[10px] font-bold text-(--accent-11)">
            current
          </span>
        ) : null}
        {linkable ? (
          <span className={`${mutedClass} ml-auto`}>show in editor</span>
        ) : null}
      </button>
      {frame.callee !== null ? (
        <div className="mt-2 flex flex-wrap items-baseline gap-x-2">
          <span className={mutedClass}>callee</span>
          <ValueDisplay value={frame.callee} refs={refs} />
        </div>
      ) : null}
      {frame.locals.length > 0 ? (
        <ul className={`${slotListClass} mt-2`}>
          {frame.locals.map((slot) => (
            <SlotRow key={slot.slot} slot={slot} refs={refs} />
          ))}
        </ul>
      ) : (
        <p className={`${mutedClass} m-0 mt-2`}>No locals in this frame.</p>
      )}
      {frame.captures.length > 0 ? (
        <>
          <h4 className={subheadingClass}>
            Captures ({frame.captures.length})
          </h4>
          <ul className={slotListClass}>
            {frame.captures.map((capture) => (
              <li
                key={capture.index}
                className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5"
              >
                <span className="shrink-0 font-mono text-xs font-bold text-(--gray-12)">
                  {capture.name}
                </span>
                <ValueDisplay value={capture.value} refs={refs} />
                <span className={slotBadgeClass}>free {capture.index}</span>
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {frame.temporaries.length > 0 ? (
        <details className="mt-2">
          <summary className={`${mutedClass} cursor-pointer select-none`}>
            Operand stack temporaries ({frame.temporaries.length})
          </summary>
          <ul className={`${slotListClass} mt-1.5`}>
            {frame.temporaries.map((temporary) => (
              <li
                key={temporary.slot}
                className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5"
              >
                <ValueDisplay value={temporary.value} refs={refs} />
                <span className={slotBadgeClass}>stack {temporary.slot}</span>
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </section>
  )
}

function OutputCard({
  result,
  stdout,
}: {
  result: string | null
  stdout: string
}) {
  return (
    <section className={cardClass}>
      <h2 className={sectionHeadingClass}>Program output</h2>
      {result !== null ? (
        <p className="m-0 mb-2 text-xs text-(--gray-11)">
          Result: <code className={displayTextClass}>{result}</code>
        </p>
      ) : null}
      {stdout !== '' ? (
        <pre className="m-0 rounded-md bg-(--gray-a3) p-2.5 font-mono text-xs whitespace-pre-wrap text-(--gray-12)">
          {stdout}
        </pre>
      ) : (
        <p className={`${mutedClass} m-0`}>The program printed nothing.</p>
      )}
    </section>
  )
}

function HitExplorer({
  hits,
  hitPosition,
  onSelectHit,
  result,
  stdout,
  pinnedHeapId,
  hoveredHeapId,
  onHover,
  onPin,
  onSpanSelect,
}: {
  hits: DebuggerHit[]
  hitPosition: number
  onSelectHit: (position: number) => void
  result: string | null
  stdout: string
  pinnedHeapId: number | null
  hoveredHeapId: number | null
  onHover: (heapId: number | null) => void
  onPin: (heapId: number) => void
  onSpanSelect?: (span: SourceSpan) => void
}) {
  // The reset effect runs after the render that swaps in new hits; clamp so
  // that intermediate render cannot read past a shorter hit list.
  const position = Math.min(hitPosition, hits.length - 1)
  const hit = hits[position]
  const includedIds = useMemo(
    () => new Set(hit.heap.objects.map((object) => object.id)),
    [hit]
  )
  // Innermost (current) frame first, like every debugger's stack pane;
  // the envelope stores main first.
  const displayFrames = useMemo(() => [...hit.frames].reverse(), [hit])
  const refs: RefControls = { includedIds, pinnedHeapId, onHover, onPin }

  return (
    <>
      <div className={`${cardClass} flex flex-wrap items-center gap-3 py-3`}>
        <button
          type="button"
          className={pagerButtonClass}
          aria-label="Previous hit"
          disabled={position === 0}
          onClick={() => onSelectHit(position - 1)}
        >
          ◀
        </button>
        <span className="text-sm font-bold text-(--gray-12)">
          Hit {position + 1} of {hits.length}
        </span>
        <button
          type="button"
          className={pagerButtonClass}
          aria-label="Next hit"
          disabled={position === hits.length - 1}
          onClick={() => onSelectHit(position + 1)}
        >
          ▶
        </button>
        <span className={mutedClass}>
          Switching hits highlights that <code>debugger;</code> statement in the
          editor.
        </span>
      </div>
      <div className="grid grid-cols-2 items-start gap-3 max-[1100px]:grid-cols-1">
        <div className="flex min-w-0 flex-col gap-3">
          <section aria-label="Call stack" className={`${cardClass} p-3`}>
            <h2 className={`${sectionHeadingClass} mb-2`}>
              Call stack ({displayFrames.length}{' '}
              {displayFrames.length === 1 ? 'frame' : 'frames'})
            </h2>
            <p className={`${stackCaptionClass} mb-1`}>
              ▲ top of stack · most recent call
            </p>
            <div className={stackTubeClass}>
              {displayFrames.map((frame, index) => (
                <FrameSegment
                  key={`${hit.index}-${index}`}
                  frame={frame}
                  depth={displayFrames.length - 1 - index}
                  isCurrent={index === 0}
                  refs={refs}
                  onSpanSelect={onSpanSelect}
                />
              ))}
            </div>
            <p className={`${stackCaptionClass} mt-1`}>
              ▼ stack base · program entry
            </p>
          </section>
          {hit.globals.length > 0 ? (
            <section className={cardClass}>
              <h2 className={sectionHeadingClass}>Globals</h2>
              <ul className={slotListClass}>
                {hit.globals.map((slot) => (
                  <SlotRow key={slot.slot} slot={slot} refs={refs} />
                ))}
              </ul>
            </section>
          ) : null}
        </div>
        <div className="flex min-w-0 flex-col gap-3">
          <DebuggerHeapGraphView
            hit={hit}
            highlightedHeapId={hoveredHeapId ?? pinnedHeapId}
          />
          {result !== null || stdout !== '' ? (
            <OutputCard result={result} stdout={stdout} />
          ) : null}
        </div>
      </div>
    </>
  )
}

export function DebuggerView({ state, onSpanSelect }: DebuggerViewProps) {
  const [hitPosition, setHitPosition] = useState(0)
  const [pinnedHeapId, setPinnedHeapId] = useState<number | null>(null)
  const [hoveredHeapId, setHoveredHeapId] = useState<number | null>(null)

  useEffect(() => {
    setHitPosition(0)
    setPinnedHeapId(null)
    setHoveredHeapId(null)

    let initialSpan: SourceSpan | null = null
    if (state.status === 'ok' || state.status === 'error') {
      if (state.hits.length > 0) {
        initialSpan = state.hits[0]?.span ?? null
      } else if (state.status === 'error') {
        initialSpan = state.span
      }
    }
    onSpanSelect?.(initialSpan)
  }, [onSpanSelect, state])

  if (state.status === 'idle') {
    return (
      <div className={emptyStateClass}>
        <h2 className={stateHeadingClass}>Stack &amp; heap debugger</h2>
        <p className={stateTextClass}>
          Run the current source to capture a snapshot of the call stack and
          heap at every <code>debugger;</code> statement.
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
        <h2 className={stateHeadingClass}>Running debugger…</h2>
        <p className={stateTextClass}>
          The program is executing with a fixed instruction budget.
        </p>
      </output>
    )
  }

  if (state.status === 'invalid') {
    return (
      <section className={standaloneErrorCardClass} role="alert">
        <span className={errorStageClass}>response error</span>
        <h2 className={stateHeadingClass}>Invalid debugger response</h2>
        <pre className="whitespace-pre-wrap wrap-anywhere">{state.message}</pre>
      </section>
    )
  }

  const { hits, stdout } = state
  const result = state.status === 'ok' ? state.result : null

  const selectHit = (position: number) => {
    const clamped = Math.max(0, Math.min(position, hits.length - 1))
    setHitPosition(clamped)
    setPinnedHeapId(null)
    setHoveredHeapId(null)
    const span = hits[clamped]?.span
    if (span && onSpanSelect) {
      onSpanSelect(span)
    }
  }

  const togglePin = (heapId: number) => {
    setPinnedHeapId((previous) => (previous === heapId ? null : heapId))
  }

  const errorCard =
    state.status === 'error' ? (
      <section
        className={
          hits.length === 0 && stdout === ''
            ? standaloneErrorCardClass
            : inlineErrorCardClass
        }
        role="alert"
      >
        <span className={errorStageClass}>
          {state.stage} error · {state.kind}
        </span>
        <h2 className={stateHeadingClass}>Program failed</h2>
        <pre className="whitespace-pre-wrap wrap-anywhere">{state.message}</pre>
        {state.span !== null ? (
          onSpanSelect ? (
            <button
              type="button"
              className={spanButtonClass}
              onClick={() => {
                if (state.span !== null) {
                  onSpanSelect(state.span)
                }
              }}
            >
              Show in editor ({state.span.start}–{state.span.end})
            </button>
          ) : (
            <p className={`${stateTextClass} ${mutedClass}`}>
              Source span: {state.span.start}–{state.span.end}
            </p>
          )
        ) : null}
      </section>
    ) : null

  if (hits.length === 0) {
    return (
      <div className="flex flex-col gap-3">
        {errorCard}
        {state.status === 'ok' ? (
          <div className={emptyStateClass}>
            <h2 className={stateHeadingClass}>No debugger hits</h2>
            <p className={stateTextClass}>
              The program finished without executing a <code>debugger;</code>{' '}
              statement. Add one inside the code path you want to inspect and
              run again.
            </p>
            <p className={`${stateTextClass} ${mutedClass}`}>
              Result: <code>{state.result}</code>
            </p>
          </div>
        ) : null}
        {stdout !== '' ? <OutputCard result={null} stdout={stdout} /> : null}
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      {errorCard}
      {state.status === 'error' ? (
        <p className={grayNoticeClass}>
          The snapshots below were recorded before the failure.
        </p>
      ) : null}
      {state.droppedHits > 0 ? (
        <p className={truncationNoticeClass}>
          Recording stopped after the first {hits.length} hits;{' '}
          {state.droppedHits} later <code>debugger;</code> execution
          {state.droppedHits === 1 ? ' was' : 's were'} not captured.
        </p>
      ) : null}
      <HitExplorer
        hits={hits}
        hitPosition={hitPosition}
        onSelectHit={selectHit}
        result={result}
        stdout={stdout}
        pinnedHeapId={pinnedHeapId}
        hoveredHeapId={hoveredHeapId}
        onHover={setHoveredHeapId}
        onPin={togglePin}
        onSpanSelect={onSpanSelect}
      />
    </div>
  )
}
