import type {
  BinaryExpression,
  BlockStatement,
  ClassDeclaration,
  Expression,
  FunctionDeclaration,
  LetStatement,
  Literal,
  MethodDefinition,
  Program,
  SetPropertyStatement,
  Statement,
} from './types'
import { identifierName, tokenType } from './types'

enum Precedence {
  Lowest,
  Equals,
  LessGreater,
  Sum,
  Product,
  Prefix,
  Postfix,
  Primary,
}

const operators: Record<string, string> = {
  PLUS: '+',
  MINUS: '-',
  ASTERISK: '*',
  SLASH: '/',
  BANG: '!',
  LT: '<',
  GT: '>',
  EQ: '==',
  NotEq: '!=',
}

const infixPrecedence: Record<string, Precedence> = {
  EQ: Precedence.Equals,
  NotEq: Precedence.Equals,
  LT: Precedence.LessGreater,
  GT: Precedence.LessGreater,
  PLUS: Precedence.Sum,
  MINUS: Precedence.Sum,
  ASTERISK: Precedence.Product,
  SLASH: Precedence.Product,
}

interface PrintedExpression {
  code: string
  precedence: Precedence
}

export function printProgram(program: Program): string {
  return program.body.map(printStatement).join('')
}

function printStatement(statement: Statement): string {
  switch (statement.type) {
    case 'Let':
      return printLet(statement)
    case 'ReturnStatement':
      return `return ${printExpression(statement.argument)};`
    case 'ClassDeclaration':
      return printClass(statement)
    case 'SetPropertyStatement':
      return printSetProperty(statement)
    default:
      return `${printExpression(statement)};`
  }
}

function printLet(statement: LetStatement): string {
  return `let ${identifierName(statement)}=${printExpression(statement.expr)};`
}

function printClass(statement: ClassDeclaration): string {
  return `class ${statement.name.name}{${statement.methods
    .map(printMethod)
    .join('')}}`
}

function printMethod(method: MethodDefinition): string {
  return `${method.name.name}(${method.params
    .map((param) => param.name)
    .join(',')})${printBlock(method.body)}`
}

function printSetProperty(statement: SetPropertyStatement): string {
  const object = printChild(statement.object, Precedence.Postfix)
  return `${object}.${statement.property.name}=${printExpression(
    statement.value
  )};`
}

function printBlock(block: BlockStatement): string {
  return `{${block.body.map(printStatement).join('')}}`
}

export function printExpression(expression: Expression): string {
  return renderExpression(expression).code
}

function printChild(
  expression: Expression,
  minimum: Precedence,
  parenthesizeEqual = false
): string {
  const printed = renderExpression(expression)
  return printed.precedence < minimum ||
    (parenthesizeEqual && printed.precedence === minimum)
    ? `(${printed.code})`
    : printed.code
}

function renderExpression(expression: Expression): PrintedExpression {
  switch (expression.type) {
    case 'IDENTIFIER':
      return primary(expression.name)
    case 'Integer':
      return primary(expression.raw)
    case 'Boolean':
      return primary(String(expression.raw))
    case 'String':
      return primary(`"${expression.raw}"`)
    case 'Array':
      return primary(`[${expression.elements.map(printExpression).join(',')}]`)
    case 'Hash':
      return primary(
        `{${expression.elements
          .map(
            ([key, value]) =>
              `${printExpression(key)}:${printExpression(value)}`
          )
          .join(',')}}`
      )
    case 'UnaryExpression': {
      const operator = operatorFor(expression.op.kind.type)
      return {
        code: `${operator}${printChild(expression.operand, Precedence.Prefix)}`,
        precedence: Precedence.Prefix,
      }
    }
    case 'BinaryExpression':
      return renderBinary(expression)
    case 'IF':
      return {
        code: `if(${printExpression(expression.condition)})${printBlock(
          expression.consequent
        )}${
          expression.alternate ? `else${printBlock(expression.alternate)}` : ''
        }`,
        precedence: Precedence.Lowest,
      }
    case 'FunctionDeclaration':
      return renderFunction(expression)
    case 'FunctionCall':
      return {
        code: `${printChild(
          expression.callee,
          Precedence.Postfix
        )}(${expression.arguments.map(printExpression).join(',')})`,
        precedence: Precedence.Postfix,
      }
    case 'Index':
      return {
        code: `${printChild(
          expression.object,
          Precedence.Postfix
        )}[${printExpression(expression.index)}]`,
        precedence: Precedence.Postfix,
      }
    case 'ThisExpression':
      return primary('this')
    case 'PropertyExpression':
      return {
        code: `${printChild(expression.object, Precedence.Postfix)}.${
          expression.property.name
        }`,
        precedence: Precedence.Postfix,
      }
    case 'NewExpression':
      return {
        code: `new ${expression.callee.name}(${expression.arguments
          .map(printExpression)
          .join(',')})`,
        precedence: Precedence.Postfix,
      }
  }
}

function renderBinary(expression: BinaryExpression): PrintedExpression {
  const kind = tokenType(expression.op)
  const precedence = infixPrecedence[kind]
  if (precedence === undefined) {
    throw new Error(`Unknown binary operator: ${kind}`)
  }
  return {
    code: `${printChild(expression.left, precedence)}${operatorFor(
      kind
    )}${printChild(expression.right, precedence, true)}`,
    precedence,
  }
}

function renderFunction(expression: FunctionDeclaration): PrintedExpression {
  return {
    code: `fn(${expression.params
      .map((param) => param.name)
      .join(',')})${printBlock(expression.body)}`,
    precedence: Precedence.Lowest,
  }
}

function primary(code: string): PrintedExpression {
  return { code, precedence: Precedence.Primary }
}

function operatorFor(kind: string): string {
  const operator = operators[kind]
  if (operator === undefined) {
    throw new Error(`Unknown operator: ${kind}`)
  }
  return operator
}
