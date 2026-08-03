import {
  openLintPanel,
  setDiagnostics,
  type Diagnostic,
} from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'

import { utf8ByteSpanToUtf16, type SourceSpanLike } from './sourceSpan'

export type DiagnosticsProvider = (source: string) => Promise<Diagnostic[]>

export interface DiagnosticsTool {
  /** `source` shown on the diagnostic reporting the tool's own failure. */
  name: string
  /** Subject of that failure message, e.g. `Linter failed: ...`. */
  subject: string
}

/**
 * Map a Rust UTF-8 byte span onto CodeMirror's UTF-16 document positions. A
 * diagnostic without a span — a parser error that could not be located —
 * lands at the document start.
 */
export function editorSpan(
  source: string,
  span: SourceSpanLike | undefined
): SourceSpanLike {
  if (span === undefined) return { start: 0, end: 0 }
  return utf8ByteSpanToUtf16(source, span)
}

function failureDiagnostic(tool: DiagnosticsTool, error: unknown): Diagnostic {
  const message = error instanceof Error ? error.message : String(error)
  return {
    from: 0,
    to: 0,
    severity: 'error',
    source: tool.name,
    message: `${tool.subject} failed: ${message}`,
  }
}

/**
 * Analyse the current document once and surface the results as squiggles plus
 * the diagnostics panel below the editor. Results are discarded when the
 * document changes during the run; accepted ranges follow subsequent edits
 * but are only refreshed by the next run. The panel holds one tool's output at
 * a time, so a later run replaces an earlier one.
 */
export async function runDiagnostics(
  view: EditorView,
  provider: DiagnosticsProvider,
  tool: DiagnosticsTool
): Promise<void> {
  const state = view.state
  let diagnostics: Diagnostic[]
  try {
    diagnostics = await provider(state.doc.toString())
  } catch (error) {
    console.error(`${tool.name} failed:`, error)
    diagnostics = [failureDiagnostic(tool, error)]
  }

  if (view.state.doc !== state.doc) return

  view.dispatch(setDiagnostics(view.state, diagnostics))
  openLintPanel(view)
}
