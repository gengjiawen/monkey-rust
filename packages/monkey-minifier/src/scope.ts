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
  // True when the binding's `let` sits inside an `if` arm: its slot is
  // written only on executions that take the branch, so a later reference
  // may still observe the unset slot.
  conditional: boolean
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

/** Identifies one block inside a scope; see [`enterBlock`]. */
type BlockId = number

/** A scope's own top level, outside every block. */
const TOP_LEVEL_BLOCK: BlockId = 0

interface Scope {
  parent?: Scope
  names: Map<string, Binding>
  // Blocks are not scopes in Monkey, but they are skippable: after an `if`, a
  // name an arm rebound means either that arm's last `let` or the binding from
  // before the branch, depending on which way the jump went
  // (compiler/symbol_table.rs). Both are therefore live at every read after
  // the block, and both must end up under one name — so the model here folds
  // them into one binding. Recording which block introduced each name is what
  // tells that case apart from a redefinition inside one block, where a
  // closure made in between keeps reading its own `let`.
  definitionBlock: Map<string, BlockId>
  currentBlock: BlockId
  openBlocks: BlockId[]
  nextBlock: BlockId
}

function createScope(parent?: Scope): Scope {
  return {
    parent,
    names: new Map(),
    definitionBlock: new Map(),
    currentBlock: TOP_LEVEL_BLOCK,
    openBlocks: [],
    nextBlock: TOP_LEVEL_BLOCK + 1,
  }
}

/**
 * Opens a block. Every block gets an id of its own, so two sibling `if` arms
 * defining the same name are redefinitions of one binding rather than two
 * independent ones.
 */
function enterBlock(scope: Scope): void {
  scope.openBlocks.push(scope.currentBlock)
  scope.currentBlock = scope.nextBlock
  scope.nextBlock += 1
}

function leaveBlock(scope: Scope): void {
  scope.currentBlock = scope.openBlocks.pop() ?? TOP_LEVEL_BLOCK
}

/**
 * The binding a `let` named `name` here would shadow for the code after the
 * block, or `undefined` when there is none to shadow — either because nothing
 * binds the name yet or because this very block bound it, in which case the
 * `let` is an ordinary redefinition and takes a binding of its own.
 *
 * A `let` outside every block is not shadowing anything either: it runs
 * unconditionally and displaces what came before it outright.
 */
function shadowedByBlock(scope: Scope, name: string): Binding | undefined {
  if (scope.currentBlock === TOP_LEVEL_BLOCK) {
    return undefined
  }
  const own = scope.names.get(name)
  if (own) {
    return scope.definitionBlock.get(name) === scope.currentBlock
      ? undefined
      : own
  }
  return scope.parent && resolve(scope.parent, name)
}

interface Context {
  callable: 'constructor' | 'function' | 'method' | null
  receiverAvailable: boolean
  conditional: boolean
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
  const root = createScope()
  for (const name of BUILTIN_NAMES) {
    define(root, createBinding(analysis, name, 'builtin', true))
  }
  analyzeStatements(program.body, root, analysis, {
    callable: null,
    receiverAvailable: false,
    conditional: false,
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
    conditional: false,
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

function define(
  scope: Scope,
  binding: Binding,
  block: BlockId = scope.currentBlock
): void {
  scope.names.set(binding.originalName, binding)
  scope.definitionBlock.set(binding.originalName, block)
}

/** Bindings the mangler may rename, and the only ones a block may rebind. */
export function isUserBinding(binding: Binding): boolean {
  return binding.kind === 'let' || binding.kind === 'parameter'
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

/** One `if` arm: its own block, mirroring compile_block_statement_as_value. */
function analyzeBlock(
  block: BlockStatement,
  scope: Scope,
  analysis: ScopeAnalysis,
  context: Context
): void {
  enterBlock(scope)
  analyzeStatements(block.body, scope, analysis, context)
  leaveBlock(scope)
}

function analyzeStatement(
  statement: Statement,
  scope: Scope,
  analysis: ScopeAnalysis,
  context: Context
): void {
  switch (statement.type) {
    case 'Let': {
      const name = identifierName(statement)
      // A `let` in an `if` arm that shadows a binding from outside the arm
      // joins it rather than starting a binding of its own: after the block
      // the name means one or the other depending on which way the jump went,
      // so both have to survive DCE together and be renamed alike
      // (compiler/symbol_table.rs, #335).
      const shadowed = shadowedByBlock(scope, name)
      if (shadowed && !isUserBinding(shadowed)) {
        // A block shadowing a class name or a builtin cannot be renamed into
        // one binding with it; leave such a program alone rather than guess.
        analysis.safe = false
      }
      const binding =
        shadowed && isUserBinding(shadowed)
          ? shadowed
          : createBinding(analysis, name, 'let', false)

      binding.conditional = binding.conditional || context.conditional
      binding.lets.push(statement)
      analysis.letBindings.set(statement, binding)
      // Mirror Compiler::compile_stmt: the RHS sees the preceding binding,
      // which for a joining `let` is the very binding it is about to write.
      analyzeExpression(statement.expr, scope, analysis, context, binding)
      // A joining `let` leaves the name owned by the block it came from — the
      // enclosing scope counts as this scope's top level — so a second `let`
      // of it in this same arm joins as well. The arm's last one is what a
      // read after the block sees, and it only carries the right name if it is
      // the same binding. Leaving the block therefore restores nothing: the
      // name never moved, and the next `let` outside the arm binds anew.
      define(
        scope,
        binding,
        shadowed
          ? scope.definitionBlock.get(name) ?? TOP_LEVEL_BLOCK
          : scope.currentBlock
      )
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
    case 'DebuggerStatement':
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
  const scope = createScope(parent)
  define(scope, createBinding(analysis, 'this', 'this', true))
  for (const parameter of method.params) {
    const binding = createBinding(
      analysis,
      parameter.identifier.name,
      'parameter',
      false
    )
    binding.identifiers.push(parameter.identifier)
    define(scope, binding)
  }
  analyzeStatements(method.body.body, scope, analysis, {
    callable: method.kind === 'Constructor' ? 'constructor' : 'method',
    receiverAvailable: true,
    conditional: false,
  })
}

function analyzeFunction(
  declaration: FunctionDeclaration,
  parent: Scope,
  analysis: ScopeAnalysis,
  context: Context,
  selfBinding?: Binding
): void {
  const scope = createScope(parent)
  if (declaration.name) {
    const binding =
      selfBinding ?? createBinding(analysis, declaration.name, 'let', true)
    binding.functions.push(declaration)
    define(scope, binding)
  }
  for (const parameter of declaration.params) {
    const binding = createBinding(
      analysis,
      parameter.identifier.name,
      'parameter',
      false
    )
    binding.identifiers.push(parameter.identifier)
    define(scope, binding)
  }
  // A body's own top-level `let`s run on every call, so conditionality does
  // not carry across the callable boundary.
  analyzeStatements(declaration.body.body, scope, analysis, {
    callable: 'function',
    receiverAvailable: context.receiverAvailable,
    conditional: false,
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
    case 'IF': {
      // Branches intentionally share this symbol table and are visited in the
      // compiler's source order.
      analyzeExpression(expression.condition, scope, analysis, context)
      const branch = { ...context, conditional: true }
      analyzeBlock(expression.consequent, scope, analysis, branch)
      if (expression.alternate) {
        analyzeBlock(expression.alternate, scope, analysis, branch)
      }
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
