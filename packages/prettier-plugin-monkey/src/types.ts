// The AST node definitions ship inside `@gengjiawen/monkey-wasm` itself:
// wasm/src/ast_types.d.ts is appended to the generated declarations. The
// plugin adds the two things only a formatter needs: comments, which the
// parser recovers separately, and the narrower token kinds the printer
// switches on.
export type * from '@gengjiawen/monkey-wasm'

import type { Span } from '@gengjiawen/monkey-wasm'

export interface MonkeyComment {
  type: 'CommentLine' | 'CommentBlock'
  value: string
  start: number
  end: number
  span: Span
  leading?: boolean
  trailing?: boolean
  printed?: boolean
}

// The printer switches on these; the trailing `{ type: string }` keeps every
// other token kind assignable.
export type TokenKind =
  | { type: 'PLUS' }
  | { type: 'MINUS' }
  | { type: 'ASTERISK' }
  | { type: 'SLASH' }
  | { type: 'BANG' }
  | { type: 'LT' }
  | { type: 'GT' }
  | { type: 'EQ' }
  | { type: 'NotEq' }
  | { type: 'ASSIGN' }
  | { type: 'IDENTIFIER'; value: { name: string } }
  | { type: string }

export interface Token {
  kind: TokenKind
  span: Span
}

/** Every node the printer visits may carry attached comments. */
export interface ASTNode {
  type: string
  span?: Span
  comments?: MonkeyComment[]
}

export interface Program extends ASTNode {
  type: 'Program'
  body: ASTNode[]
  comments?: MonkeyComment[]
}
