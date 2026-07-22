import type {
  ASTNode,
  BlockStatement,
  Expression,
  FunctionDeclaration,
  HashLiteral,
  Program,
  Statement,
} from './types'
import { tokenType } from './types'
import { analyzeScopes, type ScopeAnalysis } from './scope'

const ZERO = BigInt(0)
const NEGATIVE_ONE = BigInt(-1)
const I64_MIN = -(BigInt(1) << BigInt(63))

type ConstantValue =
  | { kind: 'integer'; value: bigint }
  | { kind: 'boolean'; value: boolean }
  | { kind: 'string'; value: string }

export function foldConstants(program: Program): Program {
  const analysis = analyzeScopes(program)
  if (!analysis.safe) {
    return program
  }
  foldStatements(program.body, analysis)
  return program
}

export function eliminateDeadLets(program: Program): Program {
  for (;;) {
    const analysis = analyzeScopes(program)
    if (!analysis.safe) {
      return program
    }
    const removed = removeDeadStatements(program.body, analysis, false, true)
    if (!removed) {
      return program
    }
  }
}

function foldStatements(
  statements: Statement[],
  analysis: ScopeAnalysis
): void {
  for (let index = 0; index < statements.length; index += 1) {
    const statement = statements[index]
    switch (statement.type) {
      case 'Let':
        statement.expr = foldLetExpression(statement.expr, analysis)
        break
      case 'ReturnStatement':
        statement.argument = foldExpression(statement.argument, analysis)
        break
      case 'ClassDeclaration':
        for (const method of statement.methods) {
          foldStatements(method.body.body, analysis)
        }
        break
      case 'SetPropertyStatement':
        statement.object = foldExpression(statement.object, analysis)
        statement.value = foldExpression(statement.value, analysis)
        break
      default:
        statements[index] = foldExpression(statement, analysis)
    }
  }
}

function foldLetExpression(
  expression: Expression,
  analysis: ScopeAnalysis
): Expression {
  const folded = foldExpression(expression, analysis)
  // The parser names a function only when it is the direct RHS of a `let`,
  // giving its body a recursive self binding. Folding an `if` (or a future
  // wrapper expression) into an anonymous function at this position would
  // therefore change captured-name resolution after printing and reparsing.
  return expression.type !== 'FunctionDeclaration' &&
    folded.type === 'FunctionDeclaration'
    ? expression
    : folded
}

function foldBlock(block: BlockStatement, analysis: ScopeAnalysis): void {
  foldStatements(block.body, analysis)
}

function foldExpression(
  expression: Expression,
  analysis: ScopeAnalysis
): Expression {
  switch (expression.type) {
    case 'IDENTIFIER':
    case 'Integer':
    case 'Boolean':
    case 'String':
    case 'ThisExpression':
      return expression
    case 'Array':
      expression.elements = expression.elements.map((element) =>
        foldExpression(element, analysis)
      )
      return expression
    case 'Hash':
      expression.elements = expression.elements.map(([key, value]) => [
        foldExpression(key, analysis),
        foldExpression(value, analysis),
      ])
      return expression
    case 'UnaryExpression':
      expression.operand = foldExpression(expression.operand, analysis)
      return constantToExpression(evaluateConstant(expression)) ?? expression
    case 'BinaryExpression':
      expression.left = foldExpression(expression.left, analysis)
      expression.right = foldExpression(expression.right, analysis)
      return constantToExpression(evaluateConstant(expression)) ?? expression
    case 'IF': {
      const hasDiagnostic = containsDiagnostic(expression, analysis)
      expression.condition = foldExpression(expression.condition, analysis)
      foldBlock(expression.consequent, analysis)
      if (expression.alternate) {
        foldBlock(expression.alternate, analysis)
      }
      const condition = evaluateConstant(expression.condition)
      if (
        hasDiagnostic ||
        !condition ||
        branchesChangeScope(expression.consequent, expression.alternate)
      ) {
        return expression
      }
      const chosen = isTruthy(condition)
        ? expression.consequent
        : expression.alternate
      if (
        !chosen ||
        chosen.body.length !== 1 ||
        !isExpression(chosen.body[0])
      ) {
        return expression
      }
      return chosen.body[0]
    }
    case 'FunctionDeclaration':
      foldBlock(expression.body, analysis)
      return expression
    case 'FunctionCall':
      expression.callee = foldExpression(expression.callee, analysis)
      expression.arguments = expression.arguments.map((argument) =>
        foldExpression(argument, analysis)
      )
      return expression
    case 'Index':
      expression.object = foldExpression(expression.object, analysis)
      expression.index = foldExpression(expression.index, analysis)
      return expression
    case 'PropertyExpression':
      expression.object = foldExpression(expression.object, analysis)
      return expression
    case 'NewExpression':
      expression.arguments = expression.arguments.map((argument) =>
        foldExpression(argument, analysis)
      )
      return expression
  }
}

