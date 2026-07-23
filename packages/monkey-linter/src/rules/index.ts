import type { Rule } from '../core'
import { builtinArity } from './builtin-arity'
import { noConstantCondition } from './no-constant-condition'
import { noDuplicateHashKey } from './no-duplicate-hash-key'
import { noLiteralTypeMismatch } from './no-literal-type-mismatch'
import { noShadowedBuiltin } from './no-shadowed-builtin'
import { noUnreachableCode } from './no-unreachable-code'
import { noUnusedExpression } from './no-unused-expression'
import { noUnusedLet } from './no-unused-let'
import { noUnusedParam } from './no-unused-param'

/**
 * The v0 rule set, in a fixed order. Diagnostics are sorted by span before they
 * reach the caller, so this order only decides tie-breaks between two rules that
 * fire at the same position.
 */
export const rules: Rule[] = [
  noUnusedLet,
  noUnusedParam,
  noUnusedExpression,
  noUnreachableCode,
  noDuplicateHashKey,
  builtinArity,
  noShadowedBuiltin,
  noConstantCondition,
  noLiteralTypeMismatch,
]

export {
  builtinArity,
  noConstantCondition,
  noDuplicateHashKey,
  noLiteralTypeMismatch,
  noShadowedBuiltin,
  noUnreachableCode,
  noUnusedExpression,
  noUnusedLet,
  noUnusedParam,
}
