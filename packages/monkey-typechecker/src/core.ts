import type { Program, Span } from '@gengjiawen/monkey-wasm'

import { checkProgram, type CheckOptions } from './check'
import {
  DIAGNOSTIC_CODES,
  type TypeDiagnostic,
  type TypeSeverity,
} from './diagnostics'

export type AnalyzeResult =
  | { status: 'ok'; program: Program }
  | {
      status: 'error'
      stage: 'parse' | 'validation'
      message: string
      span?: Span | null
    }

/** The wasm entry the checker is built on: parse + validation → tagged JSON. */
export type AnalyzeLossless = (source: string) => string

export interface CheckResult {
  diagnostics: TypeDiagnostic[]
}

/**
 * Run the pipeline against an injected `analyze_lossless` binding:
 * analyze (parse + validation) → two-pass check → sort.
 *
 * A parse or validation failure becomes a single `parse-error` /
 * `validation-error` diagnostic and stops the run. Type checking a tree that
 * did not parse would report noise about code the user never wrote.
 */
export function checkWithAnalyzer(
  analyze: AnalyzeLossless,
  source: string,
  options: CheckOptions = {}
): CheckResult {
  const analyzed = runAnalyzer(analyze, source)
  if (analyzed.status === 'error') {
    const code =
      analyzed.stage === 'parse'
        ? DIAGNOSTIC_CODES.parseError
        : DIAGNOSTIC_CODES.validationError
    const severity: TypeSeverity = 'error'
    const diagnostic: TypeDiagnostic = {
      code,
      severity,
      message: analyzed.message,
    }
    if (analyzed.span) {
      diagnostic.span = analyzed.span
    }
    return { diagnostics: [diagnostic] }
  }

  return { diagnostics: checkProgram(analyzed.program, options) }
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
  return JSON.parse(json) as AnalyzeResult
}
