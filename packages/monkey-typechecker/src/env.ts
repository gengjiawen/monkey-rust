import type { Type } from './types'

export interface Binding {
  type: Type
  /**
   * True when the binding was introduced by `let x = this` or by a chain of
   * such aliases. Field collection (section 7.8) follows these, and only these
   * — an arbitrary expression that happens to have type `Instance(C)` is not an
   * alias of the receiver.
   */
  thisAlias: boolean
}

/**
 * A lexical scope chain matching `parser/validation.rs`: a `let`'s RHS is
 * checked before the name enters scope, a named function is visible inside its
 * own body, and a class name enters scope before its own methods.
 */
export class Env {
  private readonly scopes: Map<string, Binding>[]

  constructor(scopes?: Map<string, Binding>[]) {
    this.scopes = scopes ?? [new Map()]
  }

  push(): void {
    this.scopes.push(new Map())
  }

  pop(): void {
    this.scopes.pop()
  }

  /**
   * Pops a scope and hands back what it bound. An `if` arm needs this: the
   * interpreter runs one arm against the entering environment, so the arms are
   * checked in isolation and only then merged into the enclosing scope.
   */
  popFrame(): Map<string, Binding> {
    return this.scopes.pop() ?? new Map()
  }

  define(name: string, type: Type, thisAlias = false): void {
    this.scopes[this.scopes.length - 1]!.set(name, { type, thisAlias })
  }

  lookup(name: string): Binding | undefined {
    for (let index = this.scopes.length - 1; index >= 0; index -= 1) {
      const binding = this.scopes[index]!.get(name)
      if (binding) {
        return binding
      }
    }
    return undefined
  }

  /** A snapshot that a closure can keep: the scope list, shared by reference. */
  capture(): Env {
    return new Env([...this.scopes])
  }
}
