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

type DiagnosticsProvider = (source: string) => Promise<Diagnostic[]>

function lintFailureDiagnostic(error: unknown): Diagnostic {
  const message = error instanceof Error ? error.message : String(error)
  return {
    from: 0,
    to: 0,
    severity: 'error',
    source: 'monkey-lint',
    message: `Linter failed: ${message}`,
  }
}

/**
 * Lint the current document once and surface the results as squiggles plus
 * the diagnostics panel below the editor. Results are discarded when the
 * document changes during the run; accepted ranges follow subsequent edits
 * but are only refreshed by the next run.
 */
export async function runMonkeyLint(
  view: EditorView,
  diagnosticsProvider: DiagnosticsProvider = monkeyLintDiagnostics
): Promise<void> {
  const state = view.state
  let diagnostics: Diagnostic[]
  try {
    diagnostics = await diagnosticsProvider(state.doc.toString())
  } catch (error) {
    console.error('monkey-lint failed:', error)
    diagnostics = [lintFailureDiagnostic(error)]
  }

  if (view.state.doc !== state.doc) return

  view.dispatch(setDiagnostics(view.state, diagnostics))
  openLintPanel(view)
}
