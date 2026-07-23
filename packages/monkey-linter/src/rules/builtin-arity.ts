import type { Rule } from '../core'
import type { FunctionCall, Identifier } from '../types'
import { walk } from '../walk'

/**
 * Builtins whose arity *both* backends reject identically, regardless of the
 * argument types. Only `len` qualifies for v0:
 *
 *   - `len` errors cleanly on any count other than 1 in the interpreter
 *     (`args.len() != 1`) and in the GC VM (`call_builtin_with_output`).
 *   - `first` / `last` / `rest` / `push` diverge: the interpreter indexes
 *     `args[0]` (and `args.last()`) directly, so too *many* arguments are
 *     silently ignored and too *few* panic rather than returning an error,
 *     while the VM returns a clean arity error. Flagging them would be unsound
 *     against the interpreter, so they are intentionally excluded until the
 *     backends converge.
 *   - `puts` / `print` are variadic.
 */
const FIXED_ARITY: Record<string, number> = {
  len: 1,
}

export const builtinArity: Rule = {
  name: 'builtin-arity',
  severity: 'warn',
  check({ program, scope, report }) {
    walk(program, (node) => {
      if (node.type !== 'FunctionCall') {
        return
      }
      const call = node as FunctionCall
      if (call.callee.type !== 'IDENTIFIER') {
        return
      }
      const callee = call.callee as Identifier
      // Resolve by binding identity, not by name: a user `let len = ...` shadows
      // the builtin and must not be flagged.
      const binding = scope.referenceBindings.get(callee)
      if (!binding || binding.kind !== 'builtin') {
        return
      }
      const arity = FIXED_ARITY[callee.name]
      if (arity === undefined) {
        return
      }
      const actual = call.arguments.length
      if (actual !== arity) {
        const plural = arity === 1 ? 'argument' : 'arguments'
        report(
          `builtin '${callee.name}' expects ${arity} ${plural}, got ${actual}`,
          call.span
        )
      }
    })
  },
}
