import type {
  BooleanLiteral,
  Expression,
  HashLiteral,
  IntegerLiteral,
  Span,
  StringLiteral,
} from '../types'
import type { Rule } from '../core'
import { walk } from '../walk'

/**
 * A hash literal with the same scalar-literal key written more than once. Both
 * backends keep the last write and silently drop the earlier one, so the first
 * value is dead. Only literal keys (`Integer`, `Boolean`, `String`) are
 * compared — an expression key could evaluate to anything, and the two literal
 * types that Monkey can actually hash are integers, booleans, and strings.
 */
export const noDuplicateHashKey: Rule = {
  name: 'no-duplicate-hash-key',
  severity: 'warn',
  check({ program, report }) {
    walk(program, (node) => {
      if (node.type !== 'Hash') {
        return
      }
      const seen = new Set<string>()
      for (const [key] of (node as HashLiteral).elements) {
        const identity = literalKeyIdentity(key)
        if (identity === undefined) {
          continue
        }
        if (seen.has(identity)) {
          report(`duplicate hash key ${describeKey(key)}`, key.span as Span)
        } else {
          seen.add(identity)
        }
      }
    })
  },
}

// A type tag keeps `1` (integer) distinct from `"1"` (string).
function literalKeyIdentity(key: Expression): string | undefined {
  switch (key.type) {
    case 'Integer':
      return `int:${(key as IntegerLiteral).raw}`
    case 'Boolean':
      return `bool:${(key as BooleanLiteral).raw}`
    case 'String':
      return `str:${(key as StringLiteral).raw}`
    default:
      return undefined
  }
}

function describeKey(key: Expression): string {
  switch (key.type) {
    case 'Integer':
      return (key as IntegerLiteral).raw
    case 'Boolean':
      return String((key as BooleanLiteral).raw)
    case 'String':
      return JSON.stringify((key as StringLiteral).raw)
    default:
      return ''
  }
}
