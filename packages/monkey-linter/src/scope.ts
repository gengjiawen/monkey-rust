import type {
  ClassDeclaration,
  Expression,
  FunctionDeclaration,
  Identifier,
  LetStatement,
  MethodDefinition,
  Program,
  Span,
  Statement,
} from './types'
import { identifierName } from './types'

// The seven predefined globals a fresh interpreter/compiler exposes. `print`
// is an alias of `puts` (object/builtins.rs reuses the same function).
export const BUILTIN_NAMES = [
  'len',
  'puts',
  'first',
  'last',
  'rest',
  'push',
  'print',
] as const

export type BindingKind = 'builtin' | 'class' | 'let' | 'parameter' | 'this'

export interface Binding {
  kind: BindingKind
  name: string
  references: Identifier[]
  /** Reportable declaration node; absent for `builtin` and `this`. */
  declaration?: LetStatement | ClassDeclaration | Identifier
  /** Span of the declared name, for precise diagnostics. */
  nameSpan?: Span
}

export interface ScopeAnalysis {
  bindings: Binding[]
  /** Every resolved identifier reference mapped to the binding it hit. */
  referenceBindings: Map<Identifier, Binding>
}

interface Scope {
  parent?: Scope
  names: Map<string, Binding>
}

interface Context {
  receiverAvailable: boolean
}

/**
 * Binding and reference analysis that mirrors `Compiler::compile_stmt`
 * ordering: a `let`'s right-hand side sees the *previous* binding of the same
 * name, and a `let`-bound function resolves its own name to that binding
 * (recursion). This is the same walk `@gengjiawen/monkey-minifier` uses for
 * scope/mangle; the linter only needs declarations, references, and shadowing.
 */
export function analyzeScopes(program: Program): ScopeAnalysis {
  const analysis: ScopeAnalysis = {
    bindings: [],
    referenceBindings: new Map(),
  }
  const root: Scope = { names: new Map() }
  for (const name of BUILTIN_NAMES) {
    define(root, createBinding(analysis, 'builtin', name))
  }
  analyzeStatements(program.body, root, analysis, { receiverAvailable: false })
  return analysis
}

function createBinding(
  analysis: ScopeAnalysis,
  kind: BindingKind,
  name: string
): Binding {
  const binding: Binding = { kind, name, references: [] }
  analysis.bindings.push(binding)
  return binding
}

function define(scope: Scope, binding: Binding): void {
  scope.names.set(binding.name, binding)
}

function resolve(scope: Scope, name: string): Binding | undefined {
  for (
    let current: Scope | undefined = scope;
    current;
    current = current.parent
  ) {
    const binding = current.names.get(name)
    if (binding) {
      return binding
    }
  }
  return undefined
}

function analyzeStatements(
  statements: Statement[],
  scope: Scope,
  analysis: ScopeAnalysis,
  context: Context
): void {
  for (const statement of statements) {
    analyzeStatement(statement, scope, analysis, context)
  }
}

function analyzeStatement(
  statement: Statement,
  scope: Scope,
  analysis: ScopeAnalysis,
  context: Context
): void {
  switch (statement.type) {
    case 'Let': {
      // Mirror the compiler: the RHS is analyzed against the preceding binding,
      // then the new binding shadows it.
      const binding = createBinding(analysis, 'let', identifierName(statement))
      binding.declaration = statement
      binding.nameSpan = statement.identifier.span
      analyzeExpression(statement.expr, scope, analysis, context, binding)
      define(scope, binding)
      return
    }
    case 'ReturnStatement':
      analyzeExpression(statement.argument, scope, analysis, context)
      return
    case 'ClassDeclaration':
      analyzeClass(statement, scope, analysis, context)
      return
    case 'SetPropertyStatement':
      analyzeExpression(statement.object, scope, analysis, context)
      analyzeExpression(statement.value, scope, analysis, context)
      return
    default:
      analyzeExpression(statement, scope, analysis, context)
  }
}

