'use client'

import { vim } from '@replit/codemirror-vim'
import { StateEffect, StateField } from '@codemirror/state'
import {
  Decoration,
  EditorView,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view'
import CodeMirror, { type ReactCodeMirrorProps } from '@uiw/react-codemirror'
import {
  forwardRef,
  useCallback,
  useImperativeHandle,
  useMemo,
  useRef,
} from 'react'

import { monkeyLintExtension } from './lint'

interface HighlightRange {
  from: number
  to: number
}

const setHighlight = StateEffect.define<HighlightRange[] | null>()

// CodeMirror injects its own unlayered styles at runtime, so the contested
// declarations (height, scroller font) carry `!` to keep winning over them.
const fillClass =
  'flex h-full min-h-0 flex-1 overflow-hidden [&_.cm-editor]:h-full! [&_.cm-editor]:max-h-full [&_.cm-editor]:min-h-0 [&_.cm-editor]:flex-1 [&_.cm-editor]:bg-(--color-background) [&_.cm-editor]:text-[14px] [&_.cm-scroller]:overflow-auto'

const scrollerFontClass = '[&_.cm-scroller]:font-mono!'

const highlightMark = Decoration.mark({ class: 'cm-ast-highlight' })

const playgroundEditorTheme = EditorView.theme({
  '&': {
    backgroundColor: 'var(--color-background)',
    color: 'var(--gray-12)',
  },
  '.cm-content': {
    caretColor: 'var(--accent-9)',
  },
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: 'var(--accent-9)',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection':
    {
      backgroundColor: 'var(--accent-a5) !important',
    },
  '.cm-gutters': {
    backgroundColor: 'var(--gray-2)',
    color: 'var(--gray-9)',
    borderRight: '1px solid var(--gray-a5)',
  },
  '.cm-activeLineGutter': {
    backgroundColor: 'var(--gray-a3)',
  },
  '.cm-activeLine': {
    backgroundColor: 'var(--gray-a2)',
  },
})

const highlightTheme = EditorView.baseTheme({
  '.cm-ast-highlight': {
    backgroundColor: 'var(--accent-a4)',
    borderBottom: '1.5px solid var(--accent-a8)',
  },
})

const highlightField = StateField.define<DecorationSet>({
  create() {
    return Decoration.none
  },
  update(decorations, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setHighlight)) {
        if (effect.value !== null) {
          return Decoration.set(
            effect.value.map(({ from, to }) => highlightMark.range(from, to))
          )
        }
        return Decoration.none
      }
    }

    if (transaction.docChanged) {
      return Decoration.none
    }

    return decorations
  },
  provide: (field) => EditorView.decorations.from(field),
})

export interface EditorHandle {
  highlightRange: (from: number, to: number) => void
  highlightRanges: (ranges: HighlightRange[]) => void
  clearHighlight: () => void
}

function showHighlightRanges(view: EditorView, ranges: HighlightRange[]) {
  const docLength = view.state.doc.length
  const normalized = ranges
    .map(({ from, to }) => {
      const start = Math.max(0, Math.min(Math.min(from, to), docLength))
      const end = Math.max(start, Math.min(Math.max(from, to), docLength))
      return { from: start, to: end }
    })
    .filter(({ from, to }) => from < to)
    .sort((left, right) => left.from - right.from || left.to - right.to)

  const merged: HighlightRange[] = []
  for (const range of normalized) {
    const previous = merged[merged.length - 1]
    if (previous !== undefined && range.from <= previous.to) {
      previous.to = Math.max(previous.to, range.to)
    } else {
      merged.push({ ...range })
    }
  }

  if (merged.length === 0) {
    view.dispatch({ effects: setHighlight.of(null) })
    return
  }
  view.dispatch({
    effects: [
      setHighlight.of(merged),
      EditorView.scrollIntoView(merged[0].from, { y: 'nearest' }),
    ],
  })
}

interface EditorProps {
  extra?: ReactCodeMirrorProps
  code?: string
  onChange?: (code: string) => void
  onSelectionChange?: (selection: { from: number; to: number }) => void
  vimMode?: boolean
  fill?: boolean
  lineWrapping?: boolean
  /** Run the Monkey linter on the document (squiggles + gutter markers). */
  lint?: boolean
}

export const Editor = forwardRef<EditorHandle, EditorProps>(function Editor(
  {
    extra,
    code = '',
    onChange,
    onSelectionChange,
    vimMode = true,
    fill = false,
    lineWrapping = false,
    lint = false,
  },
  ref
) {
  const viewRef = useRef<EditorView | null>(null)
  const {
    extensions: extraExtensions,
    onCreateEditor: extraOnCreateEditor,
    onUpdate: extraOnUpdate,
    ...extraProps
  } = extra ?? {}

  useImperativeHandle(
    ref,
    () => ({
      highlightRange(from: number, to: number) {
        const view = viewRef.current
        if (!view) return
        showHighlightRanges(view, [{ from, to }])
      },
      highlightRanges(ranges: HighlightRange[]) {
        const view = viewRef.current
        if (!view) return
        showHighlightRanges(view, ranges)
      },
      clearHighlight() {
        viewRef.current?.dispatch({ effects: setHighlight.of(null) })
      },
    }),
    []
  )

  const extensions = useMemo(() => {
    const next = [playgroundEditorTheme, highlightField, highlightTheme]
    if (vimMode) {
      next.push(vim())
    }
    if (lineWrapping) {
      next.push(EditorView.lineWrapping)
    }
    if (lint) {
      next.push(monkeyLintExtension)
    }
    if (extraExtensions) {
      next.push(...extraExtensions)
    }
    return next
  }, [extraExtensions, lineWrapping, lint, vimMode])

  const handleCreateEditor = useCallback(
    (
      view: EditorView,
      state: Parameters<NonNullable<ReactCodeMirrorProps['onCreateEditor']>>[1]
    ) => {
      viewRef.current = view
      extraOnCreateEditor?.(view, state)
    },
    [extraOnCreateEditor]
  )

  const handleUpdate = useCallback(
    (update: ViewUpdate) => {
      extraOnUpdate?.(update)
      if (update.selectionSet || update.docChanged) {
        const { from, to } = update.state.selection.main
        onSelectionChange?.({ from, to })
      }
    },
    [extraOnUpdate, onSelectionChange]
  )

  return (
    <CodeMirror
      {...extraProps}
      className={fill ? `${fillClass} ${scrollerFontClass}` : scrollerFontClass}
      value={code}
      height="100%"
      theme="none"
      extensions={extensions}
      onChange={onChange}
      onCreateEditor={handleCreateEditor}
      onUpdate={handleUpdate}
    />
  )
})
