import type { Diagnostic } from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'

import {
  editorSpan,
  runDiagnostics,
  type DiagnosticsProvider,
} from './diagnosticsPanel'

type TypeCheckModule = typeof import('../../monkey-typechecker/src/index')

const typeCheckTool = { name: 'monkey-typecheck', subject: 'Type checker' }

let typeCheckModulePromise: Promise<TypeCheckModule> | null = null

function loadTypeCheckModule(): Promise<TypeCheckModule> {
  typeCheckModulePromise ??= import('../../monkey-typechecker/src/index')
  return typeCheckModulePromise
}

/**
 * Type check the document and map the checker's UTF-8 byte spans onto
 * CodeMirror's UTF-16 document positions. Annotations are optional, so an
 * unannotated program simply produces fewer diagnostics.
 */
export async function monkeyTypeDiagnostics(
  source: string
): Promise<Diagnostic[]> {
  const { check } = await loadTypeCheckModule()
  return check(source).diagnostics.map((diagnostic) => {
    const span = editorSpan(source, diagnostic.span)
    return {
      from: span.start,
      to: span.end,
      severity: diagnostic.severity === 'error' ? 'error' : 'warning',
      source: diagnostic.code,
      message: diagnostic.message,
    }
  })
}

/**
 * Type check the current document and show the results in the diagnostics
 * panel.
 */
export async function runMonkeyTypeCheck(
  view: EditorView,
  diagnosticsProvider: DiagnosticsProvider = monkeyTypeDiagnostics
): Promise<void> {
  return runDiagnostics(view, diagnosticsProvider, typeCheckTool)
}
