import type { Rule } from '../core'
import type { Binding } from '../scope'
import type { ClassDeclaration, LetStatement, Span } from '../types'

/**
 * A `let` or `class` binding that is never referenced. Rebinding counts the old
 * binding as used when the new binding's initializer references it (the RHS is
 * analyzed against the previous binding, mirroring the compiler).
 *
 * A reference from inside the binding's own initializer (a recursive `let f =
 * fn() { f(); }` or a class instantiating itself in a method) does not count as
 * a use: if nothing *outside* the declaration ever touches the name, the whole
 * definition is dead, recursion and all.
 */
export const noUnusedLet: Rule = {
  name: 'no-unused-let',
  severity: 'warn',
  check({ scope, report }) {
    for (const binding of scope.bindings) {
      if (binding.kind !== 'let' && binding.kind !== 'class') {
        continue
      }
      if (hasExternalReference(binding)) {
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
      const message =
        binding.references.length > 0
          ? `${label} is only referenced by itself and never used`
          : `${label} is declared but never used`
      report(message, span)
    }
  },
}

function hasExternalReference(binding: Binding): boolean {
  if (binding.references.length === 0) {
    return false
  }
  const self = selfSpan(binding)
  if (!self) {
    // Without a declaration span the reference positions cannot be classified;
    // treat every reference as external rather than risk a false positive.
    return true
  }
  return binding.references.some(
    (reference) =>
      !reference.span ||
      reference.span.start < self.start ||
      reference.span.end > self.end
  )
}

/**
 * The source region whose references are the binding's *own*: a `let`'s
 * initializer expression, or a class's whole declaration (methods included).
 * References elsewhere — including a rebinding's RHS, which reads the previous
 * binding — are external uses.
 */
function selfSpan(binding: Binding): Span | undefined {
  if (binding.kind === 'let') {
    return (binding.declaration as LetStatement).expr.span
  }
  return (binding.declaration as ClassDeclaration).span
}
