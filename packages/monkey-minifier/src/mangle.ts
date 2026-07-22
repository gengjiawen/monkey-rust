import type { Program } from './types'
import { setIdentifierName } from './types'
import { analyzeScopes, type Binding } from './scope'

const KEYWORDS = new Set([
  'let',
  'return',
  'fn',
  'if',
  'else',
  'true',
  'false',
  'class',
  'this',
  'new',
])

export interface MangleOptions {
  reserved?: string[]
}

export function mangle(program: Program, options: MangleOptions = {}): Program {
  const analysis = analyzeScopes(program)
  if (!analysis.safe) {
    return program
  }

  const reserved = new Set(options.reserved ?? [])
  const forbidden = new Set([
    ...analysis.forbiddenNames,
    ...reserved,
    ...KEYWORDS,
  ])
  for (const binding of analysis.bindings) {
    if (binding.preserve || reserved.has(binding.originalName)) {
      binding.preserve = true
      forbidden.add(binding.originalName)
    }
  }

  const candidates = analysis.bindings
    .filter((binding) => !binding.preserve && isUserBinding(binding))
    .sort(
      (left, right) =>
        right.references.length - left.references.length || left.id - right.id
    )

  let index = 0
  for (const binding of candidates) {
    let name: string
    do {
      name = generatedName(index++)
    } while (forbidden.has(name))
    forbidden.add(name)
    renameBinding(binding, name)
  }
  return program
}

function isUserBinding(binding: Binding): boolean {
  return binding.kind === 'let' || binding.kind === 'parameter'
}

function renameBinding(binding: Binding, name: string): void {
  for (const statement of binding.lets) {
    setIdentifierName(statement, name)
  }
  for (const identifier of binding.identifiers) {
    identifier.name = name
  }
  for (const reference of binding.references) {
    reference.name = name
  }
  for (const declaration of binding.functions) {
    declaration.name = name
  }
}

function generatedName(index: number): string {
  const letter = String.fromCharCode(97 + (index % 26))
  const suffix = Math.floor(index / 26)
  return suffix === 0 ? letter : `${letter}${suffix - 1}`
}
