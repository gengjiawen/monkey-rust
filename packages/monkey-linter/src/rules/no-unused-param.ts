import type { Rule } from '../core'

/**
 * A function/method parameter that is never referenced. A leading underscore
 * (`_depth`) is an explicit "unused on purpose" opt-out and is not reported.
 */
export const noUnusedParam: Rule = {
  name: 'no-unused-param',
  severity: 'warn',
  check({ scope, report }) {
    for (const binding of scope.bindings) {
      if (binding.kind !== 'parameter') {
        continue
      }
      if (binding.references.length > 0 || binding.name.startsWith('_')) {
        continue
      }
      report(`parameter '${binding.name}' is never used`, binding.nameSpan)
    }
  },
}
