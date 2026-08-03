// The AST node definitions ship inside `@gengjiawen/monkey-wasm` itself:
// wasm/src/ast_types.d.ts is appended to the generated declarations, so the
// tree described there is always the one the linked parser build emits.
// Re-exported here so rules keep importing from one place; the accessors and
// the linter's own data model follow below.
export type * from '@gengjiawen/monkey-wasm'

import type {
  LetStatement,
  Program,
  Span,
  Token,
} from '@gengjiawen/monkey-wasm'

export function identifierName(statement: LetStatement): string {
  return statement.identifier.name
}

export function tokenType(token: Token): string {
  return token.kind.type
}

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
