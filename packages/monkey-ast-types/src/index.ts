// The single source of truth for the shape of the JSON the Monkey wasm parser
// emits. The linter, minifier, prettier plugin and type checker all decode the
// same tree, so the node definitions live here instead of being copied into
// each package.
//
// Two wasm entries produce this tree and they differ in exactly one place:
// `analyze_lossless` keeps integer literals as their source text, while the
// plain `parse` entry has already turned them into JS numbers. `raw` is typed
// for both; call `String(...)` when you need the text.

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
  name: Identifier
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

const TYPE_ANNOTATION_TYPES = new Set([
  'NamedType',
  'ArrayType',
  'HashType',
  'FunctionType',
  'OptionalType',
])

/** True for the five annotation nodes, which no runtime tool needs to walk. */
export function isTypeAnnotation(node: ASTNode): node is TypeAnnotation {
  return TYPE_ANNOTATION_TYPES.has(node.type)
}

/** Renders an annotation back to source; the result re-parses identically. */
export function printTypeAnnotation(annotation: TypeAnnotation): string {
  switch (annotation.type) {
    case 'NamedType':
      return annotation.name
    case 'ArrayType':
      return `[${printTypeAnnotation(annotation.element)}]`
    case 'HashType': {
      const key = printTypeAnnotation(annotation.key)
      return `{${key}: ${printTypeAnnotation(annotation.value)}}`
    }
    case 'FunctionType': {
      const params = annotation.params.map(printTypeAnnotation).join(', ')
      return `fn(${params}): ${printTypeAnnotation(annotation.return_type)}`
    }
    case 'OptionalType':
      // `fn(int): int?` would read the `?` as part of the return type, so a
      // nullable function type needs its grouping back.
      return annotation.inner.type === 'FunctionType'
        ? `(${printTypeAnnotation(annotation.inner)})?`
        : `${printTypeAnnotation(annotation.inner)}?`
  }
}

export function identifierName(statement: LetStatement): string {
  return statement.identifier.name
}

export function setIdentifierName(statement: LetStatement, name: string): void {
  statement.identifier.name = name
}

export function tokenType(token: Token): string {
  return token.kind.type
}
