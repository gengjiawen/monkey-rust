import { readFileSync } from 'fs'
import { join } from 'path'
import * as vscode from 'vscode'

import {
  checkWithAnalyzer,
  type TypeDiagnostic,
} from '@gengjiawen/monkey-typechecker'

import { utf8ByteOffsetToUtf16 } from './spans'

// Only the exports the extension actually calls, so adding a new wasm export
// never breaks the handwritten binding object below.
type MonkeyWasm = Pick<
  typeof import('@gengjiawen/monkey-wasm'),
  'parse' | 'compile' | 'analyze_lossless'
>
type MonkeyWasmBindings = MonkeyWasm & {
  __wbg_set_wasm: (wasm: Record<string, unknown>) => void
}
type WasmInstance = {
  exports: Record<string, unknown>
}
type WebAssemblyRuntime = {
  instantiate: (
    bytes: Uint8Array,
    imports: Record<string, Record<string, unknown>>
  ) => Promise<{ instance: WasmInstance }>
}

const bindings =
  require('@gengjiawen/monkey-wasm/monkey_wasm_bg.js') as MonkeyWasmBindings

let wasmPromise: Promise<MonkeyWasm> | null = null

async function createWasmBindings(): Promise<MonkeyWasm> {
  // The bundle and the .wasm asset are emitted side by side into dist/.
  const wasmPath = join(__dirname, 'monkey_wasm_bg.wasm')
  const wasmRuntime = (
    globalThis as unknown as {
      WebAssembly: WebAssemblyRuntime
    }
  ).WebAssembly
  const imports = {
    './monkey_wasm_bg.js': bindings as unknown as Record<string, unknown>,
  }
  const { instance } = await wasmRuntime.instantiate(
    new Uint8Array(readFileSync(wasmPath)),
    imports
  )

  bindings.__wbg_set_wasm(instance.exports)
  const start = instance.exports.__wbindgen_start
  if (typeof start === 'function') {
    start()
  }

  return {
    parse: bindings.parse,
    compile: bindings.compile,
    analyze_lossless: bindings.analyze_lossless,
  }
}

function loadWasm(): Promise<MonkeyWasm> {
  if (!wasmPromise) {
    wasmPromise = createWasmBindings()
  }
  return wasmPromise
}

let diagnosticsCollection: vscode.DiagnosticCollection

function toRange(
  doc: vscode.TextDocument,
  text: string,
  span: TypeDiagnostic['span']
): vscode.Range {
  if (!span) {
    // A parser error the Rust side could not attribute to a token; mark the
    // first character so the squiggle is still visible.
    return new vscode.Range(
      new vscode.Position(0, 0),
      new vscode.Position(0, 1)
    )
  }
  return new vscode.Range(
    doc.positionAt(utf8ByteOffsetToUtf16(text, span.start)),
    doc.positionAt(utf8ByteOffsetToUtf16(text, span.end))
  )
}

function toDiagnostic(
  doc: vscode.TextDocument,
  text: string,
  diagnostic: TypeDiagnostic
): vscode.Diagnostic {
  const converted = new vscode.Diagnostic(
    toRange(doc, text, diagnostic.span),
    diagnostic.message,
    diagnostic.severity === 'warning'
      ? vscode.DiagnosticSeverity.Warning
      : vscode.DiagnosticSeverity.Error
  )
  converted.source = 'monkey'
  converted.code = diagnostic.code
  return converted
}

export function activate(context: vscode.ExtensionContext) {
  diagnosticsCollection = vscode.languages.createDiagnosticCollection('monkey')
  context.subscriptions.push(diagnosticsCollection)

  const cfg = vscode.workspace.getConfiguration('monkey')
  const diagnosticsEnabled = cfg.get<boolean>('enableWasmDiagnostics', true)

  if (diagnosticsEnabled) {
    const validate = async (doc: vscode.TextDocument) => {
      if (doc.languageId !== 'monkey') return
      const text = doc.getText()
      try {
        const mod = await loadWasm()
        // The checker reports parse and validation failures as data, with the
        // span the Rust side recorded, so both land in the same list.
        const { diagnostics } = checkWithAnalyzer(mod.analyze_lossless, text)
        diagnosticsCollection.set(
          doc.uri,
          diagnostics.map((diagnostic) => toDiagnostic(doc, text, diagnostic))
        )
      } catch (e: any) {
        // Only reachable when the wasm module itself fails to load.
        const message = typeof e?.message === 'string' ? e.message : String(e)
        const diag = new vscode.Diagnostic(
          new vscode.Range(
            new vscode.Position(0, 0),
            new vscode.Position(0, 1)
          ),
          message,
          vscode.DiagnosticSeverity.Error
        )
        diag.source = 'monkey'
        diagnosticsCollection.set(doc.uri, [diag])
      }
    }

    context.subscriptions.push(
      vscode.workspace.onDidOpenTextDocument(validate),
      vscode.workspace.onDidChangeTextDocument(
        (e) => void validate(e.document)
      ),
      vscode.workspace.onDidSaveTextDocument(validate)
    )

    // validate already-open documents
    vscode.workspace.textDocuments.forEach(validate)
  }

  context.subscriptions.push(
    vscode.commands.registerCommand('monkey.compileToBytecode', async () => {
      const editor = vscode.window.activeTextEditor
      if (!editor) return
      const text = editor.document.getText()
      try {
        const mod = await loadWasm()
        const output = mod.compile(text)
        const doc = await vscode.workspace.openTextDocument({
          language: 'text',
          content: output,
        })
        await vscode.window.showTextDocument(doc, { preview: true })
      } catch (e: any) {
        vscode.window.showErrorMessage(
          typeof e?.message === 'string' ? e.message : String(e)
        )
      }
    }),
    vscode.commands.registerCommand('monkey.showAST', async () => {
      const editor = vscode.window.activeTextEditor
      if (!editor) return
      const text = editor.document.getText()
      try {
        const mod = await loadWasm()
        const astJson = mod.parse(text)
        const doc = await vscode.workspace.openTextDocument({
          language: 'json',
          content: astJson,
        })
        await vscode.window.showTextDocument(doc, { preview: true })
      } catch (e: any) {
        vscode.window.showErrorMessage(
          typeof e?.message === 'string' ? e.message : String(e)
        )
      }
    })
  )
}

export function deactivate() {
  diagnosticsCollection?.dispose()
}
