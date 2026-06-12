import { readFileSync } from 'fs'
import { pathToFileURL } from 'url'
import * as vscode from 'vscode'

type MonkeyWasm = {
  parse: (input: string) => string
  compile: (input: string) => string
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

let wasmPromise: Promise<MonkeyWasm> | null = null
type MonkeyWasmBindings = MonkeyWasm & {
  __wbg_set_wasm: (wasm: Record<string, unknown>) => void
}

const dynamicImport = new Function('specifier', 'return import(specifier)') as (
  specifier: string
) => Promise<MonkeyWasmBindings>

function resolveWasmAsset(fileName: string): string {
  return require.resolve(`@gengjiawen/monkey-wasm/${fileName}`)
}

async function createWasmBindings(): Promise<MonkeyWasm> {
  const bindingsPath = resolveWasmAsset('monkey_wasm_bg.js')
  const wasmPath = resolveWasmAsset('monkey_wasm_bg.wasm')
  const bindings = await dynamicImport(pathToFileURL(bindingsPath).href)
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
    compile: bindings.compile,
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
