// Expression-level inference rules: operators, the equality matrix, indexing
// and the union-elimination machinery they all share
// (docs/type-system-design.md sections 7.4 and 7.5).
//
// These are the parts of inference that do not recurse into the tree. The walk
// itself lives in `check.ts`, which owns the environment and the diagnostics —
// keeping the recursion in one file avoids a circular import between the two.

import {
  ANY,
  BOOL,
  INT,
  STRING,
  joinAll,
  members,
  optional,
  stripNull,
  type Type,
} from './types'

/**
 * The union-elimination rule of section 7.1: strip `null`, apply the operation
 * to every remaining member, and join the results. A single member failing
 * fails the whole operation.
 */
export function overMembers(
  type: Type,
  operate: (member: Type) => Type | null
): Type | null {
  const results: Type[] = []
  for (const member of members(stripNull(type))) {
    const result = operate(member)
    if (result === null) {
      return null
    }
    results.push(result)
  }
  return joinAll(results)
}

/** The same, for the two operands of a binary operator. */
export function overMemberPairs(
  left: Type,
  right: Type,
  operate: (left: Type, right: Type) => Type | null
): Type | null {
  return overMembers(left, (leftMember) =>
    overMembers(right, (rightMember) => operate(leftMember, rightMember))
  )
}

export const ARITHMETIC_OPERATORS = ['-', '*', '/']
export const COMPARISON_OPERATORS = ['<', '>']
export const EQUALITY_OPERATORS = ['==', '!=']

/**
 * `+` on two concrete (non-union) types. The `any` exemption has to name a
 * result type, and `+` is the one overloaded operator: `any + int` is `int`,
 * `any + string` is `string`, and anything else degrades to `any` so the
 * exemption stays total.
 */
function plus(left: Type, right: Type): Type | null {
  if (left.kind === 'any' || right.kind === 'any') {
    const other = left.kind === 'any' ? right : left
    if (other.kind === 'int') {
      return INT
    }
    if (other.kind === 'string') {
      return STRING
    }
    return ANY
  }
  if (left.kind === 'int' && right.kind === 'int') {
    return INT
  }
  if (left.kind === 'string' && right.kind === 'string') {
    return STRING
  }
  return null
}

function intOnly(left: Type, right: Type, result: Type): Type | null {
  if (left.kind === 'any' || right.kind === 'any') {
    return result
  }
  return left.kind === 'int' && right.kind === 'int' ? result : null
}

/** `null` means the operator rejects these operands. */
export function inferBinary(
  operator: string,
  left: Type,
  right: Type
): Type | null {
  if (operator === '+') {
    return overMemberPairs(left, right, plus)
  }
  if (ARITHMETIC_OPERATORS.includes(operator)) {
    return overMemberPairs(left, right, (a, b) => intOnly(a, b, INT))
  }
  if (COMPARISON_OPERATORS.includes(operator)) {
    return overMemberPairs(left, right, (a, b) => intOnly(a, b, BOOL))
  }
  return null
}

export function inferPrefix(operator: string, operand: Type): Type | null {
  if (operator === '!') {
    // Truthiness is defined for every value; only `false` and `null` are falsy.
    return BOOL
  }
  if (operator === '-') {
    return overMembers(operand, (member) =>
      member.kind === 'any' || member.kind === 'int' ? INT : null
    )
  }
  return null
}

export type EqualityVerdict = { ok: true } | { ok: false; reason: 'mixed' }

/**
 * `==` / `!=` do not reuse assignability; they follow the equality matrix of
 * section 7.4. Equality is total in every backend — arrays and hashes compare
 * structurally, closures and instances by identity, and operands of different
 * types are simply unequal — so the only thing left to say about a comparison
 * is whether its answer is already known at check time.
 */
export function inferEquality(left: Type, right: Type): EqualityVerdict {
  const lefts = members(stripNull(left))
  const rights = members(stripNull(right))

  for (const leftMember of lefts) {
    for (const rightMember of rights) {
      if (leftMember.kind === 'any' || rightMember.kind === 'any') {
        continue
      }
      // Identity comparisons never require the same class: `new A() == new B()`
      // is legal in every backend and constantly false.
      if (leftMember.kind !== rightMember.kind) {
        return { ok: false, reason: 'mixed' }
      }
    }
  }
  return { ok: true }
}

export type IndexVerdict =
  | { ok: true; type: Type }
  | { ok: false; reason: 'target' | 'subscript' }

/**
 * Indexing yields `T?` / `V?`: a miss is `null` at runtime in every backend.
 * A hash lookup with a key type that cannot match is rejected here even though
 * it merely misses at runtime — it is virtually always a mistake.
 */
export function inferIndex(
  target: Type,
  subscript: Type,
  assignableTo: (from: Type, to: Type) => boolean,
  isHashable: (type: Type) => boolean
): IndexVerdict {
  let reason: 'target' | 'subscript' = 'target'
  const result = overMembers(target, (member) => {
    if (member.kind === 'any') {
      return ANY
    }
    if (member.kind === 'array') {
      const indexable = members(stripNull(subscript)).every(
        (part) => part.kind === 'any' || part.kind === 'int'
      )
      if (!indexable) {
        reason = 'subscript'
        return null
      }
      return optional(member.element)
    }
    if (member.kind === 'hash') {
      if (!isHashable(subscript) || !assignableTo(subscript, member.key)) {
        reason = 'subscript'
        return null
      }
      return optional(member.value)
    }
    reason = 'target'
    return null
  })

  return result === null ? { ok: false, reason } : { ok: true, type: result }
}
