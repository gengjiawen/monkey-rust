'use client'

import { Button } from '@radix-ui/themes'
import { useCallback, type Ref } from 'react'

import type { Arm64BuildEnvelope } from './arm64'
import { arm64EditorExtensions } from './arm64Language'
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

const designDocUrl =
  'https://github.com/gengjiawen/monkey-rust/blob/main/docs/arm64-asm-backend-design.md'

const mutedClass = 'text-xs text-(--gray-10)'

const alertClass =
  'm-0 rounded-md border border-(--red-a6) bg-(--red-a3) px-3 py-2 text-sm text-(--red-11)'

/**
 * Collapsible primer for readers who have never seen AArch64: the value
 * encoding, the accumulator/stack pattern, fast vs. slow paths, and frames.
 * Token-level help lives in the hover docs (arm64Language.ts).
 */
function ReadingGuide() {
  return (
    <details className="shrink-0 border-b border-(--gray-a5) bg-(--gray-2) px-3 py-1.5 text-xs text-(--gray-11)">
      <summary className="cursor-pointer font-medium text-(--gray-12) select-none">
        How to read this assembly
      </summary>
      <ul className="m-0 mt-1.5 flex list-none flex-col gap-1.5 p-0 pb-1">
        <li>
          <strong>Tagged values.</strong> Every Monkey value is one 64-bit
          word. Integers are stored shifted left one bit — <code>3</code>{' '}
          appears as <code>#0x6</code> — while words with bit 0 set are heap
          pointers or the constants <code>#0x3</code> (false),{' '}
          <code>#0x7</code> (true), and <code>#0xb</code> (null). The{' '}
          <code>//</code> comments show the original source value.
        </li>
        <li>
          <strong>x0 is the accumulator.</strong> Every expression leaves its
          result in <code>x0</code>. <code>str x0, [sp, #-16]!</code> pushes
          it onto the stack while the other operand is computed;{' '}
          <code>ldr …, [sp], #16</code> pops it back.
        </li>
        <li>
          <strong>Fast path, slow path.</strong> <code>orr</code> +{' '}
          <code>tbnz</code> checks that both operands are inline integers, and
          inline <code>adds</code>/<code>bvs</code> handles <code>+</code>{' '}
          with overflow detection. Every other case calls the Rust runtime:{' '}
          <code>bl rt_*</code>, arguments in <code>x0</code>/<code>x1</code>,
          result in <code>x0</code>.
        </li>
        <li>
          <strong>Function frames.</strong>{' '}
          <code>stp x29, x30, [sp, #-16]!</code> + <code>mov x29, sp</code>{' '}
          open a frame, with parameters and locals in 16-byte slots below{' '}
          <code>x29</code>; <code>mov sp, x29</code> + <code>ldp</code> +{' '}
          <code>ret</code> close it. Compiled Monkey functions are the{' '}
          <code>.Lfn</code> labels, always entered through <code>rt_call</code>
          .
        </li>
        <li>
          Hover any instruction, register, label, or <code>rt_*</code> symbol
          for a description — full details in the{' '}
          <a
            href={designDocUrl}
            target="_blank"
            rel="noreferrer"
            className="text-(--accent-11) underline"
          >
            backend design doc
          </a>
          .
        </li>
      </ul>
    </details>
  )
}

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
          (and vice versa); hover a token for its meaning. Cross-assemble the
          download against the runtime library (see <code>asm/README.md</code>
          ):{' '}
          <code className="break-all text-(--gray-12)">
            {crossAssembleCommand}
          </code>
        </p>
      </div>
      <ReadingGuide />
      <Editor
        ref={editorRef}
        code={build.text}
        extra={{
          readOnly: true,
          editable: false,
          extensions: arm64EditorExtensions,
        }}
        onSelectionChange={onSelectionChange}
        vimMode={false}
        fill
      />
    </div>
  )
}
