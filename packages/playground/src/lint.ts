import { linter, lintGutter, type Diagnostic } from '@codemirror/lint'
import type { Extension } from '@codemirror/state'

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

export const monkeyLintExtension: Extension = [
  lintGutter(),
  linter(
    async (view) => {
      try {
        return await monkeyLintDiagnostics(view.state.doc.toString())
      } catch (error) {
        console.error('monkey-lint failed:', error)
        return []
      }
    },
    { delay: 300 }
  ),
]
