import type { Span } from '@gengjiawen/monkey-ast-types'

export type TypeSeverity = 'error' | 'warning'

export interface TypeDiagnostic {
  /** kebab-case, from the `DIAGNOSTIC_CODES` set below. */
  code: string
  message: string
  severity: TypeSeverity
  /**
   * UTF-8 byte offsets, the same convention as the AST spans. Every type
   * diagnostic carries one; a parser failure reported through this API may not.
   */
  span?: Span
}

/**
 * The v1 diagnostic codes (docs/type-system-design.md section 10). Treated as a
 * semi-stable API: the playground and the VS Code extension group by code, so
 * entries are appended, never renamed.
 */
export const DIAGNOSTIC_CODES = {
  typeMismatch: 'type-mismatch',
  operatorType: 'operator-type',
  mixedEquality: 'mixed-equality',
  invalidComparison: 'invalid-comparison',
  arityMismatch: 'arity-mismatch',
  notCallable: 'not-callable',
  notConstructable: 'not-constructable',
  unknownProperty: 'unknown-property',
  assignToMethod: 'assign-to-method',
  unknownTypeName: 'unknown-type-name',
  reservedTypeName: 'reserved-type-name',
  invalidHashKey: 'invalid-hash-key',
  invalidIndex: 'invalid-index',
  // Not type rules: the checker cannot run on a tree that failed to parse, so
  // the failure is surfaced through the same channel instead of thrown.
  parseError: 'parse-error',
  validationError: 'validation-error',
} as const

export type DiagnosticCode =
  (typeof DIAGNOSTIC_CODES)[keyof typeof DIAGNOSTIC_CODES]

/** Stable order: by span start, then end, then code. Span-less last. */
export function sortDiagnostics(
  diagnostics: TypeDiagnostic[]
): TypeDiagnostic[] {
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
    return a.code < b.code ? -1 : a.code > b.code ? 1 : 0
  })
}