function evaluateConstant(expression: Expression): ConstantValue | null {
  switch (expression.type) {
    case 'Integer':
      return { kind: 'integer', value: BigInt(expression.raw) }
    case 'Boolean':
      return { kind: 'boolean', value: expression.raw }
    case 'String':
      return { kind: 'string', value: expression.raw }
    case 'UnaryExpression': {
      const operand = evaluateConstant(expression.operand)
      if (!operand) {
        return null
      }
      switch (tokenType(expression.op)) {
        case 'MINUS':
          return operand.kind === 'integer'
            ? { kind: 'integer', value: BigInt.asIntN(64, -operand.value) }
            : null
        case 'BANG':
          return {
            kind: 'boolean',
            value: operand.kind === 'boolean' ? !operand.value : false,
          }
        default:
          return null
      }
    }
    case 'BinaryExpression': {
      const left = evaluateConstant(expression.left)
      const right = evaluateConstant(expression.right)
      return left && right
        ? evaluateBinary(tokenType(expression.op), left, right)
        : null
    }
    default:
      return null
  }
}

function evaluateBinary(
  operator: string,
  left: ConstantValue,
  right: ConstantValue
): ConstantValue | null {
  if (left.kind === 'integer' && right.kind === 'integer') {
    switch (operator) {
      case 'PLUS':
        return integer(BigInt.asIntN(64, left.value + right.value))
      case 'MINUS':
        return integer(BigInt.asIntN(64, left.value - right.value))
      case 'ASTERISK':
        return integer(BigInt.asIntN(64, left.value * right.value))
      case 'SLASH':
        if (
          right.value === ZERO ||
          (left.value === I64_MIN && right.value === NEGATIVE_ONE)
        ) {
          return null
        }
        return integer(left.value / right.value)
      case 'LT':
        return boolean(left.value < right.value)
      case 'GT':
        return boolean(left.value > right.value)
      case 'EQ':
        return boolean(left.value === right.value)
      case 'NotEq':
        return boolean(left.value !== right.value)
      default:
        return null
    }
  }
  if (left.kind === 'boolean' && right.kind === 'boolean') {
    if (operator === 'EQ' || operator === 'NotEq') {
      return boolean(
        operator === 'EQ'
          ? left.value === right.value
          : left.value !== right.value
      )
    }
    return null
  }
  if (left.kind === 'string' && right.kind === 'string') {
    switch (operator) {
      case 'PLUS':
        return { kind: 'string', value: left.value + right.value }
      case 'EQ':
        return boolean(left.value === right.value)
      case 'NotEq':
        return boolean(left.value !== right.value)
      default:
        return null
    }
  }
  return null
}

function integer(value: bigint): ConstantValue {
  return { kind: 'integer', value }
}

function boolean(value: boolean): ConstantValue {
  return { kind: 'boolean', value }
}

function constantToExpression(value: ConstantValue | null): Expression | null {
  if (!value) {
    return null
  }
  switch (value.kind) {
    case 'boolean':
      return { type: 'Boolean', raw: value.value }
    case 'string':
      return { type: 'String', raw: value.value }
    case 'integer':
      if (value.value === I64_MIN) {
        return null
      }
      if (value.value < ZERO) {
        return {
          type: 'UnaryExpression',
          op: { kind: { type: 'MINUS' } },
          operand: { type: 'Integer', raw: (-value.value).toString() },
        }
      }
      return { type: 'Integer', raw: value.value.toString() }
  }
}

