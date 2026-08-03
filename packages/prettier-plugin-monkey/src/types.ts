// AST node definitions live in `@gengjiawen/monkey-ast-types`, shared with the
// linter, the minifier and the type checker. The plugin adds the two things
// only a formatter needs: comments, which the parser recovers separately, and
// the narrower token kinds the printer switches on.
export * from '@gengjiawen/monkey-ast-types'

import type { Span } from '@gengjiawen/monkey-ast-types'

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
