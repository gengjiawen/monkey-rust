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
  /** References with one definite target, mapped to that binding. */
  referenceBindings: Map<Identifier, Binding>
}

interface Scope {
  parent?: Scope
  // More than one binding is possible after a conditional: the interpreter
  // mutates the environment of only the arm that executes. Keeping every
  // candidate lets later references count conservatively as uses of all
  // bindings they may observe.
  names: Map<string, Binding[]>
}

interface Context {
  receiverAvailable: boolean
}

/**
 * Binding and reference analysis. A `let`'s right-hand side sees the previous
 * binding of the same name, while a directly let-bound function resolves its
 * own name inside its body for recursion. Conditional arms start from the same
 * entering environment and their possible bindings are merged conservatively;
 * this avoids source-order guesses where the interpreter executes only one arm.
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
  scope.names.set(binding.name, [binding])
}

function resolve(scope: Scope, name: string): Binding[] | undefined {
  for (
    let current: Scope | undefined = scope;
    current;
    current = current.parent
  ) {
    const bindings = current.names.get(name)
    if (bindings) {
      return bindings
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
  const bindings = resolve(scope, identifier.name)
  if (!bindings) {
    // Unresolved references are validation errors caught before lint; a lint
    // run only reaches scope analysis on a validated tree.
    return
  }
  for (const binding of bindings) {
    binding.references.push(identifier)
  }
  // Rules such as builtin-arity require a definite binding. An ambiguous
  // post-branch reference is deliberately left unmapped.
  if (bindings.length === 1) {
    analysis.referenceBindings.set(identifier, bindings[0])
  }
}

function forkScope(scope: Scope): Scope {
  return { parent: scope.parent, names: new Map(scope.names) }
}

function mergeBranchScopes(
  scope: Scope,
  consequent: Scope,
  alternate: Scope
): void {
  const names = new Set([...consequent.names.keys(), ...alternate.names.keys()])
  for (const name of names) {
    const candidates = [
      ...(resolve(consequent, name) ?? []),
      ...(resolve(alternate, name) ?? []),
    ]
    scope.names.set(name, [...new Set(candidates)])
  }
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
    case 'IF': {
      analyzeExpression(expression.condition, scope, analysis, context)
      // The interpreter executes exactly one arm against the entering
      // environment. Analyze each arm from that same state so a declaration in
      // the consequence cannot capture references in the alternative.
      const consequent = forkScope(scope)
      analyzeStatements(
        expression.consequent.body,
        consequent,
        analysis,
        context
      )
      const alternate = forkScope(scope)
      if (expression.alternate) {
        analyzeStatements(
          expression.alternate.body,
          alternate,
          analysis,
          context
        )
      }
      // A later reference can observe the binding left behind by either path.
      // Credit every candidate rather than inventing a definite source-order
      // winner and risking a false unused-binding diagnostic.
      mergeBranchScopes(scope, consequent, alternate)
      return
    }
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
