import { readFileSync } from 'node:fs'

import { lintWithAnalyzer } from './core'
import type { AnalyzeLossless, LintOptions, LintResult } from './types'

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
 * Lint Monkey source in Node, instantiating the bundled wasm module directly.
 * Instantiation happens on the first call and is cached, so merely importing
 * this module (e.g. for `monkey-lint --help`) never pays the wasm setup cost.
 */
export function lint(source: string, options: LintOptions = {}): LintResult {
  cachedAnalyzer ??= loadNodeAnalyzer()
  return lintWithAnalyzer(cachedAnalyzer, source, options)
}

export { lintWithAnalyzer } from './core'
export type { Rule, RuleContext } from './core'
export { rules } from './rules'
export { BUILTIN_NAMES } from './scope'
export type * from './types'
