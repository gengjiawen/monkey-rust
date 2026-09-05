'use client'

import { useTheme } from 'next-themes'
import { useEffect, useMemo, useRef, useState } from 'react'

import type { GcCollectionReport } from './gcReport'
import { buildReplayPlan, replayStepSource } from './phaseReplay'

// mermaid.render needs a document-unique element id per invocation.
let renderSequence = 0

const PLAY_STEP_MS = 1200

const replayCardClass =
  'flex flex-col gap-2.5 rounded-[10px] border border-(--gray-a5) bg-(--color-panel-solid) p-4 shadow-[0_1px_2px_var(--black-a3)]'

const replayButtonClass =
  'cursor-pointer rounded-md border border-(--gray-a6) bg-transparent px-2.5 py-1 text-xs leading-[inherit] text-(--gray-11) [font-family:inherit] hover:bg-(--gray-a3) disabled:cursor-default disabled:opacity-50 disabled:hover:bg-transparent'

// `gc-replay-canvas` carries no styles; it is a hook for tests. Styling
// lives in the utilities below.
const replayCanvasClass = [
  'gc-replay-canvas overflow-x-auto py-1',
  // The rendered SVG carries a viewBox; keep it centered and shrink-to-fit.
  '[&_svg]:mx-auto [&_svg]:block [&_svg]:h-auto [&_svg]:max-w-full',
  // mermaid sizes each label's foreignObject with the font active at render
  // time; if the client font metrics differ even slightly, foreignObject's
  // default overflow: hidden amputates the last characters. Painting past
  // the box is the lesser evil.
  '[&_foreignObject]:overflow-visible',
  // Node state colors. Survivor / restored / external mirror HeapGraphView's
  // canvas; candidate shares the amber of the walkthrough badge, garbage
  // shares the freed red, and a freed node additionally fades out. Color is
  // never the only channel: the caption and the counts line under the
  // controls narrate every transition as text. `!` outweighs mermaid's own
  // #id-prefixed node styles.
  '[&_.node.candidate_:is(rect,path,polygon)]:fill-(--amber-a3)! [&_.node.candidate_:is(rect,path,polygon)]:stroke-(--amber-a6)! [&_.node.candidate_.nodeLabel]:text-(--amber-11)!',
  '[&_.node.survivor_:is(rect,path,polygon)]:fill-(--green-a3)! [&_.node.survivor_:is(rect,path,polygon)]:stroke-(--green-a6)! [&_.node.survivor_.nodeLabel]:text-(--green-11)!',
  '[&_.node.restored_:is(rect,path,polygon)]:fill-(--blue-a3)! [&_.node.restored_:is(rect,path,polygon)]:stroke-(--blue-a6)! [&_.node.restored_.nodeLabel]:text-(--blue-11)!',
  '[&_.node.garbage_:is(rect,path,polygon)]:fill-(--red-a3)! [&_.node.garbage_:is(rect,path,polygon)]:stroke-(--red-a6)! [&_.node.garbage_.nodeLabel]:text-(--red-11)!',
  '[&_.node.freed_:is(rect,path,polygon)]:fill-(--red-a3)! [&_.node.freed_:is(rect,path,polygon)]:stroke-(--red-a6)! [&_.node.freed_.nodeLabel]:text-(--red-11)! [&_.node.freed]:opacity-45',
  '[&_.node.external_:is(rect,path,polygon)]:fill-(--gray-a2)! [&_.node.external_:is(rect,path,polygon)]:stroke-(--gray-a8)! [&_.node.external_:is(rect,path,polygon)]:[stroke-dasharray:4_3]! [&_.node.external_.nodeLabel]:text-(--gray-11)!',
  // The node(s) the current step narrates get a heavier outline.
  '[&_.node.active_:is(rect,path,polygon)]:[stroke-width:2.5px]!',
].join(' ')

const mutedClass = 'text-xs text-(--gray-10)'

const metaClass = 'm-0 text-xs text-(--gray-11)'

const captionClass =
  'block min-h-[3em] text-[13px] leading-normal text-(--gray-12)'

type RenderState =
  | { status: 'rendering' }
  | { status: 'rendered'; svg: string }
  | { status: 'failed'; message: string }

