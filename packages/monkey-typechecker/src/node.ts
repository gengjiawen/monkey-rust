import type { CheckOptions } from './check'
import {
  checkWithAnalyzer,
  type AnalyzeLossless,
  type CheckResult,
} from './core'
import { loadMonkeyWasm, type MonkeyWasmGlue } from './wasm-node'

interface MonkeyAnalyzerGlue extends MonkeyWasmGlue {
  analyze_lossless: AnalyzeLossless
}

function loadNodeAnalyzer(): AnalyzeLossless {
  return (loadMonkeyWasm() as MonkeyAnalyzerGlue).analyze_lossless
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
