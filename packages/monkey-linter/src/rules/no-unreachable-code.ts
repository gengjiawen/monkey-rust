import type { Rule } from '../core'
import type { BlockStatement, Program } from '../types'
import { walk } from '../walk'

/**
 * A statement that follows a `return` in the same statement list. Both the
 * interpreter and the VM stop the enclosing block at `return`, so anything after
 * it in that block never runs. This stays within one statement list — it does
 * not reason across branches (a `return` in only one arm of an `if` leaves the
 * code after the `if` reachable).
 */
export const noUnreachableCode: Rule = {
  name: 'no-unreachable-code',
  severity: 'warn',
  check({ program, report }) {
    walk(program, (node) => {
      if (node.type !== 'Program' && node.type !== 'BlockStatement') {
        return
      }
      const body = (node as Program | BlockStatement).body
      const returnIndex = body.findIndex(
        (statement) => statement.type === 'ReturnStatement'
      )
      if (returnIndex >= 0 && returnIndex < body.length - 1) {
        report('unreachable code after return', body[returnIndex + 1].span)
      }
    })
  },
}
