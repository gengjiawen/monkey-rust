import type { Diagnostic } from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'

import {
  editorSpan,
  runDiagnostics,
  type DiagnosticsProvider,
} from './diagnosticsPanel'

type LintModule = typeof import('../../monkey-linter/src/index')

const lintTool = { name: 'monkey-lint', subject: 'Linter' }

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
    const span = editorSpan(source, diagnostic.span)
    return {
      from: span.start,
      to: span.end,
      severity: diagnostic.severity === 'error' ? 'error' : 'warning',
      source: diagnostic.rule,
      message: diagnostic.message,
    }
  })
}

/** Lint the current document and show the results in the diagnostics panel. */
export async function runMonkeyLint(
  view: EditorView,
  diagnosticsProvider: DiagnosticsProvider = monkeyLintDiagnostics
): Promise<void> {
  return runDiagnostics(view, diagnosticsProvider, lintTool)
}
