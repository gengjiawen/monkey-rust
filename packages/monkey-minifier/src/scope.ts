import type {
  ASTNode,
  BlockStatement,
  ClassDeclaration,
  Expression,
  FunctionDeclaration,
  Identifier,
  LetStatement,
  MethodDefinition,
  Program,
  Statement,
} from './types'
import { identifierName } from './types'

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
  id: number
  kind: BindingKind
  originalName: string
  preserve: boolean
  references: Identifier[]
  identifiers: Identifier[]
  lets: LetStatement[]
  functions: FunctionDeclaration[]
}

export interface ScopeAnalysis {
  bindings: Binding[]
  letBindings: Map<LetStatement, Binding>
  referenceBindings: Map<Identifier, Binding>
  unresolved: Set<Identifier>
  diagnosticNodes: Set<ASTNode>
  forbiddenNames: Set<string>
  safe: boolean
}

interface Scope {
  parent?: Scope
  names: Map<string, Binding>
}

interface Context {
  callable: 'constructor' | 'function' | 'method' | null
  receiverAvailable: boolean
}

export function analyzeScopes(program: Program): ScopeAnalysis {
  const analysis: ScopeAnalysis = {
    bindings: [],
    letBindings: new Map(),
    referenceBindings: new Map(),
    unresolved: new Set(),
    diagnosticNodes: new Set(),
    forbiddenNames: new Set(BUILTIN_NAMES),
    safe: true,
  }
  const root: Scope = { names: new Map() }
  for (const name of BUILTIN_NAMES) {
    define(root, createBinding(analysis, name, 'builtin', true))
  }
  analyzeStatements(program.body, root, analysis, {
    callable: null,
    receiverAvailable: false,
  })
  return analysis
}

function createBinding(
  analysis: ScopeAnalysis,
  name: string,
  kind: BindingKind,
  preserve: boolean
): Binding {
  const binding: Binding = {
    id: analysis.bindings.length,
    kind,
    originalName: name,
    preserve,
    references: [],
    identifiers: [],
    lets: [],
    functions: [],
  }
  analysis.bindings.push(binding)
  if (preserve) {
    analysis.forbiddenNames.add(name)
  }
  return binding
}

function define(scope: Scope, binding: Binding): void {
  scope.names.set(binding.originalName, binding)
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
      // Mirror Compiler::compile_stmt: the RHS sees the preceding binding.
      const binding = createBinding(
        analysis,
        identifierName(statement),
        'let',
        false
      )
      binding.lets.push(statement)
      analysis.letBindings.set(statement, binding)
      analyzeExpression(statement.expr, scope, analysis, context, binding)
      define(scope, binding)
      return
    }
    case 'ReturnStatement':
      if (context.callable === 'constructor') {
        analysis.diagnosticNodes.add(statement)
      }
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
  // A class is visible while its methods are compiled. Its spelling is
  // observable in rendered runtime values, so this binding is never mangled.
  const binding = createBinding(analysis, declaration.name.name, 'class', true)
  binding.identifiers.push(declaration.name)
  define(scope, binding)
  for (const method of declaration.methods) {
    analyzeMethod(method, scope, analysis, context)
  }
}

function analyzeMethod(
  method: MethodDefinition,
  parent: Scope,
  analysis: ScopeAnalysis,
  _context: Context
): void {
  const scope: Scope = { parent, names: new Map() }
  define(scope, createBinding(analysis, 'this', 'this', true))
  for (const parameter of method.params) {
    const binding = createBinding(analysis, parameter.name, 'parameter', false)
    binding.identifiers.push(parameter)
    define(scope, binding)
  }
  analyzeStatements(method.body.body, scope, analysis, {
    callable: method.kind === 'Constructor' ? 'constructor' : 'method',
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
  if (declaration.name) {
    const binding =
      selfBinding ?? createBinding(analysis, declaration.name, 'let', true)
    binding.functions.push(declaration)
    define(scope, binding)
  }
  for (const parameter of declaration.params) {
    const binding = createBinding(analysis, parameter.name, 'parameter', false)
    binding.identifiers.push(parameter)
    define(scope, binding)
  }
  analyzeStatements(declaration.body.body, scope, analysis, {
    callable: 'function',
    receiverAvailable: context.receiverAvailable,
  })
}

function analyzeIdentifier(
  identifier: Identifier,
  scope: Scope,
  analysis: ScopeAnalysis
): void {
  const binding = resolve(scope, identifier.name)
  if (!binding) {
    analysis.unresolved.add(identifier)
    analysis.diagnosticNodes.add(identifier)
    analysis.forbiddenNames.add(identifier.name)
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
      // Branches intentionally share this symbol table and are visited in the
      // compiler's source order.
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
    case 'ThisExpression':
      if (!context.receiverAvailable) {
        analysis.diagnosticNodes.add(expression)
      }
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
    default:
      analysis.safe = false
  }
}
