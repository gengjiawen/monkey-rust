'use client'

import { Editor } from './Editor'

export type MinifyState =
  | { status: 'idle' }
  | {
      status: 'ok'
      code: string
      originalBytes: number
      minifiedBytes: number
    }
  | { status: 'invalid'; message: string }

interface MinifyViewProps {
  state: MinifyState
}

const mutedClass = 'text-xs text-(--gray-10)'
const alertClass =
  'm-0 rounded-md border border-(--red-a6) bg-(--red-a3) px-3 py-2 text-sm text-(--red-11)'

export function utf8Bytes(text: string): number {
  return new TextEncoder().encode(text).byteLength
}

export function MinifyView({ state }: MinifyViewProps) {
  if (state.status === 'idle') {
    return (
      <div className="min-h-0 flex-1 overflow-auto bg-(--gray-1) p-4.5">
        <output className={`${mutedClass} block`}>Minifying…</output>
      </div>
    )
  }

  if (state.status === 'invalid') {
    return (
      <div className="min-h-0 flex-1 overflow-auto bg-(--gray-1) p-4.5">
        <p role="alert" className={alertClass}>
          {state.message}
        </p>
      </div>
    )
  }

  const saved = state.originalBytes - state.minifiedBytes
  const percentage =
    state.originalBytes === 0 ? 0 : (saved / state.originalBytes) * 100

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-(--gray-a5) bg-(--color-background) px-3 py-2">
        <output aria-label="Minified byte statistics" className={mutedClass}>
          {state.originalBytes} → {state.minifiedBytes} UTF-8 bytes ·{' '}
          {percentage >= 0 ? 'saved' : 'grew'} {Math.abs(percentage).toFixed(1)}
          %
        </output>
      </div>
      <Editor
        code={state.code}
        extra={{ readOnly: true, editable: false }}
        vimMode={false}
        fill
      />
    </div>
  )
}
