import type {
  ArrayLiteral,
  ASTNode,
  BinaryExpression,
  BlockStatement,
  ClassDeclaration,
  FunctionCall,
  FunctionDeclaration,
  HashLiteral,
  IfExpression,
  IndexExpression,
  LetStatement,
  MethodDefinition,
  NewExpression,
  Program,
  PropertyExpression,
  ReturnStatement,
  SetPropertyStatement,
  UnaryExpression,
} from './types'

/**
 * Pre-order visitor. `enter` is called for every AST node before its children,
 * with the parent node (or `null` at the root). Tokens, spans, and primitive
 * fields are not nodes and are never visited; rules read those directly off the
 * node they are attached to.
 */
export type WalkVisitor = (node: ASTNode, parent: ASTNode | null) => void

export function walk(program: Program, enter: WalkVisitor): void {
  visit(program, null, enter)
}

function visit(node: ASTNode, parent: ASTNode | null, enter: WalkVisitor): void {
  enter(node, parent)
  for (const child of childrenOf(node)) {
    visit(child, node, enter)
  }
}

/**
 * Child AST nodes in source order. The `Let` identifier is a lexer `Token`
 * (not an AST node) and is intentionally excluded — `identifierName()` reads it
 * directly. Operator tokens are likewise skipped.
 */
export function childrenOf(node: ASTNode): ASTNode[] {
  switch (node.type) {
    case 'Program':
      return (node as Program).body
    case 'BlockStatement':
      return (node as BlockStatement).body
    case 'Let':
      return [(node as LetStatement).expr]
    case 'ReturnStatement':
      return [(node as ReturnStatement).argument]
    case 'ClassDeclaration': {
      const declaration = node as ClassDeclaration
      return [declaration.name, ...declaration.methods]
    }
    case 'MethodDefinition': {
      const method = node as MethodDefinition
      return [method.name, ...method.params, method.body]
    }
    case 'SetPropertyStatement': {
      const set = node as SetPropertyStatement
      return [set.object, set.property, set.value]
    }
    case 'Array':
      return (node as ArrayLiteral).elements
    case 'Hash':
      return (node as HashLiteral).elements.flatMap((pair) => pair)
    case 'UnaryExpression':
      return [(node as UnaryExpression).operand]
    case 'BinaryExpression': {
      const binary = node as BinaryExpression
      return [binary.left, binary.right]
    }
    case 'IF': {
      const branch = node as IfExpression
      return branch.alternate
        ? [branch.condition, branch.consequent, branch.alternate]
        : [branch.condition, branch.consequent]
    }
    case 'FunctionDeclaration': {
      const fn = node as FunctionDeclaration
      return [...fn.params, fn.body]
    }
    case 'FunctionCall': {
      const call = node as FunctionCall
      return [call.callee, ...call.arguments]
    }
    case 'Index': {
      const index = node as IndexExpression
      return [index.object, index.index]
    }
    case 'PropertyExpression': {
      const property = node as PropertyExpression
      return [property.object, property.property]
    }
    case 'NewExpression': {
      const expression = node as NewExpression
      return [expression.callee, ...expression.arguments]
    }
    default:
      // Leaves: IDENTIFIER, Integer, Boolean, String, ThisExpression.
      return []
  }
}