function isTruthy(value: ConstantValue): boolean {
  return value.kind === 'boolean' ? value.value : true
}

function branchesChangeScope(
  consequent: BlockStatement,
  alternate: BlockStatement | null | undefined
): boolean {
  return (
    blockChangesScope(consequent) ||
    (alternate ? blockChangesScope(alternate) : false)
  )
}

function blockChangesScope(block: BlockStatement): boolean {
  return block.body.some(statementChangesScope)
}

function statementChangesScope(statement: Statement): boolean {
  switch (statement.type) {
    case 'Let':
    case 'ClassDeclaration':
      return true
    case 'ReturnStatement':
      return expressionChangesScope(statement.argument)
    case 'SetPropertyStatement':
      return (
        expressionChangesScope(statement.object) ||
        expressionChangesScope(statement.value)
      )
    default:
      return expressionChangesScope(statement)
  }
}

function expressionChangesScope(expression: Expression): boolean {
  switch (expression.type) {
    case 'IF':
      return (
        expressionChangesScope(expression.condition) ||
        blockChangesScope(expression.consequent) ||
        (expression.alternate ? blockChangesScope(expression.alternate) : false)
      )
    case 'UnaryExpression':
      return expressionChangesScope(expression.operand)
    case 'BinaryExpression':
      return (
        expressionChangesScope(expression.left) ||
        expressionChangesScope(expression.right)
      )
    case 'Array':
      return expression.elements.some(expressionChangesScope)
    case 'Hash':
      return expression.elements.some(
        ([key, value]) =>
          expressionChangesScope(key) || expressionChangesScope(value)
      )
    case 'FunctionCall':
      return (
        expressionChangesScope(expression.callee) ||
        expression.arguments.some(expressionChangesScope)
      )
    case 'Index':
      return (
        expressionChangesScope(expression.object) ||
        expressionChangesScope(expression.index)
      )
    case 'PropertyExpression':
      return expressionChangesScope(expression.object)
    case 'NewExpression':
      return expression.arguments.some(expressionChangesScope)
    // Function bodies have their own compiler symbol scope.
    case 'FunctionDeclaration':
    default:
      return false
  }
}

function isExpression(statement: Statement): statement is Expression {
  return ![
    'Let',
    'ReturnStatement',
    'ClassDeclaration',
    'SetPropertyStatement',
  ].includes(statement.type)
}

function removeDeadStatements(
  statements: Statement[],
  analysis: ScopeAnalysis,
  preserveTrailingLet: boolean,
  removeLets: boolean
): boolean {
  let removed = false
  const retained: Statement[] = []
  for (const [index, statement] of statements.entries()) {
    if (removeLets && statement.type === 'Let') {
      const binding = analysis.letBindings.get(statement)
      if (
        binding &&
        binding.references.length === 0 &&
        isPureTotal(statement.expr, analysis) &&
        // A trailing let is a value barrier in function/method bodies and in
        // either arm of an if expression. Removing it can expose the previous
        // expression as an implicit return/branch value. Keeping the final
        // candidate is conservative and lets earlier dead bindings disappear.
        (!preserveTrailingLet || index !== statements.length - 1)
      ) {
        removed = true
        continue
      }
    }
    removed = removeNested(statement, analysis, removeLets) || removed
    retained.push(statement)
  }
  statements.splice(0, statements.length, ...retained)
  return removed
}

function removeNested(
  statement: Statement,
  analysis: ScopeAnalysis,
  removeLets: boolean
): boolean {
  switch (statement.type) {
    case 'Let':
      return removeNestedExpression(statement.expr, analysis, removeLets)
    case 'ReturnStatement':
      return removeNestedExpression(statement.argument, analysis, removeLets)
    case 'ClassDeclaration':
      return statement.methods.reduce(
        (removed, method) =>
          removeDeadStatements(
            method.body.body,
            analysis,
            true,
            !callableHasIncompleteIf(method.body)
          ) || removed,
        false
      )
    case 'SetPropertyStatement':
      return (
        removeNestedExpression(statement.object, analysis, removeLets) ||
        removeNestedExpression(statement.value, analysis, removeLets)
      )
    default:
      return removeNestedExpression(statement, analysis, removeLets)
  }
}

