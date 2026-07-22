import { readFileSync } from 'fs'
import { join } from 'path'
import * as vscode from 'vscode'

type MonkeyWasm = typeof import('@gengjiawen/monkey-wasm')
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
    readFileSync(wasmPath),
    imports
  )

  bindings.__wbg_set_wasm(instance.exports)
  const start = instance.exports.__wbindgen_start
  if (typeof start === 'function') {
    start()
  }

  return {
    parse: bindings.parse,
    parse_lossless: bindings.parse_lossless,
    compile: bindings.compile,
    compile_detail: bindings.compile_detail,
    compile_with_debug: bindings.compile_with_debug,
    run_gc_with_report: bindings.run_gc_with_report,
    compile_to_snapshot: bindings.compile_to_snapshot,
    run_snapshot: bindings.run_snapshot,
    run_snapshot_with_output: bindings.run_snapshot_with_output,
    compile_to_arm64: bindings.compile_to_arm64,
  }
}

function loadWasm(): Promise<MonkeyWasm> {
  if (!wasmPromise) {
    wasmPromise = createWasmBindings()
  }
  return wasmPromise
}

let diagnosticsCollection: vscode.DiagnosticCollection

export function activate(context: vscode.ExtensionContext) {
  diagnosticsCollection = vscode.languages.createDiagnosticCollection('monkey')
  context.subscriptions.push(diagnosticsCollection)

  const cfg = vscode.workspace.getConfiguration('monkey')
  const diagnosticsEnabled = cfg.get<boolean>('enableWasmDiagnostics', true)

  if (diagnosticsEnabled) {
    const validate = async (doc: vscode.TextDocument) => {
      if (doc.languageId !== 'monkey') return
      try {
        const text = doc.getText()
        const mod = await loadWasm()
        // parse returns JSON AST string on success
        mod.parse(text)
        diagnosticsCollection.set(doc.uri, [])
      } catch (e: any) {
        const message = typeof e?.message === 'string' ? e.message : String(e)
        const diag = new vscode.Diagnostic(
          new vscode.Range(
            new vscode.Position(0, 0),
            new vscode.Position(0, 1)
          ),
          message,
          vscode.DiagnosticSeverity.Error
        )
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
