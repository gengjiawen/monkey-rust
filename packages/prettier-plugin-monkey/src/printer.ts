import { doc, util, type AstPath, type Doc, type Options } from 'prettier'
import type {
  Program,
  BlockStatement,
  Literal,
  LetStatement,
  ReturnStatement,
  BinaryExpression,
  UnaryExpression,
  IfExpression,
  FunctionDeclaration,
  FunctionCall,
  IndexExpression,
  Identifier,
  IntegerLiteral,
  BooleanLiteral,
  StringLiteral,
  ArrayLiteral,
  HashLiteral,
  MonkeyComment,
  ASTNode,
  ClassDeclaration,
  MethodDefinition,
  SetPropertyStatement,
  ThisExpression,
  PropertyExpression,
  NewExpression,
} from './types'

const { group, indent, line, softline, hardline, join, ifBreak } = doc.builders

export function print(
  path: AstPath,
  options: Options,
  print: (path: AstPath) => Doc
): Doc {
  const node = path.getValue()

  if (!node) {
    return ''
  }

  switch (node.type) {
    case 'Program':
      return printProgram(node as Program, path, print, options)
    case 'Let':
      return printLetStatement(node as LetStatement, path, print, options)
    case 'ReturnStatement':
      return printReturnStatement(node as ReturnStatement, path, print, options)
    case 'BlockStatement':
      return printBlockStatement(node as BlockStatement, path, print, options)
    case 'ClassDeclaration':
      return printClassDeclaration(node as ClassDeclaration, path, print)
    case 'MethodDefinition':
      return printMethodDefinition(node as MethodDefinition, path, print)
    case 'SetPropertyStatement':
      return printSetPropertyStatement(
        node as SetPropertyStatement,
        path,
        print
      )
    case 'IDENTIFIER':
      return printIdentifier(node as Identifier)
    case 'UnaryExpression':
      return printUnaryExpression(node as UnaryExpression, path, print, options)
    case 'BinaryExpression':
      return printBinaryExpression(
        node as BinaryExpression,
        path,
        print,
        options
      )
    case 'IF':
      return printIfExpression(node as IfExpression, path, print, options)
    case 'FunctionDeclaration':
      return printFunctionDeclaration(
        node as FunctionDeclaration,
        path,
        print,
        options
      )
    case 'FunctionCall':
      return printFunctionCall(node as FunctionCall, path, print, options)
    case 'Index':
      return printIndexExpression(node as IndexExpression, path, print, options)
    case 'ThisExpression':
      return printThisExpression(node as ThisExpression)
    case 'PropertyExpression':
      return printPropertyExpression(node as PropertyExpression, path, print)
    case 'NewExpression':
      return printNewExpression(node as NewExpression, path, print)
    case 'Integer':
    case 'Boolean':
    case 'String':
    case 'Array':
    case 'Hash':
      return printLiteral(node as Literal, path, print, options)
    default:
      throw new Error(`Unknown node type: ${String(node.type)}`)
  }
}

export function canAttachComment(node: unknown): boolean {
  return !!node && typeof node === 'object' && 'type' in node
}

export function isBlockComment(comment: MonkeyComment): boolean {
  return comment.type === 'CommentBlock'
}

export function printComment(commentPath: AstPath): Doc {
  const comment = commentPath.getValue() as MonkeyComment | null
  if (!comment) {
    return ''
  }

  return printCommentNode(comment)
}

export function handleOwnLineComment(
  comment: MonkeyComment & {
    enclosingNode?: ASTNode
  }
): boolean {
  const enclosingNode = comment.enclosingNode
  if (
    enclosingNode?.type === 'ClassDeclaration' &&
    (enclosingNode as ClassDeclaration).methods.length === 0
  ) {
    util.addDanglingComment(enclosingNode, comment, undefined)
    return true
  }

  return false
}

function printCommentNode(comment: MonkeyComment): Doc {
  if (comment.type === 'CommentBlock') {
    return `/*${comment.value}*/`
  }

  return `//${comment.value}`
}

function printProgram(
  node: Program,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  if (node.body.length === 0) {
    return ''
  }

  const parts: Doc[] = []

  path.each((statementPath: AstPath) => {
    parts.push(print(statementPath))
  }, 'body')

  return [join(hardline, parts), hardline]
}