function removeNestedExpression(
  expression: Expression,
  analysis: ScopeAnalysis,
  removeLets: boolean
): boolean {
  switch (expression.type) {
    case 'FunctionDeclaration': {
      const removeFunctionLets = !callableHasIncompleteIf(expression.body)
      return removeDeadStatements(
        expression.body.body,
        analysis,
        true,
        removeFunctionLets
      )
    }
    case 'IF':
      return (
        removeDeadStatements(
          expression.consequent.body,
          analysis,
          true,
          removeLets
        ) ||
        (expression.alternate
          ? removeDeadStatements(
              expression.alternate.body,
              analysis,
              true,
              removeLets
            )
          : false) ||
        removeNestedExpression(expression.condition, analysis, removeLets)
      )
    case 'UnaryExpression':
      return removeNestedExpression(expression.operand, analysis, removeLets)
    case 'BinaryExpression':
      return (
        removeNestedExpression(expression.left, analysis, removeLets) ||
        removeNestedExpression(expression.right, analysis, removeLets)
      )
    case 'Array':
      return expression.elements.reduce(
        (removed, item) =>
          removeNestedExpression(item, analysis, removeLets) || removed,
        false
      )
    case 'Hash':
      return expression.elements.reduce(
        (removed, [key, value]) =>
          removeNestedExpression(key, analysis, removeLets) ||
          removeNestedExpression(value, analysis, removeLets) ||
          removed,
        false
      )
    case 'FunctionCall':
      return [expression.callee, ...expression.arguments].reduce(
        (removed, item) =>
          removeNestedExpression(item, analysis, removeLets) || removed,
        false
      )
    case 'Index':
      return (
        removeNestedExpression(expression.object, analysis, removeLets) ||
        removeNestedExpression(expression.index, analysis, removeLets)
      )
    case 'PropertyExpression':
      return removeNestedExpression(expression.object, analysis, removeLets)
    case 'NewExpression':
      return expression.arguments.reduce(
        (removed, item) =>
          removeNestedExpression(item, analysis, removeLets) || removed,
        false
      )
    default:
      return false
  }
}

// The compiler reserves every local slot when a function is entered. An `if`
// arm that falls through without producing a value (an empty arm or one ending
// in `let`) can make the surrounding expression consume one of those slots.
// Removing any local from that callable then changes which value is consumed,
// even when the removed binding itself has no references. Keep its local-slot
// layout intact; nested functions and methods are assessed independently.
function callableHasIncompleteIf(body: BlockStatement): boolean {
  return blockContainsIncompleteIf(body)
}

function blockContainsIncompleteIf(block: BlockStatement): boolean {
  return block.body.some(statementContainsIncompleteIf)
}

function statementContainsIncompleteIf(statement: Statement): boolean {
  switch (statement.type) {
    case 'Let':
      return expressionContainsIncompleteIf(statement.expr)
    case 'ReturnStatement':
      return expressionContainsIncompleteIf(statement.argument)
    case 'ClassDeclaration':
      return false
    case 'SetPropertyStatement':
      return (
        expressionContainsIncompleteIf(statement.object) ||
        expressionContainsIncompleteIf(statement.value)
      )
    default:
      return expressionContainsIncompleteIf(statement)
  }
}

