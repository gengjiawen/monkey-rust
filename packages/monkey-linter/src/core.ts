import { rules } from './rules'
import { analyzeScopes, type ScopeAnalysis } from './scope'
import type {
  AnalyzeLossless,
  AnalyzeResult,
  Diagnostic,
  LintOptions,
  LintResult,
  Program,
  Severity,
  Span,
} from './types'

export interface RuleContext {
  program: Program
  scope: ScopeAnalysis
  /** Emit a diagnostic; the core attaches the rule id and effective severity. */
  report(message: string, span?: Span): void
}

export interface Rule {
  name: string
  severity: Severity
  check(context: RuleContext): void
}

// The analysis-failure diagnostics are not configurable rules, but overriding
// them is a stated no-op rather than a typo, so they count as known names.
const SYNTHETIC_RULES = new Set(['parse-error', 'validation-error'])

/**
 * Run the full pipeline against an injected `analyze_lossless` binding:
 * analyze (parse + validation) → scope analysis → per-rule walk → sort.
 *
 * A parse or validation failure becomes a single non-configurable
 * `parse-error` / `validation-error` diagnostic and stops the run — the linter
 * never lints half of a broken tree.
 *
 * An unknown name or invalid level in `options.rules` throws: malformed
 * overrides must not silently do nothing or leak an invalid diagnostic shape.
 */
export function lintWithAnalyzer(
  analyze: AnalyzeLossless,
  source: string,
  options: LintOptions = {}
): LintResult {
  validateRuleOverrides(options)
  const analyzed = runAnalyzer(analyze, source)
  if (analyzed.status === 'error') {
    const rule = analyzed.stage === 'parse' ? 'parse-error' : 'validation-error'
    const diagnostic: Diagnostic = {
      rule,
      severity: 'error',
      message: analyzed.message,
    }
    if (analyzed.span) {
      diagnostic.span = analyzed.span
    }
    return { diagnostics: [diagnostic] }
  }

  const { program } = analyzed
  const scope = analyzeScopes(program)
  const diagnostics: Diagnostic[] = []
  for (const rule of rules) {
    const level = options.rules?.[rule.name] ?? rule.severity
    if (level === 'off') {
      continue
    }
    const severity: Severity = level
    rule.check({
      program,
      scope,
      report(message, span) {
        const diagnostic: Diagnostic = { rule: rule.name, severity, message }
        if (span) {
          diagnostic.span = span
        }
        diagnostics.push(diagnostic)
      },
    })
  }
  return { diagnostics: sortDiagnostics(diagnostics) }
}

function validateRuleOverrides(options: LintOptions): void {
  if (!options.rules) {
    return
  }
  for (const [name, level] of Object.entries(options.rules)) {
    if (
      !SYNTHETIC_RULES.has(name) &&
      !rules.some((rule) => rule.name === name)
    ) {
      throw new Error(`unknown rule '${name}'`)
    }
    if (level !== 'off' && level !== 'warn' && level !== 'error') {
      throw new Error(
        `invalid level '${String(
          level
        )}' for rule '${name}'; expected 'off', 'warn', or 'error'`
      )
    }
  }
}

function runAnalyzer(analyze: AnalyzeLossless, source: string): AnalyzeResult {
  let json: string
  try {
    json = analyze(source)
  } catch (error) {
    // `analyze_lossless` returns failures as data; a thrown error is an
    // unexpected panic. Surface it as a parse-stage diagnostic rather than
    // crashing the caller.
    const message = error instanceof Error ? error.message : String(error)
    return { status: 'error', stage: 'parse', message }
  }
  const result = JSON.parse(json) as AnalyzeResult
  return result
}

/** Stable order: by span start, then end, then rule id. Span-less last. */
function sortDiagnostics(diagnostics: Diagnostic[]): Diagnostic[] {
  return [...diagnostics].sort((a, b) => {
    if (a.span && b.span) {
      if (a.span.start !== b.span.start) {
        return a.span.start - b.span.start
      }
      if (a.span.end !== b.span.end) {
        return a.span.end - b.span.end
      }
    } else if (a.span || b.span) {
      return a.span ? -1 : 1
    }
    return a.rule < b.rule ? -1 : a.rule > b.rule ? 1 : 0
  })
}
