// AST types are vendored from `@gengjiawen/monkey-minifier`. The prettier
// plugin and minifier already each maintain a copy; the linter is the third
// consumer. Vendoring keeps rule iteration off the wasm build cycle and avoids
// binding a cross-package AST refactor into the first release. See
// docs/linter-plan.md for the plan to extract `@gengjiawen/monkey-ast` once a
// third implementation has stabilized the shape.

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
  identifier: Token & { kind: IdentifierTokenKind }
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
  params: Identifier[]
  body: BlockStatement
}

export interface SetPropertyStatement extends ASTNode {
  type: 'SetPropertyStatement'
  object: Expression
  property: Identifier
  value: Expression
}

export type Statement =
  | LetStatement
  | ReturnStatement
  | ClassDeclaration
  | SetPropertyStatement
  | Expression

export interface Identifier extends ASTNode {
  type: 'IDENTIFIER'
  name: string
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
  params: Identifier[]
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
  raw: string
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

export function identifierName(statement: LetStatement): string {
  return statement.identifier.kind.value.name
}

export function tokenType(token: Token): string {
  return token.kind.type
}

// --- Linter data model (docs/linter-plan.md) --------------------------------

export type Severity = 'error' | 'warn'

export interface Diagnostic {
  /** Rule id, e.g. `no-unused-let`. */
  rule: string
  severity: Severity
  /** Human-facing one-liner, including identifier names and other context. */
  message: string
  /** UTF-8 byte offsets, matching the AST span. Optional: parser errors lack one. */
  span?: Span
}

/** Per-rule level override. `off` disables a rule entirely. */
export type RuleLevel = 'off' | 'warn' | 'error'

export interface LintOptions {
  /** Override default rule levels; only affects real lint rules. */
  rules?: Record<string, RuleLevel>
}

export interface LintResult {
  diagnostics: Diagnostic[]
}

export type AnalyzeResult =
  | { status: 'ok'; program: Program }
  | {
      status: 'error'
      stage: 'parse' | 'validation'
      message: string
      span?: Span | null
    }

/** The wasm entry the linter is built on: parse + validation → tagged JSON. */
export type AnalyzeLossless = (source: string) => string
