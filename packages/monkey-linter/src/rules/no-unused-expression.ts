import type { Expression, Statement } from '../types'
import type { Rule } from '../core'

/**
 * An expression used as a statement whose value is discarded and whose
 * evaluation has no observable effect — a `let`/`return` was probably intended.
 *
 * "Discarded" is decided by position. Every block yields its final statement's
 * value, so a tail expression is *observed* when the enclosing construct's own
 * value is observed:
 *
 *   - the program's final statement is observed (it is the run's result);
 *   - a function/method body's tail is observed (it is the return value), except
 *     a constructor body, whose value `new` throws away in favour of `this`;
 *   - an `if` branch's tail is observed exactly when the `if` itself is;
 *   - every non-tail statement is discarded.
 *
 * Only *pure* expressions are reported. A pure expression contains no call,
 * `new`, `if`, or index/property read, so flagging one never hides a side
 * effect or a possible runtime error — and, conveniently, a pure expression has
 * no nested blocks, so the walk never double-reports.
 */
export const noUnusedExpression: Rule = {
  name: 'no-unused-expression',
  severity: 'warn',
  check({ program, report }) {
    const checkStatements = (
      statements: Statement[],
      tailObserved: boolean
    ): void => {
      statements.forEach((statement, index) => {
        const observed = index === statements.length - 1 && tailObserved
        checkStatement(statement, observed)
      })
    }

    const checkStatement = (statement: Statement, observed: boolean): void => {
      switch (statement.type) {
        case 'Let':
          descend(statement.expr)
          return
        case 'ReturnStatement':
          descend(statement.argument)
          return
        case 'ClassDeclaration':
          for (const method of statement.methods) {
            checkStatements(method.body.body, method.kind !== 'Constructor')
          }
          return
        case 'SetPropertyStatement':
          descend(statement.object)
          descend(statement.value)
          return
        default:
          if (!observed && isPure(statement)) {
            report(
              'expression value is unused; did you mean to `let` or `return` it?',
              statement.span
            )
          }
          descend(statement, observed)
      }
    }

    // Recurse into the statement lists nested inside an expression. Never
    // reports on its own — reporting happens only at statement position above.
    const descend = (expression: Expression, observed = false): void => {
      switch (expression.type) {
        case 'FunctionDeclaration':
          checkStatements(expression.body.body, true)
          return
        case 'IF':
          descend(expression.condition)
          checkStatements(expression.consequent.body, observed)
          if (expression.alternate) {
            checkStatements(expression.alternate.body, observed)
          }
          return
        case 'UnaryExpression':
          descend(expression.operand)
          return
        case 'BinaryExpression':
          descend(expression.left)
          descend(expression.right)
          return
        case 'Array':
          for (const element of expression.elements) {
            descend(element)
          }
          return
        case 'Hash':
          for (const [key, value] of expression.elements) {
            descend(key)
            descend(value)
          }
          return
        case 'FunctionCall':
          descend(expression.callee)
          for (const argument of expression.arguments) {
            descend(argument)
          }
          return
        case 'NewExpression':
          for (const argument of expression.arguments) {
            descend(argument)
          }
          return
        case 'Index':
          descend(expression.object)
          descend(expression.index)
          return
        case 'PropertyExpression':
          descend(expression.object)
          return
        default:
          // IDENTIFIER, Integer, Boolean, String, ThisExpression: no nested
          // statement lists.
          return
      }
    }

    checkStatements(program.body, true)
  },
}

/**
 * An expression with no side effect and no nested block. A call, `new`, `if`, or
 * function literal may run arbitrary code, so those are never pure. Index and
 * property reads have no user-defined getters, but both backends raise a
 * runtime error when the object is not indexable (or has no such property), so
 * they are conservatively excluded too (docs/linter-plan.md; revisit in v1).
 * Everything else is pure only if all of its sub-expressions are.
 */
function isPure(expression: Expression): boolean {
  switch (expression.type) {
    case 'IDENTIFIER':
    case 'Integer':
    case 'Boolean':
    case 'String':
    case 'ThisExpression':
      return true
    case 'UnaryExpression':
      return isPure(expression.operand)
    case 'BinaryExpression':
      return isPure(expression.left) && isPure(expression.right)
    case 'Array':
      return expression.elements.every(isPure)
    case 'Hash':
      return expression.elements.every(
        ([key, value]) => isPure(key) && isPure(value)
      )
    default:
      // FunctionCall, NewExpression, IF, FunctionDeclaration, Index,
      // PropertyExpression.
      return false
  }
}