export function PhaseReplayView({ report }: { report: GcCollectionReport }) {
  const plan = useMemo(() => buildReplayPlan(report), [report])
  const { resolvedTheme } = useTheme()
  const isDark = resolvedTheme === 'dark'
  const [stepIndex, setStepIndex] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [renderState, setRenderState] = useState<RenderState>({
    status: 'rendering',
  })
  const sectionRef = useRef<HTMLElement>(null)

  const stepCount = plan.status === 'ok' ? plan.steps.length : 0
  const boundedIndex = Math.min(stepIndex, Math.max(0, stepCount - 1))
  const step = plan.status === 'ok' ? plan.steps[boundedIndex] : null
  const source =
    plan.status === 'ok' && step ? replayStepSource(plan.model, step) : null

  // A new report means a new plan: rewind to the first step.
  useEffect(() => {
    setStepIndex(0)
    setPlaying(false)
  }, [plan])

  useEffect(() => {
    if (!playing) {
      return
    }
    if (boundedIndex >= stepCount - 1) {
      setPlaying(false)
      return
    }
    const timer = window.setTimeout(() => {
      setStepIndex((current) => Math.min(current + 1, stepCount - 1))
    }, PLAY_STEP_MS)
    return () => {
      window.clearTimeout(timer)
    }
  }, [playing, boundedIndex, stepCount])

  useEffect(() => {
    if (source === null) {
      return
    }
    let cancelled = false
    // Keep the previous frame visible while the next one renders; resetting
    // to a placeholder would flash the card on every step.
    setRenderState((previous) =>
      previous.status === 'rendered' ? previous : { status: 'rendering' }
    )

    const renderFrame = async () => {
      try {
        const { default: mermaid } = await import('mermaid')
        // Same trick as HeapGraphView: mermaid measures labels outside the
        // Radix theme scope, so hand it the resolved font stack to keep
        // measurement and display in agreement.
        const fontFamily = sectionRef.current
          ? getComputedStyle(sectionRef.current).fontFamily
          : ''
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: 'strict',
          theme: isDark ? 'dark' : 'default',
          ...(fontFamily ? { fontFamily } : {}),
        })
        renderSequence += 1
        const { svg } = await mermaid.render(
          `gc-phase-replay-${renderSequence}`,
          source
        )
        if (!cancelled) {
          setRenderState({ status: 'rendered', svg })
        }
      } catch (error) {
        if (!cancelled) {
          setRenderState({
            status: 'failed',
            message: error instanceof Error ? error.message : String(error),
          })
        }
      }
    }
    void renderFrame()

    return () => {
      cancelled = true
    }
  }, [source, isDark])

  const goTo = (index: number) => {
    setPlaying(false)
    setStepIndex(Math.max(0, Math.min(index, stepCount - 1)))
  }

  const togglePlay = () => {
    if (playing) {
      setPlaying(false)
      return
    }
    if (boundedIndex >= stepCount - 1) {
      setStepIndex(0)
    }
    setPlaying(true)
  }

  return (
    <section
      ref={sectionRef}
      className={replayCardClass}
      aria-label="Phase replay"
    >
      <h2 className="m-0 text-base text-(--gray-12)">Phase replay</h2>
      {plan.status === 'unavailable' ? (
        <p className={mutedClass}>{plan.reason}</p>
      ) : step === null ? null : (
        <>
          <p className={`m-0 ${mutedClass}`}>
            Deterministic replay reconstructed from the report&apos;s sorted
            edges and witness paths — not the collector&apos;s actual event
            order. The real collector ran all three phases atomically before
            anything was displayed.
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              className={replayButtonClass}
              onClick={togglePlay}
            >
              {playing
                ? 'Pause'
                : boundedIndex >= stepCount - 1
                  ? 'Replay'
                  : 'Play'}
            </button>
            <button
              type="button"
              className={replayButtonClass}
              onClick={() => goTo(boundedIndex - 1)}
              disabled={boundedIndex === 0}
            >
              Back
            </button>
            <button
              type="button"
              className={replayButtonClass}
              onClick={() => goTo(boundedIndex + 1)}
              disabled={boundedIndex >= stepCount - 1}
            >
              Next
            </button>
            <input
              type="range"
              className="min-w-[140px] flex-1"
              min={0}
              max={stepCount - 1}
              step={1}
              value={boundedIndex}
              onChange={(event) => goTo(Number(event.target.value))}
              aria-label="Replay step"
            />
            <span className={metaClass}>
              Step {boundedIndex + 1} of {stepCount} · {step.phase}
            </span>
          </div>
          <output className={captionClass} aria-live="polite">
            {step.caption}
          </output>
          <p className={metaClass}>
            Candidates {step.counts.candidates} · Restored{' '}
            {step.counts.restored} · Garbage {step.counts.garbage} · Freed{' '}
            {step.counts.freed}
          </p>
          {renderState.status === 'rendered' ? (
            <div
              className={replayCanvasClass}
              // mermaid output is generated from validated report data and
              // rendered under securityLevel: 'strict'.
              dangerouslySetInnerHTML={{ __html: renderState.svg }}
            />
          ) : renderState.status === 'failed' ? (
            <p className={mutedClass} role="alert">
              The replay frame could not be rendered: {renderState.message}
            </p>
          ) : (
            <output className={mutedClass} aria-live="polite">
              Rendering replay…
            </output>
          )}
        </>
      )}
    </section>
  )
}
