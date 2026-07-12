'use client'

import { useTheme } from 'next-themes'
import { useEffect, useMemo, useRef, useState } from 'react'

import type { GcCollectionReport } from './gcReport'
import { buildHeapGraph } from './heapGraph'

// mermaid.render needs a document-unique element id per invocation.
let renderSequence = 0

type RenderState =
  | { status: 'rendering' }
  | { status: 'rendered'; svg: string }
  | { status: 'failed'; message: string }

export function HeapGraphView({ report }: { report: GcCollectionReport }) {
  const graph = useMemo(() => buildHeapGraph(report), [report])
  const { resolvedTheme } = useTheme()
  const isDark = resolvedTheme === 'dark'
  const source = graph.status === 'ok' ? graph.source : null
  const [renderState, setRenderState] = useState<RenderState>({
    status: 'rendering',
  })
  const sectionRef = useRef<HTMLElement>(null)
  // Set from an effect so server and first client render agree on "no button".
  const [fullscreenEnabled, setFullscreenEnabled] = useState(false)
  const [isFullscreen, setIsFullscreen] = useState(false)
  useEffect(() => {
    setFullscreenEnabled(Boolean(document.fullscreenEnabled))
    const onFullscreenChange = () => {
      setIsFullscreen(document.fullscreenElement === sectionRef.current)
    }
    document.addEventListener('fullscreenchange', onFullscreenChange)
    return () => {
      document.removeEventListener('fullscreenchange', onFullscreenChange)
    }
  }, [])

  const toggleFullscreen = () => {
    if (document.fullscreenElement === sectionRef.current) {
      document.exitFullscreen().catch(() => {
        // Leaving full screen failed; the fullscreenchange listener keeps
        // the button label in sync with whatever state the browser is in.
      })
    } else {
      sectionRef.current?.requestFullscreen().catch(() => {
        // The browser refused (permissions, transient state); the card
        // simply stays inline.
      })
    }
  }

  useEffect(() => {
    if (source === null) {
      return
    }
    let cancelled = false
    setRenderState({ status: 'rendering' })

    const renderGraph = async () => {
      try {
        const { default: mermaid } = await import('mermaid')
        // mermaid measures labels in a container attached to <body>, outside
        // the Radix theme scope. 'inherit' would resolve to a different font
        // there than inside this card, and labels sized with the narrower
        // font get their trailing characters clipped. Hand mermaid the
        // resolved font stack so measurement and display agree.
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
          `gc-heap-graph-${renderSequence}`,
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
    void renderGraph()

    return () => {
      cancelled = true
    }
  }, [source, isDark])

  return (
    <section
      ref={sectionRef}
      className="gc-card gc-graph-card"
      aria-label="Heap topology graph"
    >
      <div className="gc-graph-head">
        <h2>Heap topology</h2>
        {fullscreenEnabled &&
        graph.status === 'ok' &&
        renderState.status === 'rendered' ? (
          <button
            type="button"
            className="gc-graph-fullscreen-button"
            onClick={toggleFullscreen}
          >
            {isFullscreen ? 'Exit full screen' : 'Full screen'}
          </button>
        ) : null}
      </div>
      {graph.status === 'unavailable' ? (
        <p className="gc-muted">{graph.reason}</p>
      ) : (
        <>
          <ul className="gc-muted gc-graph-key">
            <li>
              Solid arrows show heap-to-heap references at collection start.
            </li>
            <li>
              Dotted arrows from External refs mark each trial survivor&apos;s
              remaining non-heap references (×N is its trial RC).
            </li>
            <li>
              The · Survivor / · Restored / · Freed suffix on each node is that
              object&apos;s fate after the collection; the arrows still show
              the topology from before it.
            </li>
          </ul>
          {renderState.status === 'rendered' ? (
            <div
              className="gc-graph-canvas"
              // mermaid output is generated from validated report data and
              // rendered under securityLevel: 'strict'.
              dangerouslySetInnerHTML={{ __html: renderState.svg }}
            />
          ) : renderState.status === 'failed' ? (
            <p className="gc-muted" role="alert">
              The graph could not be rendered: {renderState.message}
            </p>
          ) : (
            <output className="gc-muted" aria-live="polite">
              Rendering graph…
            </output>
          )}
          {graph.droppedIsolated > 0 ? (
            <p className="gc-footnote">
              {graph.droppedIsolated} object
              {graph.droppedIsolated > 1 ? 's' : ''} with no visited heap edges
              (mostly VM bookkeeping values) {graph.droppedIsolated > 1 ? 'are' : 'is'}{' '}
              not drawn.
            </p>
          ) : null}
        </>
      )}
    </section>
  )
}
