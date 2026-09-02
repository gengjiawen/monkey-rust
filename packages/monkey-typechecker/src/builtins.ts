// Builtin signatures, taken from the measured behavior of `object/builtins.rs`
// (docs/type-system-design.md section 7.6):
//
//   len   : (string | [T]) -> int
//   puts  : (...any) -> null        // `print` is an alias of the same builtin
//   first : ([T]) -> T?
//   last  : ([T]) -> T?
//   rest  : ([T]) -> [T]?           // an empty array yields null, not []
//   push  : ([T], T) -> [T]
//
// The generics are deliberately tiny: one type variable per signature,
// instantiated at the call site as the join of every constraint it picks up.

import {
  ANY,
  INT,
  NULL,
  STRING,
  arrayOf,
  joinAll,
  members,
  optional,
  union,
  type Type,
} from './types'

/** A signature fragment that may mention the single type variable `T`. */
export type Template =
  | { shape: 'concrete'; type: Type }
  | { shape: 'variable' }
  | { shape: 'array'; element: Template }
  | { shape: 'optional'; inner: Template }

const T: Template = { shape: 'variable' }
const concrete = (type: Type): Template => ({ shape: 'concrete', type })
const arrayTemplate = (element: Template): Template => ({
  shape: 'array',
  element,
})
const optionalTemplate = (inner: Template): Template => ({
  shape: 'optional',
  inner,
})

export interface BuiltinSignature {
  params: Template[]
  ret: Template
  /** `puts`/`print` take any number of arguments; nothing else does. */
  variadic?: boolean
}

const SIGNATURES: Record<string, BuiltinSignature> = {
  len: {
    params: [concrete(union([STRING, arrayOf(ANY)]))],
    ret: concrete(INT),
  },
  puts: { params: [concrete(ANY)], ret: concrete(NULL), variadic: true },
  print: { params: [concrete(ANY)], ret: concrete(NULL), variadic: true },
  first: { params: [arrayTemplate(T)], ret: optionalTemplate(T) },
  last: { params: [arrayTemplate(T)], ret: optionalTemplate(T) },
  rest: {
    params: [arrayTemplate(T)],
    ret: optionalTemplate(arrayTemplate(T)),
  },
  push: { params: [arrayTemplate(T), T], ret: arrayTemplate(T) },
}

/**
 * Looked up by user-written identifiers, so a `Map` rather than the object:
 * indexing the object with `toString` or `constructor` would hand back an
 * `Object.prototype` method as if it were a builtin signature.
 */
export const BUILTIN_SIGNATURES: ReadonlyMap<string, BuiltinSignature> =
  new Map(Object.entries(SIGNATURES))

export const BUILTIN_NAMES = [...BUILTIN_SIGNATURES.keys()]

/**
 * Gathers every type the argument forces onto `T`. An `any` argument reaching a
 * structural position hides the variable's shape, so it constrains `T` to `any`
 * rather than leaving it free — that is what makes `push(x, 1)` with `x: any`
 * produce `[any]` instead of `[int]`.
 */
function collect(template: Template, argument: Type, into: Type[]): void {
  switch (template.shape) {
    case 'concrete':
      return
    case 'variable':
      into.push(argument)
      return
    case 'optional':
      collect(template.inner, argument, into)
      return
    case 'array': {
      if (argument.kind === 'any') {
        into.push(ANY)
        return
      }
      for (const member of members(argument)) {
        if (member.kind === 'array') {
          collect(template.element, member.element, into)
        }
      }
      return
    }
  }
}

function instantiate(template: Template, variable: Type): Type {
  switch (template.shape) {
    case 'concrete':
      return template.type
    case 'variable':
      return variable
    case 'array':
      return arrayOf(instantiate(template.element, variable))
    case 'optional':
      return optional(instantiate(template.inner, variable))
  }
}

export interface InstantiatedBuiltin {
  params: Type[]
  ret: Type
  variadic: boolean
}

/**
 * Resolves `T` from the call's arguments and substitutes it through the
 * signature. A variable left unconstrained — no arguments, or none that reach
 * its position — instantiates to `any`.
 */
export function instantiateBuiltin(
  signature: BuiltinSignature,
  args: Type[]
): InstantiatedBuiltin {
  const constraints: Type[] = []
  signature.params.forEach((template, index) => {
    const argument = args[index]
    if (argument) {
      collect(template, argument, constraints)
    }
  })
  const variable = constraints.length === 0 ? ANY : joinAll(constraints)

  return {
    params: signature.params.map((template) => instantiate(template, variable)),
    ret: instantiate(signature.ret, variable),
    variadic: signature.variadic ?? false,
  }
}
