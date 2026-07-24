import type { Rule } from '../core'
import { BUILTIN_NAMES } from '../scope'

const BUILTINS: ReadonlySet<string> = new Set(BUILTIN_NAMES)

/**
 * A `let`, parameter, or class binding whose name collides with a predefined
 * builtin (`len`, `puts`, `first`, `last`, `rest`, `push`, `print`). Shadowing
 * is legal — the compiler resolves the local binding — but it makes the builtin
 * unreachable for the rest of that scope, which is almost always a mistake.
 */
export const noShadowedBuiltin: Rule = {
  name: 'no-shadowed-builtin',
  severity: 'warn',
  check({ scope, report }) {
    for (const binding of scope.bindings) {
      if (binding.kind === 'builtin' || binding.kind === 'this') {
        continue
      }
      if (BUILTINS.has(binding.name)) {
        report(`'${binding.name}' shadows a builtin`, binding.nameSpan)
      }
    }
  },
}
