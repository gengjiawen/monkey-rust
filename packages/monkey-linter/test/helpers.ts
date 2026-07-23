import { lint } from '../src'
import type { Diagnostic, LintOptions } from '../src/types'

export function diagnose(source: string, options?: LintOptions): Diagnostic[] {
  return lint(source, options).diagnostics
}

/** Rule ids that fired, in the linter's stable (span-sorted) order. */
export function rulesOf(source: string, options?: LintOptions): string[] {
  return diagnose(source, options).map((diagnostic) => diagnostic.rule)
}

/** `rule@start-end: message` per diagnostic — a compact, span-pinning view. */
export function compact(source: string, options?: LintOptions): string[] {
  return diagnose(source, options).map((diagnostic) => {
    const where = diagnostic.span
      ? `${diagnostic.span.start}-${diagnostic.span.end}`
      : '?'
    return `${diagnostic.rule}@${where}: ${diagnostic.message}`
  })
}
