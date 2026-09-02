import { lintWithAnalyzer } from './core'
import { loadMonkeyWasm, type MonkeyWasmGlue } from './wasm-node'
import type { AnalyzeLossless, LintOptions, LintResult } from './types'

interface MonkeyAnalyzerGlue extends MonkeyWasmGlue {
  analyze_lossless: AnalyzeLossless
}

function loadNodeAnalyzer(): AnalyzeLossless {
  return (loadMonkeyWasm() as MonkeyAnalyzerGlue).analyze_lossless
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
