// AST node definitions live in `@gengjiawen/monkey-ast-types`, shared with the
// minifier, the prettier plugin and the type checker. Re-exported here so rules
// keep importing from one place; the linter's own data model follows below.
export * from '@gengjiawen/monkey-ast-types'

import type { Program, Span } from '@gengjiawen/monkey-ast-types'

// --- Linter data model (docs/linter-plan.md) --------------------------------

export type Severity = 'error' | 'warn'

export interface Diagnostic {
  /** Rule id, e.g. `no-unused-let`. */
  rule: string
  severity: Severity
  /** Human-facing one-liner, including identifier names and other context. */
  message: string
  /** UTF-8 byte offsets, matching the AST span. Optional: parser errors lack one. */
  span?: Span
}

/** Per-rule level override. `off` disables a rule entirely. */
export type RuleLevel = 'off' | 'warn' | 'error'

export interface LintOptions {
  /** Override default rule levels; only affects real lint rules. */
  rules?: Record<string, RuleLevel>
}

export interface LintResult {
  diagnostics: Diagnostic[]
}

export type AnalyzeResult =
  | { status: 'ok'; program: Program }
  | {
      status: 'error'
      stage: 'parse' | 'validation'
      message: string
      span?: Span | null
    }

/** The wasm entry the linter is built on: parse + validation → tagged JSON. */
export type AnalyzeLossless = (source: string) => string
