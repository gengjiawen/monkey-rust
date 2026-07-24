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
 * Only expressions that are both side-effect-free and guaranteed not to raise
 * a runtime error are reported. The deliberately small safe subset has no
 * nested blocks, so the walk never double-reports.
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
          descend(statement.expr, true)
          return
        case 'ReturnStatement':
          descend(statement.argument, true)
          return
        case 'ClassDeclaration':
          for (const method of statement.methods) {
            checkStatements(method.body.body, method.kind !== 'Constructor')
          }
          return
        case 'SetPropertyStatement':
          descend(statement.object, true)
          descend(statement.value, true)
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
    const descend = (expression: Expression, observed: boolean): void => {
      switch (expression.type) {
        case 'FunctionDeclaration':
          checkStatements(expression.body.body, true)
          return
        case 'IF':
          descend(expression.condition, true)
          checkStatements(expression.consequent.body, observed)
          if (expression.alternate) {
            checkStatements(expression.alternate.body, observed)
          }
          return
        case 'UnaryExpression':
          descend(expression.operand, true)
          return
        case 'BinaryExpression':
          descend(expression.left, true)
          descend(expression.right, true)
          return
        case 'Array':
          for (const element of expression.elements) {
            descend(element, true)
          }
          return
        case 'Hash':
          for (const [key, value] of expression.elements) {
            descend(key, true)
            descend(value, true)
          }
          return
        case 'FunctionCall':
          descend(expression.callee, true)
          for (const argument of expression.arguments) {
            descend(argument, true)
          }
          return
        case 'NewExpression':
          for (const argument of expression.arguments) {
            descend(argument, true)
          }
          return
        case 'Index':
          descend(expression.object, true)
          descend(expression.index, true)
          return
        case 'PropertyExpression':
          descend(expression.object, true)
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
 * An expression with no side effect, no nested block, and no possible runtime
 * error. Calls, `new`, conditionals, function literals, index/property reads,
 * operators, and hashes are conservatively excluded: even apparently pure
 * forms can fail because of arity, types, division by zero, or unhashable keys.
 * Array literals are safe only when every element is safe.
 */
function isPure(expression: Expression): boolean {
  switch (expression.type) {
    case 'IDENTIFIER':
    case 'Integer':
    case 'Boolean':
    case 'String':
    case 'ThisExpression':
      return true
    case 'Array':
      return expression.elements.every(isPure)
    default:
      // UnaryExpression, BinaryExpression, Hash, FunctionCall, NewExpression,
      // IF, FunctionDeclaration, Index, PropertyExpression.
      return false
  }
}
