import {
  openLintPanel,
  setDiagnostics,
  type Diagnostic,
} from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'

import { utf8ByteSpanToUtf16 } from './sourceSpan'

type LintModule = typeof import('../../monkey-linter/src/index')

let lintModulePromise: Promise<LintModule> | null = null

function loadLintModule(): Promise<LintModule> {
  lintModulePromise ??= import('../../monkey-linter/src/index')
  return lintModulePromise
}

/**
 * Run the Monkey linter and map its UTF-8 byte spans onto CodeMirror's UTF-16
 * document positions. A parser diagnostic without a span lands at the document
 * start.
 */
export async function monkeyLintDiagnostics(
  source: string
): Promise<Diagnostic[]> {
  const { lint } = await loadLintModule()
  return lint(source).diagnostics.map((diagnostic) => {
    const span =
      diagnostic.span === undefined
        ? { start: 0, end: 0 }
        : utf8ByteSpanToUtf16(source, diagnostic.span)
    return {
      from: span.start,
      to: span.end,
      severity: diagnostic.severity === 'error' ? 'error' : 'warning',
      source: diagnostic.rule,
      message: diagnostic.message,
    }
  })
}

/**
 * Lint the current document once and surface the results as squiggles plus
 * the diagnostics panel below the editor. Ranges follow subsequent edits but
 * are only refreshed by the next run.
 */
export async function runMonkeyLint(view: EditorView): Promise<void> {
  const diagnostics = await monkeyLintDiagnostics(view.state.doc.toString())
  view.dispatch(setDiagnostics(view.state, diagnostics))
  openLintPanel(view)
}
