import type { BooleanLiteral, IfExpression } from '../types'
import type { Rule } from '../core'
import { walk } from '../walk'

/**
 * An `if` whose condition is a literal, so the branch is decided at parse time.
 * Both backends treat only `false` and `null` as falsy — every integer (even
 * `0`) and every string (even `""`) is truthy — so the message states the fixed
 * outcome. `null` is not a literal in the grammar, so the only falsy constant is
 * `false`.
 */
export const noConstantCondition: Rule = {
  name: 'no-constant-condition',
  severity: 'warn',
  check({ program, report }) {
    walk(program, (node) => {
      if (node.type !== 'IF') {
        return
      }
      const condition = (node as IfExpression).condition
      if (
        condition.type !== 'Boolean' &&
        condition.type !== 'Integer' &&
        condition.type !== 'String'
      ) {
        return
      }
      const alwaysFalsy =
        condition.type === 'Boolean' && (condition as BooleanLiteral).raw === false
      const outcome = alwaysFalsy ? 'falsy' : 'truthy'
      report(`condition is constant (always ${outcome})`, condition.span)
    })
  },
}
