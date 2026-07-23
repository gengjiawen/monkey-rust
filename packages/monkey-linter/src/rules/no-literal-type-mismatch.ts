import type { Rule } from '../core'
import type { BinaryExpression, Expression } from '../types'
import { tokenType } from '../types'
import { walk } from '../walk'

type ScalarKind = 'Integer' | 'Boolean' | 'String'

const SCALAR_TYPES = new Set<string>(['Integer', 'Boolean', 'String'])

// Operator token `type` → source symbol, for readable messages.
const OPERATOR_SYMBOL: Record<string, string> = {
  PLUS: '+',
  MINUS: '-',
  ASTERISK: '*',
  SLASH: '/',
  LT: '<',
  GT: '>',
}

const TYPE_LABEL: Record<ScalarKind, string> = {
  Integer: 'integer',
  Boolean: 'boolean',
  String: 'string',
}

/**
 * A binary operator applied to two scalar literals whose types *both* the
 * interpreter and the GC VM reject at runtime. The oracle is derived directly
 * from `eval_infix` (interpreter/lib.rs) and
 * `execute_binary_operation`/`execute_comparison` (gc/vm.rs):
 *
 *   - `==` / `!=` are never flagged. The interpreter compares any two values
 *     structurally and never errors; the VM diverges (it errors on mixed
 *     types), so flagging equality would be unsound against the interpreter.
 *   - `+` is valid only for two integers or two strings.
 *   - `-`, `*`, `/`, `<`, `>` are valid only for two integers.
 *
 * Only literal operands are inspected — a variable could hold any type, so the
 * rule stays quiet unless both sides are known scalar literals.
 */
export const noLiteralTypeMismatch: Rule = {
  name: 'no-literal-type-mismatch',
  severity: 'warn',
  check({ program, report }) {
    walk(program, (node) => {
      if (node.type !== 'BinaryExpression') {
        return
      }
      const binary = node as BinaryExpression
      const left = scalarKind(binary.left)
      const right = scalarKind(binary.right)
      if (!left || !right) {
        return
      }
      const op = tokenType(binary.op)
      if (!isMismatch(op, left, right)) {
        return
      }
      const symbol = OPERATOR_SYMBOL[op] ?? op
      report(
        `operator '${symbol}' cannot be applied to ${TYPE_LABEL[left]} and ${TYPE_LABEL[right]}`,
        binary.span
      )
    })
  },
}

function scalarKind(expression: Expression): ScalarKind | undefined {
  return SCALAR_TYPES.has(expression.type)
    ? (expression.type as ScalarKind)
    : undefined
}

function isMismatch(op: string, left: ScalarKind, right: ScalarKind): boolean {
  const bothInt = left === 'Integer' && right === 'Integer'
  const bothStr = left === 'String' && right === 'String'
  switch (op) {
    case 'EQ':
    case 'NotEq':
      return false
    case 'PLUS':
      return !(bothInt || bothStr)
    case 'MINUS':
    case 'ASTERISK':
    case 'SLASH':
    case 'LT':
    case 'GT':
      return !bothInt
    default:
      return false
  }
}
