// The shape of the JSON tree `parse` and `analyze_lossless` emit. wasm-bindgen
// appends this file to the generated `monkey_wasm.d.ts` (see ast_types.rs), so
// the package that ships the parser also ships the types for its output, and
// the two can never drift apart. The linter, minifier, prettier plugin and
// type checker all import these instead of keeping their own copies.
//
// The two entries differ in exactly one place: `analyze_lossless` keeps
// integer literals as their source text, while the plain `parse` entry has
// already turned them into JS numbers. `raw` is typed for both; call
// `String(...)` when you need the text.

export interface Span {
  start: number
  end: number
}

export interface TokenKind {
  type: string
  value?: unknown
}

export interface IdentifierTokenKind extends TokenKind {
  type: 'IDENTIFIER'
  value: { name: string }
}

export interface Token {
  kind: TokenKind
  span?: Span
}

export interface ASTNode {
  type: string
  span?: Span
}

export interface Program extends ASTNode {
  type: 'Program'
  body: Statement[]
}

export interface BlockStatement extends ASTNode {
  type: 'BlockStatement'
  body: Statement[]
}

export interface LetStatement extends ASTNode {
  type: 'Let'
  identifier: Identifier
  type_annotation?: TypeAnnotation | null
  expr: Expression
}

export interface ReturnStatement extends ASTNode {
  type: 'ReturnStatement'
  argument: Expression
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
  params: Param[]
  /** Always absent on a constructor: the parser rejects the annotation. */
  return_type?: TypeAnnotation | null
  body: BlockStatement
}

export interface SetPropertyStatement extends ASTNode {
  type: 'SetPropertyStatement'
  object: Expression
  property: Identifier
  value: Expression
}

export interface DebuggerStatement extends ASTNode {
  type: 'DebuggerStatement'
}

export type Statement =
  | LetStatement
  | ReturnStatement
  | ClassDeclaration
  | SetPropertyStatement
  | DebuggerStatement
  | Expression

export interface Identifier extends ASTNode {
  type: 'IDENTIFIER'
  name: string
}

/** A function or method parameter: a name plus its optional annotation. */
export interface Param extends ASTNode {
  type: 'Param'
  identifier: Identifier
  type_annotation?: TypeAnnotation | null
}

export interface UnaryExpression extends ASTNode {
  type: 'UnaryExpression'
  op: Token
  operand: Expression
}

export interface BinaryExpression extends ASTNode {
  type: 'BinaryExpression'
  op: Token
  left: Expression
  right: Expression
}

export interface IfExpression extends ASTNode {
  type: 'IF'
  condition: Expression
  consequent: BlockStatement
  alternate?: BlockStatement | null
}

export interface FunctionDeclaration extends ASTNode {
  type: 'FunctionDeclaration'
  params: Param[]
  return_type?: TypeAnnotation | null
  body: BlockStatement
  name: string
}

export interface FunctionCall extends ASTNode {
  type: 'FunctionCall'
  callee: Expression
  arguments: Expression[]
}

export interface IndexExpression extends ASTNode {
  type: 'Index'
  object: Expression
  index: Expression
}

export interface ThisExpression extends ASTNode {
  type: 'ThisExpression'
}

export interface PropertyExpression extends ASTNode {
  type: 'PropertyExpression'
  object: Expression
  property: Identifier
}

export interface NewExpression extends ASTNode {
  type: 'NewExpression'
  callee: Identifier
  arguments: Expression[]
}

export interface IntegerLiteral extends ASTNode {
  type: 'Integer'
  /** Source text from `analyze_lossless`, a number from `parse`. */
  raw: string | number
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
  elements: Expression[]
}

export interface HashLiteral extends ASTNode {
  type: 'Hash'
  elements: [Expression, Expression][]
}

export type Literal =
  | IntegerLiteral
  | BooleanLiteral
  | StringLiteral
  | ArrayLiteral
  | HashLiteral

export type Expression =
  | Identifier
  | Literal
  | UnaryExpression
  | BinaryExpression
  | IfExpression
  | FunctionDeclaration
  | FunctionCall
  | IndexExpression
  | ThisExpression
  | PropertyExpression
  | NewExpression

// --- Type annotations -------------------------------------------------------
//
// Annotations are optional syntax that every execution backend erases, so a
// tool that only cares about runtime behavior can skip these subtrees whole.
// See docs/type-system-design.md sections 5 and 6.

export interface NamedType extends ASTNode {
  type: 'NamedType'
  /** `int` | `bool` | `string` | `any` | `null` | a class name */
  name: string
}

export interface ArrayType extends ASTNode {
  type: 'ArrayType'
  element: TypeAnnotation
}

export interface HashType extends ASTNode {
  type: 'HashType'
  key: TypeAnnotation
  value: TypeAnnotation
}

export interface FunctionType extends ASTNode {
  type: 'FunctionType'
  params: TypeAnnotation[]
  return_type: TypeAnnotation
}

export interface OptionalType extends ASTNode {
  type: 'OptionalType'
  inner: TypeAnnotation
}

export type TypeAnnotation =
  | NamedType
  | ArrayType
  | HashType
  | FunctionType
  | OptionalType
