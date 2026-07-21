import type { Expression, Identifier, Program, Statement } from './types'
import { tokenType } from './types'
import { analyzeScopes } from './scope'

// `let x=<literal>;` spends seven characters beyond the literal itself once
// the name mangles to a single character.
const BINDING_OVERHEAD = 7

interface Substitution {
  replacements: Map<Identifier, Expression>
  replaced: boolean
}

// Replace references to literal-initialized bindings with the literal.
//
// The compiler resolves a name to the binding whose `let` most recently
// precedes it in source order, and each such slot is written exactly once —
// redeclaring a name allocates a fresh slot instead of mutating the old one.
// A reference therefore always observes its own binding's initializer, except
// when the `let` sits inside an `if` arm and may never have run; those
// bindings stay put (`Binding.conditional`).
//
// Bindings whose references disappear here become dead and are collected by
// `eliminateDeadLets`. Returns whether any reference was replaced so the
// caller can re-fold and iterate to a fixpoint.
export function propagateConstants(program: Program): boolean {
  const analysis = analyzeScopes(program)
  if (!analysis.safe) {
    return false
  }
  const substitution: Substitution = {
    replacements: new Map(),
    replaced: false,
  }
  for (const [statement, binding] of analysis.letBindings) {
    if (binding.conditional || binding.references.length === 0) {
      continue
    }
    const width = literalWidth(statement.expr)
    if (width === null) {
      continue
    }
    // Inlining trades the binding (`let x=<literal>;` plus one-character
    // reads) for a literal copy at every use site; skip when the copies cost
    // more than the binding they free.
    if (binding.references.length * (width - 1) > BINDING_OVERHEAD + width) {
      continue
    }
    for (const reference of binding.references) {
      substitution.replacements.set(reference, statement.expr)
    }
  }
  if (substitution.replacements.size === 0) {
    return false
  }
  substituteStatements(program.body, substitution)
  return substitution.replaced
}

function literalWidth(expression: Expression): number | null {
  switch (expression.type) {
    case 'Integer':
      return expression.raw.length
    case 'Boolean':
      return expression.raw ? 4 : 5
    case 'String':
      return expression.raw.length + 2
    case 'UnaryExpression':
      // Folding renders a negative integer as MINUS over a positive literal.
      return tokenType(expression.op) === 'MINUS' &&
        expression.operand.type === 'Integer'
        ? expression.operand.raw.length + 1
        : null
    default:
      return null
  }
}

// Every use site gets its own copy: later passes rewrite nodes in place, and
// one node aliased across the tree would receive every rewrite at once.
function cloneLiteral(expression: Expression): Expression {
  switch (expression.type) {
    case 'Integer':
      return { type: 'Integer', raw: expression.raw }
    case 'Boolean':
      return { type: 'Boolean', raw: expression.raw }
    case 'String':
      return { type: 'String', raw: expression.raw }
    case 'UnaryExpression':
      return {
        type: 'UnaryExpression',
        op: { kind: { type: 'MINUS' } },
        operand: cloneLiteral(expression.operand),
      }
    default:
      throw new Error(`not a propagated literal: ${expression.type}`)
  }
}

function substituteStatements(
  statements: Statement[],
  substitution: Substitution
): void {
  for (let index = 0; index < statements.length; index += 1) {
    const statement = statements[index]
    switch (statement.type) {
      case 'Let':
        statement.expr = substituteExpression(statement.expr, substitution)
        break
      case 'ReturnStatement':
        statement.argument = substituteExpression(
          statement.argument,
          substitution
        )
        break
      case 'ClassDeclaration':
        for (const method of statement.methods) {
          substituteStatements(method.body.body, substitution)
        }
        break
      case 'SetPropertyStatement':
        statement.object = substituteExpression(statement.object, substitution)
        statement.value = substituteExpression(statement.value, substitution)
        break
      default:
        statements[index] = substituteExpression(statement, substitution)
    }
  }
}

function substituteExpression(
  expression: Expression,
  substitution: Substitution
): Expression {
  switch (expression.type) {
    case 'IDENTIFIER': {
      const template = substitution.replacements.get(expression)
      if (!template) {
        return expression
      }
      substitution.replaced = true
      return cloneLiteral(template)
    }
    case 'Integer':
    case 'Boolean':
    case 'String':
    case 'ThisExpression':
      return expression
    case 'Array':
      expression.elements = expression.elements.map((element) =>
        substituteExpression(element, substitution)
      )
      return expression
    case 'Hash':
      expression.elements = expression.elements.map(([key, value]) => [
        substituteExpression(key, substitution),
        substituteExpression(value, substitution),
      ])
      return expression
    case 'UnaryExpression':
      expression.operand = substituteExpression(
        expression.operand,
        substitution
      )
      return expression
    case 'BinaryExpression':
      expression.left = substituteExpression(expression.left, substitution)
      expression.right = substituteExpression(expression.right, substitution)
      return expression
    case 'IF':
      expression.condition = substituteExpression(
        expression.condition,
        substitution
      )
      substituteStatements(expression.consequent.body, substitution)
      if (expression.alternate) {
        substituteStatements(expression.alternate.body, substitution)
      }
      return expression
    case 'FunctionDeclaration':
      substituteStatements(expression.body.body, substitution)
      return expression
    case 'FunctionCall':
      expression.callee = substituteExpression(expression.callee, substitution)
      expression.arguments = expression.arguments.map((argument) =>
        substituteExpression(argument, substitution)
      )
      return expression
    case 'Index':
      expression.object = substituteExpression(expression.object, substitution)
      expression.index = substituteExpression(expression.index, substitution)
      return expression
    case 'PropertyExpression':
      expression.object = substituteExpression(expression.object, substitution)
      return expression
    case 'NewExpression':
      // The callee slot must stay an identifier node; an untouched reference
      // here keeps its binding alive instead.
      expression.arguments = expression.arguments.map((argument) =>
        substituteExpression(argument, substitution)
      )
      return expression
  }
}
