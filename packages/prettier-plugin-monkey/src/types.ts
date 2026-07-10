// Type definitions for Monkey AST nodes

export interface Span {
  start: number
  end: number
}

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

export interface Token {
  kind: TokenKind
  span: Span
}

// Only define token kinds that are actually used in printer
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
  | { type: string } // Allow other token types

// Base interface for all AST nodes
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

export interface LetStatement extends ASTNode {
  type: 'Let'
  identifier: Token
  expr: ASTNode
}

export interface ReturnStatement extends ASTNode {
  type: 'ReturnStatement'
  argument: ASTNode
}

export interface BlockStatement extends ASTNode {
  type: 'BlockStatement'
  body: ASTNode[]
}

export type MethodKind = 'Constructor' | 'Method'

export interface ClassDeclaration extends ASTNode {
  type: 'ClassDeclaration'
  name: Identifier
  methods: MethodDefinition[]
}

export interface MethodDefinition extends ASTNode {
  type: 'MethodDefinition'
  kind: MethodKind
  name: Identifier
  params: Identifier[]
  body: BlockStatement
}

export interface SetPropertyStatement extends ASTNode {
  type: 'SetPropertyStatement'
  object: ASTNode
  property: Identifier
  value: ASTNode
}

export interface Identifier extends ASTNode {
  type: 'IDENTIFIER'
  name: string
}

export interface UnaryExpression extends ASTNode {
  type: 'UnaryExpression'
  op: Token
  operand: ASTNode
}

export interface BinaryExpression extends ASTNode {
  type: 'BinaryExpression'
  op: Token
  left: ASTNode
  right: ASTNode
}

export interface IfExpression extends ASTNode {
  type: 'IF'
  condition: ASTNode
  consequent: BlockStatement
  alternate?: BlockStatement
}

export interface FunctionDeclaration extends ASTNode {
  type: 'FunctionDeclaration'
  params: Identifier[]
  body: BlockStatement
  name?: string
}

export interface FunctionCall extends ASTNode {
  type: 'FunctionCall'
  callee: ASTNode
  arguments: ASTNode[]
}

export interface IndexExpression extends ASTNode {
  type: 'Index'
  object: ASTNode
  index: ASTNode
}

export interface ThisExpression extends ASTNode {
  type: 'ThisExpression'
}

export interface PropertyExpression extends ASTNode {
  type: 'PropertyExpression'
  object: ASTNode
  property: Identifier
}

export interface NewExpression extends ASTNode {
  type: 'NewExpression'
  callee: Identifier
  arguments: ASTNode[]
}

// Literals
export interface IntegerLiteral extends ASTNode {
  type: 'Integer'
  raw: number
}

export interface BooleanLiteral extends ASTNode {
  type: 'Boolean'
  raw: boolean
}

export interface StringLiteral extends ASTNode {
  type: 'String'
  raw: string
}

export interface ArrayLiteral extends ASTNode {
  type: 'Array'
  elements: ASTNode[]
}

export interface HashLiteral extends ASTNode {
  type: 'Hash'
  elements: [ASTNode, ASTNode][]
}

export type Literal =
  | IntegerLiteral
  | BooleanLiteral
  | StringLiteral
  | ArrayLiteral
  | HashLiteral