function printLetStatement(
  node: LetStatement,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  const identifierName = (node.identifier.kind as any).value?.name || ''

  return group(['let ', identifierName, ' = ', path.call(print, 'expr'), ';'])
}

function printReturnStatement(
  node: ReturnStatement,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  return group(['return ', path.call(print, 'argument'), ';'])
}

function printBlockStatement(
  node: BlockStatement,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  if (node.body.length === 0) {
    const comments = printInnerComments(node)
    if (comments.length === 0) {
      return '{}'
    }

    return group([
      '{',
      indent([hardline, join(hardline, comments)]),
      hardline,
      '}',
    ])
  }

  const parts: Doc[] = []
  path.each((statementPath: AstPath) => {
    parts.push(print(statementPath))
  }, 'body')

  return group(['{', indent([hardline, join(hardline, parts)]), hardline, '}'])
}

function printClassDeclaration(
  node: ClassDeclaration,
  path: AstPath,
  print: (path: AstPath) => Doc
): Doc {
  const methods: Doc[] = []
  path.each((methodPath: AstPath) => {
    methods.push(print(methodPath))
  }, 'methods')

  const danglingComments = printInnerComments(node)
  const members = [...methods, ...danglingComments]

  if (members.length === 0) {
    return ['class ', path.call(print, 'name'), ' {}']
  }

  return group([
    'class ',
    path.call(print, 'name'),
    ' {',
    indent([hardline, join([hardline, hardline], members)]),
    hardline,
    '}',
  ])
}

function printMethodDefinition(
  node: MethodDefinition,
  path: AstPath,
  print: (path: AstPath) => Doc
): Doc {
  return group([
    path.call(print, 'name'),
    printDelimitedList(path, print, 'params'),
    ' ',
    path.call(print, 'body'),
  ])
}

function printSetPropertyStatement(
  node: SetPropertyStatement,
  path: AstPath,
  print: (path: AstPath) => Doc
): Doc {
  return group([
    printPostfixChild(node.object, path.call(print, 'object')),
    '.',
    path.call(print, 'property'),
    ' = ',
    path.call(print, 'value'),
    ';',
  ])
}

function printIdentifier(node: Identifier): Doc {
  return node.name
}

function printUnaryExpression(
  node: UnaryExpression,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  const operator = getTokenValue(node.op)
  return group(['(', operator, path.call(print, 'operand'), ')'])
}

function printBinaryExpression(
  node: BinaryExpression,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  const operator = getTokenValue(node.op)

  // Only add parentheses for nested binary expressions
  const needsParens = (n: any): boolean => {
    return n && (n.type === 'BinaryExpression' || n.type === 'UnaryExpression')
  }

  const leftNeedsParens = needsParens(node.left)
  const rightNeedsParens = needsParens(node.right)

  return group([
    leftNeedsParens ? '(' : '',
    path.call(print, 'left'),
    leftNeedsParens ? ')' : '',
    ' ',
    operator,
    ' ',
    rightNeedsParens ? '(' : '',
    path.call(print, 'right'),
    rightNeedsParens ? ')' : '',
  ])
}

function printIfExpression(
  node: IfExpression,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  const parts: Doc[] = [
    'if (',
    path.call(print, 'condition'),
    ') ',
    path.call(print, 'consequent'),
  ]

  if (node.alternate) {
    parts.push(' else ', path.call(print, 'alternate'))
  }

  return group(parts)
}

function printFunctionDeclaration(
  node: FunctionDeclaration,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  return group([
    'fn',
    printDelimitedList(path, print, 'params'),
    ' ',
    path.call(print, 'body'),
  ])
}

function printFunctionCall(
  node: FunctionCall,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  return group([
    printPostfixChild(node.callee, path.call(print, 'callee')),
    printDelimitedList(path, print, 'arguments'),
  ])
}

function printIndexExpression(
  node: IndexExpression,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  return group([
    printPostfixChild(node.object, path.call(print, 'object')),
    '[',
    path.call(print, 'index'),
    ']',
  ])
}

function printThisExpression(node: ThisExpression): Doc {
  return 'this'
}

