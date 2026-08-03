// The AST node definitions ship inside `@gengjiawen/monkey-wasm` itself:
// wasm/src/ast_types.d.ts is appended to the generated declarations, so the
// tree described there is always the one the linked parser build emits.
// Re-exported here so the rest of the minifier keeps importing from one place;
// the accessors below are the only runtime code needed on top.
export type * from '@gengjiawen/monkey-wasm'

import type { LetStatement, Token } from '@gengjiawen/monkey-wasm'

export function identifierName(statement: LetStatement): string {
  return statement.identifier.name
}

export function setIdentifierName(statement: LetStatement, name: string): void {
  statement.identifier.name = name
}

export function tokenType(token: Token): string {
  return token.kind.type
}