function analyzeClass(
  declaration: ClassDeclaration,
  scope: Scope,
  analysis: ScopeAnalysis,
  context: Context
): void {
  const binding = createBinding(analysis, 'class', declaration.name.name)
  binding.declaration = declaration
  binding.nameSpan = declaration.name.span
  // The class name is visible while its methods are analyzed (for `new Self()`).
  define(scope, binding)
  for (const method of declaration.methods) {
    analyzeMethod(method, scope, analysis)
  }
}

function analyzeMethod(
  method: MethodDefinition,
  parent: Scope,
  analysis: ScopeAnalysis
): void {
  const scope: Scope = { parent, names: new Map() }
  define(scope, createBinding(analysis, 'this', 'this'))
  for (const parameter of method.params) {
    defineParameter(parameter, scope, analysis)
  }
  analyzeStatements(method.body.body, scope, analysis, {
    receiverAvailable: true,
  })
}

function analyzeFunction(
  declaration: FunctionDeclaration,
  parent: Scope,
  analysis: ScopeAnalysis,
  context: Context,
  selfBinding?: Binding
): void {
  const scope: Scope = { parent, names: new Map() }
  if (selfBinding) {
    // A `let f = fn() { ... }` resolves `f` inside its own body to the `let`.
    define(scope, selfBinding)
  }
  for (const parameter of declaration.params) {
    defineParameter(parameter, scope, analysis)
  }
  analyzeStatements(declaration.body.body, scope, analysis, {
    receiverAvailable: context.receiverAvailable,
  })
}

function defineParameter(
  parameter: Identifier,
  scope: Scope,
  analysis: ScopeAnalysis
): void {
  const binding = createBinding(analysis, 'parameter', parameter.name)
  binding.declaration = parameter
  binding.nameSpan = parameter.span
  define(scope, binding)
}

function analyzeIdentifier(
  identifier: Identifier,
  scope: Scope,
  analysis: ScopeAnalysis
): void {
  const binding = resolve(scope, identifier.name)
  if (!binding) {
    // Unresolved references are validation errors caught before lint; a lint
    // run only reaches scope analysis on a validated tree.
    return
  }
  binding.references.push(identifier)
  analysis.referenceBindings.set(identifier, binding)
}

function analyzeExpression(
  expression: Expression,
  scope: Scope,
  analysis: ScopeAnalysis,
  context: Context,
  directLetBinding?: Binding
): void {
  switch (expression.type) {
    case 'IDENTIFIER':
      analyzeIdentifier(expression, scope, analysis)
      return
    case 'Integer':
    case 'Boolean':
    case 'String':
    case 'ThisExpression':
      return
    case 'Array':
      for (const element of expression.elements) {
        analyzeExpression(element, scope, analysis, context)
      }
      return
    case 'Hash':
      for (const [key, value] of expression.elements) {
        analyzeExpression(key, scope, analysis, context)
        analyzeExpression(value, scope, analysis, context)
      }
      return
    case 'UnaryExpression':
      analyzeExpression(expression.operand, scope, analysis, context)
      return
    case 'BinaryExpression':
      analyzeExpression(expression.left, scope, analysis, context)
      analyzeExpression(expression.right, scope, analysis, context)
      return
    case 'IF':
      // Branches share the enclosing scope, matching the compiler.
      analyzeExpression(expression.condition, scope, analysis, context)
      analyzeStatements(expression.consequent.body, scope, analysis, context)
      if (expression.alternate) {
        analyzeStatements(expression.alternate.body, scope, analysis, context)
      }
      return
    case 'FunctionDeclaration':
      analyzeFunction(expression, scope, analysis, context, directLetBinding)
      return
    case 'FunctionCall':
      analyzeExpression(expression.callee, scope, analysis, context)
      for (const argument of expression.arguments) {
        analyzeExpression(argument, scope, analysis, context)
      }
      return
    case 'Index':
      analyzeExpression(expression.object, scope, analysis, context)
      analyzeExpression(expression.index, scope, analysis, context)
      return
    case 'PropertyExpression':
      analyzeExpression(expression.object, scope, analysis, context)
      return
    case 'NewExpression':
      analyzeIdentifier(expression.callee, scope, analysis)
      for (const argument of expression.arguments) {
        analyzeExpression(argument, scope, analysis, context)
      }
      return
  }
}