function printPropertyExpression(
  node: PropertyExpression,
  path: AstPath,
  print: (path: AstPath) => Doc
): Doc {
  return group([
    printPostfixChild(node.object, path.call(print, 'object')),
    '.',
    path.call(print, 'property'),
  ])
}

function printNewExpression(
  node: NewExpression,
  path: AstPath,
  print: (path: AstPath) => Doc
): Doc {
  return group([
    'new ',
    path.call(print, 'callee'),
    printDelimitedList(path, print, 'arguments'),
  ])
}

function printDelimitedList(
  path: AstPath,
  print: (path: AstPath) => Doc,
  property: 'params' | 'arguments'
): Doc {
  const items: Doc[] = []
  path.each((itemPath: AstPath) => {
    items.push(print(itemPath))
  }, property)

  if (items.length === 0) {
    return '()'
  }

  return group([
    '(',
    indent([softline, join([',', line], items)]),
    softline,
    ')',
  ])
}

function printPostfixChild(node: ASTNode, childDoc: Doc): Doc {
  if (
    node.type === 'BinaryExpression' ||
    node.type === 'IF' ||
    node.type === 'FunctionDeclaration'
  ) {
    return ['(', childDoc, ')']
  }

  return childDoc
}

function printInnerComments(node: ASTNode): Doc[] {
  if (!node.comments || !node.span) {
    return []
  }

  const comments: Doc[] = []
  for (const comment of node.comments) {
    if (
      comment.start >= node.span.start &&
      comment.end <= node.span.end &&
      !comment.printed
    ) {
      comment.printed = true
      comments.push(printCommentNode(comment))
    }
  }
  return comments
}

function printLiteral(
  node: Literal,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  switch (node.type) {
    case 'Integer':
      return String((node as IntegerLiteral).raw)
    case 'Boolean':
      return String((node as BooleanLiteral).raw)
    case 'String': {
      const str = (node as StringLiteral).raw
      return `"${str}"`
    }
    case 'Array':
      return printArrayLiteral(node as ArrayLiteral, path, print, options)
    case 'Hash':
      return printHashLiteral(node as HashLiteral, path, print, options)
    default:
      return ''
  }
}

function printArrayLiteral(
  node: ArrayLiteral,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  if (node.elements.length === 0) {
    return '[]'
  }

  const elements: Doc[] = []

  path.each((elementPath: AstPath) => {
    elements.push(print(elementPath))
  }, 'elements')

  const shouldBreak = node.elements.length > 3

  return group(
    [
      '[',
      indent([
        shouldBreak ? hardline : softline,
        join([',', line], elements),
        options.trailingComma === 'none' ? '' : ifBreak(','),
      ]),
      shouldBreak ? hardline : softline,
      ']',
    ],
    { shouldBreak }
  )
}

function printHashLiteral(
  node: HashLiteral,
  path: AstPath,
  print: (path: AstPath) => Doc,
  options: Options
): Doc {
  if (node.elements.length === 0) {
    return '{}'
  }

  const pairs: Doc[] = []

  node.elements.forEach((_, index) => {
    const key = path.call(print, 'elements', index, 0)
    const value = path.call(print, 'elements', index, 1)

    pairs.push(group([key, ': ', value]))
  })

  const shouldBreak = node.elements.length > 2

  return group(
    [
      '{',
      options.bracketSpacing ? ifBreak('', ' ') : '',
      indent([
        shouldBreak ? hardline : softline,
        join([',', line], pairs),
        options.trailingComma === 'none' ? '' : ifBreak(','),
      ]),
      shouldBreak ? hardline : softline,
      options.bracketSpacing ? ifBreak('', ' ') : '',
      '}',
    ],
    { shouldBreak }
  )
}

function getTokenValue(token: any): string {
  const kind = token.kind

  switch (kind.type) {
    case 'PLUS':
      return '+'
    case 'MINUS':
      return '-'
    case 'ASTERISK':
      return '*'
    case 'SLASH':
      return '/'
    case 'BANG':
      return '!'
    case 'LT':
      return '<'
    case 'GT':
      return '>'
    case 'EQ':
      return '=='
    case 'NotEq':
      return '!='
    case 'ASSIGN':
      return '='
    default:
      return ''
  }
}
