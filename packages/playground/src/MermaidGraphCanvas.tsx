'use client'

import { useTheme } from 'next-themes'
import { useEffect, useRef, useState, type ReactNode } from 'react'

// mermaid.render needs a document-unique element id per invocation; the
// counter is shared by every canvas on the page.
let renderSequence = 0

// `gc-graph-card` / `gc-graph-canvas` carry no styles; they are hooks for
// tests (and the fullscreen target). Styling lives in the utilities below.
const graphCardClass =
  'gc-graph-card group flex flex-col gap-2.5 rounded-[10px] border border-(--gray-a5) bg-(--color-panel-solid) p-4 shadow-[0_1px_2px_var(--black-a3)] [&:fullscreen]:overflow-auto [&:fullscreen]:rounded-none [&:fullscreen]:border-0 [&:fullscreen]:bg-(--color-panel-solid) [&:fullscreen]:p-6'

const graphButtonClass =
  'cursor-pointer rounded-md border border-(--gray-a6) bg-transparent px-2.5 py-1 text-xs leading-[inherit] text-(--gray-11) [font-family:inherit] hover:bg-(--gray-a3)'

const baseCanvasClass = [
  'gc-graph-canvas overflow-x-auto py-1',
  // The rendered SVG carries a viewBox; in fullscreen it may fill the card.
  // `!` beats the inline max-width mermaid writes onto the SVG root.
  '[&_svg]:mx-auto [&_svg]:block [&_svg]:h-auto [&_svg]:max-w-full',
  '[:fullscreen_&]:flex [:fullscreen_&]:min-h-0 [:fullscreen_&]:flex-1 [:fullscreen_&]:items-center [:fullscreen_&]:justify-center',
  '[:fullscreen_&_svg]:h-full [:fullscreen_&_svg]:w-full [:fullscreen_&_svg]:max-w-none!',
  // mermaid sizes each label's foreignObject with the font active at render
  // time; if the client font metrics differ even slightly, foreignObject's
  // default overflow: hidden amputates the last characters. Painting past
  // the box is the lesser evil.
  '[&_foreignObject]:overflow-visible',
].join(' ')

const mutedClass = 'text-xs text-(--gray-10)'

type RenderState =
  | { status: 'rendering' }
  | { status: 'rendered'; svg: string }
  | { status: 'failed'; message: string }

export interface MermaidGraphCanvasProps {
  title: string
  ariaLabel: string
  /** Prefix for the document-unique element ids handed to mermaid.render. */
  idPrefix: string
  /** Mermaid source, or null when there is nothing to draw. */
  source: string | null
  /** Shown instead of the legend and canvas while source is null. */
  emptyContent?: ReactNode
  /**
   * Extra utilities appended to the canvas element, e.g. the palette for
   * the `:::class` tags a graph builder emits (mermaid classDef cannot
   * express CSS variables, so node colors live in the host stylesheet).
   */
  canvasClassName?: string
  /** Legend content rendered between the header and the canvas. */
  children?: ReactNode
  /** Rendered below the canvas: truncation notes and similar footnotes. */
  footer?: ReactNode
}

export function MermaidGraphCanvas({
  title,
  ariaLabel,
  idPrefix,
  source,
  emptyContent,
  canvasClassName,
  children,
  footer,
}: MermaidGraphCanvasProps) {
  const { resolvedTheme } = useTheme()
  const isDark = resolvedTheme === 'dark'
  const [renderState, setRenderState] = useState<RenderState>({
    status: 'rendering',
  })
  const sectionRef = useRef<HTMLElement>(null)
  // Set from an effect so server and first client render agree on "no button".
  const [fullscreenEnabled, setFullscreenEnabled] = useState(false)
  const [isFullscreen, setIsFullscreen] = useState(false)
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>(
    'idle'
  )
  const copyResetRef = useRef<number | null>(null)

  useEffect(() => {
    setFullscreenEnabled(Boolean(document.fullscreenEnabled))
    const onFullscreenChange = () => {
      setIsFullscreen(document.fullscreenElement === sectionRef.current)
    }
    document.addEventListener('fullscreenchange', onFullscreenChange)
    return () => {
      document.removeEventListener('fullscreenchange', onFullscreenChange)
      if (copyResetRef.current !== null) {
        window.clearTimeout(copyResetRef.current)
      }
    }
  }, [])

  const copySource = async () => {
    if (source === null) {
      return
    }
    try {
      await navigator.clipboard.writeText(source)
      setCopyState('copied')
    } catch {
      setCopyState('failed')
    }
    if (copyResetRef.current !== null) {
      window.clearTimeout(copyResetRef.current)
    }
    copyResetRef.current = window.setTimeout(() => setCopyState('idle'), 2000)
  }

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
          `${idPrefix}-${renderSequence}`,
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
  }, [source, isDark, idPrefix])

  return (
    <section ref={sectionRef} className={graphCardClass} aria-label={ariaLabel}>
      <div className="flex items-center justify-between gap-3">
        <h2 className="m-0 text-base text-(--gray-12)">{title}</h2>
        <span className="flex items-center gap-2">
          {source !== null ? (
            <button
              type="button"
              className={graphButtonClass}
              onClick={copySource}
            >
              {copyState === 'copied'
                ? 'Copied'
                : copyState === 'failed'
                  ? 'Copy failed'
                  : 'Copy mermaid source'}
            </button>
          ) : null}
          {fullscreenEnabled &&
          source !== null &&
          renderState.status === 'rendered' ? (
            <button
              type="button"
              className={graphButtonClass}
              onClick={toggleFullscreen}
            >
              {isFullscreen ? 'Exit full screen' : 'Full screen'}
            </button>
          ) : null}
        </span>
      </div>
      {source === null ? (
        <p className={mutedClass}>{emptyContent}</p>
      ) : (
        <>
          {children}
          {renderState.status === 'rendered' ? (
            <div
              className={
                canvasClassName
                  ? `${baseCanvasClass} ${canvasClassName}`
                  : baseCanvasClass
              }
              // mermaid output is generated from validated data and rendered
              // under securityLevel: 'strict'.
              dangerouslySetInnerHTML={{ __html: renderState.svg }}
            />
          ) : renderState.status === 'failed' ? (
            <p className={mutedClass} role="alert">
              The graph could not be rendered: {renderState.message}
            </p>
          ) : (
            <output className={mutedClass} aria-live="polite">
              Rendering graph…
            </output>
          )}
          {footer}
        </>
      )}
    </section>
  )
}
