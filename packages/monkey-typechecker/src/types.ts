// The type universe and its algebra: docs/type-system-design.md section 7.1-7.3.
//
// `Union` has no user syntax — it only appears as an internal representation,
// produced by `join` and by the `T?` annotation, which is exactly
// `Union(T, Null)`.

/**
 * Identity of a `class` declaration. Two declarations with the same name get
 * different ids, so `class A {} let Old = A; class A {}` keeps the alias
 * pointing at the first declaration's type.
 */
export type ClassId = number

export interface AnyType {
  kind: 'any'
}
export interface IntType {
  kind: 'int'
}
export interface BoolType {
  kind: 'bool'
}
export interface StringType {
  kind: 'string'
}
export interface NullType {
  kind: 'null'
}
export interface ArrayType {
  kind: 'array'
  element: Type
}
export interface HashType {
  kind: 'hash'
  key: Type
  value: Type
}
export interface FnType {
  kind: 'fn'
  params: Type[]
  ret: Type
}
/** The class value itself — the operand of `new`. */
export interface ClassType {
  kind: 'class'
  id: ClassId
  name: string
}
/** An instance produced by `new`, nominal by declaration identity. */
export interface InstanceType {
  kind: 'instance'
  id: ClassId
  name: string
}
export interface UnionType {
  kind: 'union'
  members: Type[]
}

export type Type =
  | AnyType
  | IntType
  | BoolType
  | StringType
  | NullType
  | ArrayType
  | HashType
  | FnType
  | ClassType
  | InstanceType
  | UnionType

export const ANY: AnyType = { kind: 'any' }
export const INT: IntType = { kind: 'int' }
export const BOOL: BoolType = { kind: 'bool' }
export const STRING: StringType = { kind: 'string' }
export const NULL: NullType = { kind: 'null' }

export function arrayOf(element: Type): ArrayType {
  return { kind: 'array', element }
}

export function hashOf(key: Type, value: Type): HashType {
  return { kind: 'hash', key, value }
}

export function fnOf(params: Type[], ret: Type): FnType {
  return { kind: 'fn', params, ret }
}

export function classOf(id: ClassId, name: string): ClassType {
  return { kind: 'class', id, name }
}

export function instanceOf(id: ClassId, name: string): InstanceType {
  return { kind: 'instance', id, name }
}

/**
 * A structural key used for de-duplication and for the "exactly the same type"
 * rule. Classes and instances key on their id, never on their display name.
 */
export function typeKey(type: Type): string {
  switch (type.kind) {
    case 'any':
    case 'int':
    case 'bool':
    case 'string':
    case 'null':
      return type.kind
    case 'array':
      return `[${typeKey(type.element)}]`
    case 'hash':
      return `{${typeKey(type.key)}:${typeKey(type.value)}}`
    case 'fn':
      return `fn(${type.params.map(typeKey).join(',')}):${typeKey(type.ret)}`
    case 'class':
      return `class#${type.id}`
    case 'instance':
      return `instance#${type.id}`
    case 'union':
      return `(${type.members.map(typeKey).join('|')})`
  }
}

/**
 * Builds a union, normalized: flattened, de-duplicated, collapsing to `any` if
 * any member is `any`, and unwrapping a one-member union. Member order is the
 * order they were first seen, which keeps diagnostics stable.
 */
export function union(members: Type[]): Type {
  const flat: Type[] = []
  const seen = new Set<string>()
  const push = (type: Type): void => {
    if (type.kind === 'union') {
      type.members.forEach(push)
      return
    }
    const key = typeKey(type)
    if (!seen.has(key)) {
      seen.add(key)
      flat.push(type)
    }
  }
  members.forEach(push)

  if (flat.some((member) => member.kind === 'any')) {
    return ANY
  }
  if (flat.length === 0) {
    return ANY
  }
  if (flat.length === 1) {
    return flat[0]!
  }
  return { kind: 'union', members: flat }
}

/** `T?` — the only union with user syntax. */
export function optional(type: Type): Type {
  return union([type, NULL])
}

export function join(left: Type, right: Type): Type {
  return union([left, right])
}

export function joinAll(types: Type[]): Type {
  return types.length === 0 ? NULL : union(types)
}

export function isNullable(type: Type): boolean {
  return type.kind === 'null' || members(type).some((m) => m.kind === 'null')
}

/** A union's members, or the type itself as a one-element list. */
export function members(type: Type): Type[] {
  return type.kind === 'union' ? type.members : [type]
}

/**
 * Removes `null` from a union before an operation is checked — the deliberately
 * unsound v1 null policy of section 7.5, which lets `first(xs) + 1` through.
 * A bare `null` is left alone: it carries no non-null member to fall back on.
 */
export function stripNull(type: Type): Type {
  if (type.kind !== 'union') {
    return type
  }
  const rest = type.members.filter((member) => member.kind !== 'null')
  return rest.length === 0 ? NULL : union(rest)
}

const HASHABLE_KINDS = ['int', 'bool', 'string', 'any']

/** Only these can be hash keys; everything else is an `invalid-hash-key`. */
export function isHashable(type: Type): boolean {
  return members(stripNull(type)).every((member) =>
    HASHABLE_KINDS.includes(member.kind)
  )
}

/** docs/type-system-design.md section 7.2. */
export function assignable(from: Type, to: Type): boolean {
  if (from.kind === 'any' || to.kind === 'any') {
    return true
  }
  if (typeKey(from) === typeKey(to)) {
    return true
  }
  if (to.kind === 'union') {
    if (to.members.some((member) => assignable(from, member))) {
      return true
    }
  }
  if (from.kind === 'union') {
    const stripped = stripNull(from)
    return members(stripped).every((member) => assignable(member, to))
  }
  if (from.kind === 'array' && to.kind === 'array') {
    // Monkey arrays are immutable — there is no index assignment and `push`
    // returns a new array — so covariance is sound here.
    return assignable(from.element, to.element)
  }
  if (from.kind === 'hash' && to.kind === 'hash') {
    return assignable(from.key, to.key) && assignable(from.value, to.value)
  }
  if (from.kind === 'fn' && to.kind === 'fn') {
    if (from.params.length !== to.params.length) {
      return false
    }
    const parametersFit = to.params.every((param, index) =>
      assignable(param, from.params[index]!)
    )
    return parametersFit && assignable(from.ret, to.ret)
  }
  // Instances and classes are nominal; `typeKey` above already covered the
  // same-id case, so reaching here means different declarations.
  return false
}

function needsParenthesesBeforeQuestionMark(type: Type): boolean {
  return type.kind === 'fn' || type.kind === 'union'
}

/** Renders a type the way an annotation would spell it. */
export function display(type: Type): string {
  switch (type.kind) {
    case 'any':
    case 'int':
    case 'bool':
    case 'string':
    case 'null':
      return type.kind
    case 'array':
      return `[${display(type.element)}]`
    case 'hash':
      return `{${display(type.key)}: ${display(type.value)}}`
    case 'fn':
      return `fn(${type.params.map(display).join(', ')}): ${display(type.ret)}`
    case 'class':
      return type.name
    case 'instance':
      return type.name
    case 'union': {
      const rest = type.members.filter((member) => member.kind !== 'null')
      if (rest.length === 0) {
        return 'null'
      }
      const text = rest.map(display).join(' | ')
      if (rest.length === type.members.length) {
        return text
      }
      const parenthesize =
        rest.length > 1 || needsParenthesesBeforeQuestionMark(rest[0]!)
      return parenthesize ? `(${text})?` : `${text}?`
    }
  }
}
