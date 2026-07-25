import type { Rule } from '../core'
import type { FunctionCall, Identifier } from '../types'
import { walk } from '../walk'

/**
 * Builtins whose arity *both* backends reject identically, regardless of the
 * argument types. Every fixed-arity builtin qualifies: each one routes through
 * `check_arity` in `object/builtins.rs` (interpreter and bytecode VM) and
 * through the matching guards in `call_builtin_with_output` (`gc/value.rs`),
 * so a wrong count is an error value on both sides.
 *
 * `first` / `last` / `rest` / `push` used to be excluded because the
 * interpreter indexed `args[0]` directly — extra arguments were silently
 * ignored and a short call panicked instead of erroring. That divergence was
 * fixed, so they are checked here too.
 *
 * `puts` / `print` stay out: they are variadic.
 */
const FIXED_ARITY: Record<string, number> = {
  len: 1,
  first: 1,
  last: 1,
  rest: 1,
  push: 2,
}

export const builtinArity: Rule = {
  name: 'builtin-arity',
  severity: 'error',
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
