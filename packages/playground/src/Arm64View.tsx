'use client'

import { Button } from '@radix-ui/themes'
import { useCallback, type Ref } from 'react'

import type { Arm64BuildEnvelope } from './arm64'
import { Editor, type EditorHandle } from './Editor'
import type { SourceSpan } from './gcReport'

export type Arm64BuildState =
  | { status: 'idle' }
  | Arm64BuildEnvelope
  | { status: 'invalid'; message: string }

interface Arm64ViewProps {
  build: Arm64BuildState
  /** Handle of the assembly pane, for source → assembly highlighting. */
  editorRef: Ref<EditorHandle>
  onSelectionChange?: (selection: { from: number; to: number }) => void
  onErrorSpanSelect?: (span: SourceSpan) => void
}

/** Links a downloaded `.s` against the runtime library (see asm/README.md). */
const crossAssembleCommand =
  'aarch64-linux-gnu-gcc program.s libmonkey_asm.a -o program -static'

const mutedClass = 'text-xs text-(--gray-10)'

const alertClass =
  'm-0 rounded-md border border-(--red-a6) bg-(--red-a3) px-3 py-2 text-sm text-(--red-11)'

function downloadAssembly(text: string) {
  const blob = new Blob([text + '\n'], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = 'program.s'
  anchor.click()
  URL.revokeObjectURL(url)
}

export function Arm64View({
  build,
  editorRef,
  onSelectionChange,
  onErrorSpanSelect,
}: Arm64ViewProps) {
  const handleDownload = useCallback(() => {
    if (build.status === 'ok') {
      downloadAssembly(build.text)
    }
  }, [build])

  if (build.status !== 'ok') {
    return (
      <div className="min-h-0 flex-1 overflow-auto bg-(--gray-1) p-4.5">
        {build.status === 'idle' ? (
          <output className={`${mutedClass} block`}>Lowering to arm64…</output>
        ) : null}
        {build.status === 'error' ? (
          <div>
            <p role="alert" className={alertClass}>
              {build.stage} error: {build.message}
            </p>
            {build.span !== null ? (
              <Button
                size="1"
                variant="soft"
                className="mt-2"
                onClick={() => {
                  const { span } = build
                  if (span !== null) {
                    onErrorSpanSelect?.(span)
                  }
                }}
              >
                Show in editor ({build.span.start}–{build.span.end})
              </Button>
            ) : null}
          </div>
        ) : null}
        {build.status === 'invalid' ? (
          <p role="alert" className={alertClass}>
            {build.message}
          </p>
        ) : null}
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1.5 border-b border-(--gray-a5) bg-(--color-background) px-3 py-2">
        <Button size="1" variant="soft" onClick={handleDownload}>
          Download .s
        </Button>
        <p className={`${mutedClass} m-0 min-w-0 flex-1 basis-52`}>
          The exact text <code>monkey-asm emit</code> writes — nothing executes
          arm64 in the browser. Select an instruction to highlight its source
          (and vice versa). Cross-assemble the download against the runtime
          library (see <code>asm/README.md</code>):{' '}
          <code className="break-all text-(--gray-12)">
            {crossAssembleCommand}
          </code>
        </p>
      </div>
      <Editor
        ref={editorRef}
        code={build.text}
        extra={{ readOnly: true, editable: false }}
        onSelectionChange={onSelectionChange}
        vimMode={false}
        fill
      />
    </div>
  )
}