function expressionContainsIncompleteIf(expression: Expression): boolean {
  switch (expression.type) {
    case 'IF':
      return (
        ifCanFallThroughWithoutValue(expression) ||
        expressionContainsIncompleteIf(expression.condition) ||
        blockContainsIncompleteIf(expression.consequent) ||
        (expression.alternate
          ? blockContainsIncompleteIf(expression.alternate)
          : false)
      )
    case 'UnaryExpression':
      return expressionContainsIncompleteIf(expression.operand)
    case 'BinaryExpression':
      return (
        expressionContainsIncompleteIf(expression.left) ||
        expressionContainsIncompleteIf(expression.right)
      )
    case 'Array':
      return expression.elements.some(expressionContainsIncompleteIf)
    case 'Hash':
      return expression.elements.some(
        ([key, value]) =>
          expressionContainsIncompleteIf(key) ||
          expressionContainsIncompleteIf(value)
      )
    case 'FunctionCall':
      return (
        expressionContainsIncompleteIf(expression.callee) ||
        expression.arguments.some(expressionContainsIncompleteIf)
      )
    case 'Index':
      return (
        expressionContainsIncompleteIf(expression.object) ||
        expressionContainsIncompleteIf(expression.index)
      )
    case 'PropertyExpression':
      return expressionContainsIncompleteIf(expression.object)
    case 'NewExpression':
      return expression.arguments.some(expressionContainsIncompleteIf)
    case 'FunctionDeclaration':
    default:
      return false
  }
}

function ifCanFallThroughWithoutValue(expression: {
  consequent: BlockStatement
  alternate?: BlockStatement | null
}): boolean {
  return (
    blockCanFallThroughWithoutValue(expression.consequent) ||
    (expression.alternate
      ? blockCanFallThroughWithoutValue(expression.alternate)
      : false)
  )
}

function blockCanFallThroughWithoutValue(block: BlockStatement): boolean {
  const last = block.body[block.body.length - 1]
  if (!last) {
    return true
  }
  switch (last.type) {
    case 'Let':
      return true
    case 'ReturnStatement':
    case 'ClassDeclaration':
    case 'SetPropertyStatement':
      return false
    case 'IF':
      return ifCanFallThroughWithoutValue(last)
    default:
      return false
  }
}

function isPureTotal(expression: Expression, analysis: ScopeAnalysis): boolean {
  switch (expression.type) {
    case 'Integer':
    case 'Boolean':
    case 'String':
      return true
    case 'IDENTIFIER':
      return analysis.referenceBindings.has(expression)
    case 'Array':
      return expression.elements.every((element) =>
        isPureTotal(element, analysis)
      )
    case 'Hash':
      return hashIsPureTotal(expression, analysis)
    case 'UnaryExpression':
      return (
        isPureTotal(expression.operand, analysis) &&
        (tokenType(expression.op) === 'BANG' ||
          (tokenType(expression.op) === 'MINUS' &&
            evaluateConstant(expression.operand)?.kind === 'integer'))
      )
    case 'BinaryExpression':
      return (
        isPureTotal(expression.left, analysis) &&
        isPureTotal(expression.right, analysis) &&
        evaluateConstant(expression) !== null
      )
    case 'FunctionDeclaration':
      return !containsDiagnostic(expression, analysis)
    default:
      // Calls/new can have effects. Property/index operations may throw. If
      // and unknown future nodes are deliberately retained.
      return false
  }
}

function hashIsPureTotal(hash: HashLiteral, analysis: ScopeAnalysis): boolean {
  return hash.elements.every(([key, value]) => {
    const constant = evaluateConstant(key)
    return (
      constant !== null &&
      isPureTotal(key, analysis) &&
      isPureTotal(value, analysis)
    )
  })
}

function containsDiagnostic(node: ASTNode, analysis: ScopeAnalysis): boolean {
  if (analysis.diagnosticNodes.has(node)) {
    return true
  }
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) {
      for (const child of value) {
        if (Array.isArray(child)) {
          for (const tupleChild of child) {
            if (
              isNode(tupleChild) &&
              containsDiagnostic(tupleChild, analysis)
            ) {
              return true
            }
          }
        } else if (isNode(child) && containsDiagnostic(child, analysis)) {
          return true
        }
      }
    } else if (isNode(value) && containsDiagnostic(value, analysis)) {
      return true
    }
  }
  return false
}

function isNode(value: unknown): value is ASTNode {
  return (
    typeof value === 'object' &&
    value !== null &&
    'type' in value &&
    typeof (value as { type?: unknown }).type === 'string'
  )
}
