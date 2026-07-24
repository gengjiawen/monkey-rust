import { analyze_lossless } from '@gengjiawen/monkey-wasm'

import { lintWithAnalyzer } from './core'
import type { LintOptions, LintResult } from './types'

/**
 * Lint Monkey source in a bundler/browser environment. The host bundles the
 * wasm `analyze_lossless` export directly (wasm-pack's `bundler` target); Node
 * consumers should import `@gengjiawen/monkey-lint/node` instead.
 */
export function lint(source: string, options: LintOptions = {}): LintResult {
  return lintWithAnalyzer(analyze_lossless, source, options)
}

export { lintWithAnalyzer } from './core'
export type { Rule, RuleContext } from './core'
export { rules } from './rules'
export { BUILTIN_NAMES } from './scope'
export type * from './types'
