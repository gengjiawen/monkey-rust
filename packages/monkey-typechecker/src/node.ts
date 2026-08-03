import { readFileSync } from 'node:fs'

import type { CheckOptions } from './check'
import {
  checkWithAnalyzer,
  type AnalyzeLossless,
  type CheckResult,
} from './core'

interface MonkeyWasmGlue extends WebAssembly.ModuleImports {
  __wbg_set_wasm(exports: WebAssembly.Exports): void
  analyze_lossless: AnalyzeLossless
}

function loadNodeAnalyzer(): AnalyzeLossless {
  // wasm-pack's bundler target statically imports `.wasm`, which Node cannot
  // execute directly. Load the generated glue without its bundler entrypoint
  // and instantiate the same module through Node's WebAssembly API. Node 24 can
  // synchronously require this dependency's ESM glue module.
  const glue =
    require('@gengjiawen/monkey-wasm/monkey_wasm_bg.js') as MonkeyWasmGlue
  const wasmPath = require.resolve(
    '@gengjiawen/monkey-wasm/monkey_wasm_bg.wasm'
  )
  const module = new WebAssembly.Module(readFileSync(wasmPath))
  const instance = new WebAssembly.Instance(module, {
    './monkey_wasm_bg.js': glue,
  })
  glue.__wbg_set_wasm(instance.exports)
  const start = instance.exports.__wbindgen_start
  if (typeof start === 'function') {
    start()
  }
  return glue.analyze_lossless
}

let cachedAnalyzer: AnalyzeLossless | undefined

/**
 * Type check Monkey source in Node, instantiating the bundled wasm module
 * directly. Instantiation happens on the first call and is cached.
 */
export function check(source: string, options: CheckOptions = {}): CheckResult {
  cachedAnalyzer ??= loadNodeAnalyzer()
  return checkWithAnalyzer(cachedAnalyzer, source, options)
}

export { checkProgram } from './check'
export type { CheckOptions } from './check'
export { checkWithAnalyzer } from './core'
export type { AnalyzeLossless, AnalyzeResult, CheckResult } from './core'
export { DIAGNOSTIC_CODES } from './diagnostics'
export type {
  DiagnosticCode,
  TypeDiagnostic,
  TypeSeverity,
} from './diagnostics'
export { BUILTIN_NAMES } from './builtins'
export { assignable, display, type Type } from './types'
