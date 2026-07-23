import type { Rule } from '../core'
import type { ClassDeclaration, LetStatement } from '../types'

/**
 * A `let` or `class` binding that is never referenced. Rebinding counts the old
 * binding as used when the new binding's initializer references it (the RHS is
 * analyzed against the previous binding, mirroring the compiler).
 */
export const noUnusedLet: Rule = {
  name: 'no-unused-let',
  severity: 'warn',
  check({ scope, report }) {
    for (const binding of scope.bindings) {
      if (binding.kind !== 'let' && binding.kind !== 'class') {
        continue
      }
      if (binding.references.length > 0) {
        continue
      }
      const declaration = binding.declaration as
        | LetStatement
        | ClassDeclaration
      const span = binding.nameSpan ?? declaration.span
      const label =
        binding.kind === 'class'
          ? `class '${binding.name}'`
          : `'${binding.name}'`
      report(`${label} is declared but never used`, span)
    }
  },
}
