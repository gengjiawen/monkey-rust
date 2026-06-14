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

const setHighlight = StateEffect.define<{ from: number; to: number } | null>()

const highlightMark = Decoration.mark({ class: 'cm-ast-highlight' })

const highlightField = StateField.define<DecorationSet>({
  create() {
    return Decoration.none
  },
  update(decorations, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setHighlight)) {
        if (effect.value) {
          return Decoration.set([
            highlightMark.range(effect.value.from, effect.value.to),
          ])
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

const highlightTheme = EditorView.baseTheme({
  '.cm-ast-highlight': {
    backgroundColor: 'rgba(47, 111, 237, 0.16)',
    borderBottom: '1.5px solid rgba(47, 111, 237, 0.55)',
  },
})

export interface EditorHandle {
  highlightRange: (from: number, to: number) => void
  clearHighlight: () => void
}

interface EditorProps {
  extra?: ReactCodeMirrorProps
  code?: string
  onChange?: (code: string) => void
  onSelectionChange?: (selection: { from: number; to: number }) => void
  vimMode?: boolean
  fill?: boolean
}

export const Editor = forwardRef<EditorHandle, EditorProps>(function Editor(
  {
    extra,
    code = '',
    onChange,
    onSelectionChange,
    vimMode = true,
    fill = false,
  },
  ref,
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

        const docLength = view.state.doc.length
        const start = Math.max(0, Math.min(Math.min(from, to), docLength))
        const end = Math.max(start, Math.min(Math.max(from, to), docLength))

        view.dispatch({
          effects: [
            setHighlight.of(start === end ? null : { from: start, to: end }),
            EditorView.scrollIntoView(start, { y: 'nearest' }),
          ],
        })
      },
      clearHighlight() {
        viewRef.current?.dispatch({ effects: setHighlight.of(null) })
      },
    }),
    [],
  )

  const extensions = useMemo(() => {
    const next = vimMode ? [vim()] : []
    next.push(highlightField, highlightTheme)
    if (extraExtensions) {
      next.push(...extraExtensions)
    }
    return next
  }, [extraExtensions, vimMode])

  const handleCreateEditor = useCallback(
    (
      view: EditorView,
      state: Parameters<NonNullable<ReactCodeMirrorProps['onCreateEditor']>>[1],
    ) => {
      viewRef.current = view
      extraOnCreateEditor?.(view, state)
    },
    [extraOnCreateEditor],
  )

  const handleUpdate = useCallback(
    (update: ViewUpdate) => {
      extraOnUpdate?.(update)
      if (update.selectionSet || update.docChanged) {
        const { from, to } = update.state.selection.main
        onSelectionChange?.({ from, to })
      }
    },
    [extraOnUpdate, onSelectionChange],
  )

  return (
    <CodeMirror
      {...extraProps}
      className={fill ? 'editor-fill' : undefined}
      value={code}
      height={fill ? undefined : '100%'}
      extensions={extensions}
      onChange={onChange}
      onCreateEditor={handleCreateEditor}
      onUpdate={handleUpdate}
    />
  )
})
