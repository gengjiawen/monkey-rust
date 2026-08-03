import { analyze_lossless } from '@gengjiawen/monkey-wasm'

import type { CheckOptions } from './check'
import { checkWithAnalyzer, type CheckResult } from './core'

/**
 * Type check Monkey source in a bundler/browser environment. The host bundles
 * the wasm `analyze_lossless` export directly (wasm-pack's `bundler` target);
 * Node consumers should import `@gengjiawen/monkey-typechecker/node` instead.
 */
export function check(source: string, options: CheckOptions = {}): CheckResult {
  return checkWithAnalyzer(analyze_lossless, source, options)
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
