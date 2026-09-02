// The program walk: two-pass class collection, statement checking and the
// recursive half of expression inference (docs/type-system-design.md section 7).

import type {
  ArrayLiteral,
  ASTNode,
  BinaryExpression,
  BlockStatement,
  ClassDeclaration,
  Expression,
  FunctionCall,
  FunctionDeclaration,
  HashLiteral,
  Identifier,
  IfExpression,
  IndexExpression,
  LetStatement,
  MethodDefinition,
  NewExpression,
  Program,
  PropertyExpression,
  ReturnStatement,
  SetPropertyStatement,
  Span,
  Statement,
  TypeAnnotation,
  UnaryExpression,
} from '@gengjiawen/monkey-wasm'

import {
  BUILTIN_SIGNATURES,
  instantiateBuiltin,
  type InstantiatedBuiltin,
} from './builtins'
import {
  DIAGNOSTIC_CODES,
  sortDiagnostics,
  type TypeDiagnostic,
  type TypeSeverity,
} from './diagnostics'
import { Env } from './env'
import {
  EQUALITY_OPERATORS,
  inferBinary,
  inferEquality,
  inferIndex,
  inferPrefix,
} from './infer'
import {
  ANY,
  BOOL,
  INT,
  NULL,
  STRING,
  arrayOf,
  assignable,
  classOf,
  display,
  fnOf,
  hashOf,
  instanceOf,
  isHashable,
  joinAll,
  members,
  optional,
  stripNull,
  type ClassId,
  type ClassType,
  type FnType,
  type Type,
} from './types'

export interface CheckOptions {
  // v1 has no options; `strictNull` and friends land here later.
}

const PRIMITIVE_TYPE_NAMES: Record<string, Type> = {
  int: INT,
  bool: BOOL,
  string: STRING,
  any: ANY,
  null: NULL,
}

const EMPTY_SPAN: Span = { start: 0, end: 0 }

function spanOf(node: { span?: Span } | undefined): Span {
  return node?.span ?? EMPTY_SPAN
}

interface MethodSignature {
  params: Type[]
  /** The annotation, or `undefined` — other methods then see `any`. */
  declaredReturn: Type | undefined
  /** `declaredReturn ?? any`: what every other method sees. */
  ret: Type
  node: MethodDefinition
}

interface ClassInfo {
  id: ClassId
  name: string
  /**
   * Instance methods only. The constructor is not a property of the instance
   * — `new A().constructor` fails at runtime on every backend — so it is kept
   * apart, where `new` alone can see it.
   */
  methods: Map<string, MethodSignature>
  constructorSignature: MethodSignature | undefined
  fields: Map<string, Type>
}

interface FunctionFrame {
  returnTypes: Type[]
  declaredReturn: Type | undefined
}

/** What a run of statements leaves behind: its value, and whether it fell through. */
interface Completion {
  reachable: boolean
  value: Type
  /** Span of the tail expression, for a return-type mismatch diagnostic. */
  span: Span
}

class Checker {
  readonly diagnostics: TypeDiagnostic[] = []

  private env = new Env()
  private readonly classIds = new Map<ClassDeclaration, ClassId>()
  private readonly classes = new Map<ClassId, ClassInfo>()
  private readonly frames: FunctionFrame[] = []
  private nextClassId = 0

  /** Pass 1 collects class shapes in silence; pass 2 reports. */
  private collecting = true
  private currentClass: ClassInfo | undefined
  /** Nesting depth inside statements a `return` already made unreachable. */
  private deadCode = 0
  /**
   * Set once an expression in the current statement cannot complete: an `if`
   * in expression position whose arms all `return`. Reset per statement by
   * `checkBlock`, which treats such a statement like a `return`.
   */
  private diverged = false

  run(program: Program): void {
    this.collecting = true
    this.env = new Env()
    this.checkBlock(program)

    this.collecting = false
    this.env = new Env()
    this.checkBlock(program)
  }

  private report(
    code: string,
    message: string,
    span: Span,
    severity: TypeSeverity = 'error'
  ): void {
    if (this.collecting) {
      return
    }
    this.diagnostics.push({ code, message, span, severity })
  }

  // --- Statements ----------------------------------------------------------

  private checkBlock(block: { body: Statement[]; span?: Span }): Completion {
    // A nested block's divergence is reported through its completion, never
    // through the flag, so the enclosing statement's view is kept intact.
    const outerDiverged = this.diverged
    let reachable = true
    let value: Type = NULL
    // An empty body is `null` with no statement to blame, so a fallthrough
    // diagnostic points at the braces themselves.
    let span: Span = spanOf(block)

    for (const statement of block.body) {
      if (!reachable) {
        // Statements after a `return` are still checked — a type error there is
        // still an error — but they contribute neither to the block's value nor
        // to the enclosing function's inferred return type.
        this.deadCode += 1
        this.checkStatement(statement)
        this.deadCode -= 1
        continue
      }
      this.diverged = false
      const result = this.checkStatement(statement)
      // `let x = if (c) { return 1; } else { return 2; };` never binds `x`:
      // the block ends here exactly as it would after a `return`.
      reachable = result.reachable && !this.diverged
      value = result.value
      span = result.span
    }

    this.diverged = outerDiverged
    return { reachable, value, span }
  }

  private checkStatement(statement: Statement): Completion {
    switch (statement.type) {
      case 'Let':
        this.checkLet(statement as LetStatement)
        return { reachable: true, value: NULL, span: spanOf(statement) }
      case 'ReturnStatement':
        this.checkReturn(statement as ReturnStatement)
        return { reachable: false, value: NULL, span: spanOf(statement) }
      case 'ClassDeclaration':
        this.checkClass(statement as ClassDeclaration)
        return { reachable: true, value: NULL, span: spanOf(statement) }
      case 'SetPropertyStatement':
        this.checkSetProperty(statement as SetPropertyStatement)
        return { reachable: true, value: NULL, span: spanOf(statement) }
      case 'DebuggerStatement':
        return { reachable: true, value: NULL, span: spanOf(statement) }
      case 'IF': {
        // An `if` whose arms both return makes the rest of the block dead.
        const branch = this.inferIf(statement as IfExpression)
        return {
          reachable: branch.reachable,
          value: branch.type,
          span: spanOf(statement),
        }
      }
      default:
        return {
          reachable: true,
          value: this.inferExpression(statement as Expression),
          span: spanOf(statement),
        }
    }
  }

  private checkLet(statement: LetStatement): void {
    const annotation = statement.type_annotation
      ? this.resolveAnnotation(statement.type_annotation)
      : undefined

    // A function literal is visible under its own name inside its body, which
    // is what makes `let f = fn(n) { f(n - 1) }` resolve.
    const initializer =
      statement.expr.type === 'FunctionDeclaration'
        ? this.inferFunction(
            statement.expr as FunctionDeclaration,
            statement.identifier.name
          )
        : this.inferExpression(statement.expr)

    if (annotation && !assignable(initializer, annotation)) {
      this.reportMismatch(initializer, annotation, spanOf(statement.expr))
    }

    this.env.define(
      statement.identifier.name,
      annotation ?? initializer,
      this.isReceiverAlias(statement.expr)
    )
  }

  private checkReturn(statement: ReturnStatement): void {
    const type = this.inferExpression(statement.argument)
    const frame = this.frames[this.frames.length - 1]
    if (!frame) {
      return
    }
    if (this.deadCode === 0) {
      frame.returnTypes.push(type)
    }
    if (frame.declaredReturn && !assignable(type, frame.declaredReturn)) {
      this.reportMismatch(
        type,
        frame.declaredReturn,
        spanOf(statement.argument)
      )
    }
  }

  /**
   * The declared-return check for the value a body falls off the end with.
   * A body that only *may* fall through — a guard clause like
   * `if (c) { return 1; }` — completes with a bare `null` alongside real
   * `return`s; under section 7.7's join the null folds into `T?` and the
   * optimistic policy accepts it, so it is exempt here. A bare `null` with no
   * returns anywhere is the function's entire result and still reports.
   */
  private checkFallthrough(frame: FunctionFrame, completion: Completion): void {
    if (!frame.declaredReturn || !completion.reachable) {
      return
    }
    if (completion.value.kind === 'null' && frame.returnTypes.length > 0) {
      return
    }
    if (!assignable(completion.value, frame.declaredReturn)) {
      this.reportMismatch(
        completion.value,
        frame.declaredReturn,
        completion.span
      )
    }
  }

  private checkSetProperty(statement: SetPropertyStatement): void {
    const receiver = this.inferExpression(statement.object)
    const value = this.inferExpression(statement.value)
    const name = statement.property.name
    const span = spanOf(statement.property)

    if (this.collecting) {
      this.collectField(statement, name, value)
      return
    }

    for (const member of members(stripNull(receiver))) {
      if (member.kind === 'any') {
        continue
      }
      if (member.kind !== 'instance') {
        this.report(
          DIAGNOSTIC_CODES.unknownProperty,
          `property '${name}' does not exist on '${display(member)}'`,
          span
        )
        continue
      }
      const info = this.classes.get(member.id)
      if (!info) {
        continue
      }
      // Methods are checked first: pass 1 keeps method names out of the field
      // map precisely so this diagnostic stays reachable.
      if (info.methods.has(name)) {
        this.report(
          DIAGNOSTIC_CODES.assignToMethod,
          `assigning to method '${name}' shadows it only on this instance of '${info.name}'; annotate the receiver as 'any' if intended`,
          span
        )
        continue
      }
      const field = info.fields.get(name)
      if (!field) {
        this.report(
          DIAGNOSTIC_CODES.unknownProperty,
          `property '${name}' does not exist on '${info.name}'`,
          span
        )
        continue
      }
      if (!assignable(value, field)) {
        this.reportMismatch(value, field, spanOf(statement.value))
      }
    }
  }

  /** Records `this.x = expr` (or `alias.x = expr`) into the enclosing class. */
  private collectField(
    statement: SetPropertyStatement,
    name: string,
    value: Type
  ): void {
    const info = this.currentClass
    if (!info || !this.isReceiverAlias(statement.object)) {
      return
    }
    // A name that is already a method stays out of the field map so that
    // `assign-to-method` can fire on it in pass 2.
    if (info.methods.has(name)) {
      return
    }
    const previous = info.fields.get(name)
    info.fields.set(name, previous ? joinAll([previous, value]) : value)
  }

  /** `this`, or an identifier bound through a chain of `let x = this`. */
  private isReceiverAlias(expression: Expression | ASTNode): boolean {
    if (expression.type === 'ThisExpression') {
      return true
    }
    if (expression.type === 'IDENTIFIER') {
      return (
        this.env.lookup((expression as Identifier).name)?.thisAlias ?? false
      )
    }
    return false
  }

  // --- Classes -------------------------------------------------------------

  private checkClass(declaration: ClassDeclaration): void {
    const info = this.classInfo(declaration)
    // The class name is in scope for its own methods, so bind it first.
    this.env.define(declaration.name.name, classOf(info.id, info.name))

    if (PRIMITIVE_TYPE_NAMES[info.name]) {
      this.report(
        DIAGNOSTIC_CODES.reservedTypeName,
        `class '${info.name}' shadows a builtin type name; annotations cannot refer to it`,
        spanOf(declaration.name),
        'warning'
      )
    }

    // Signatures come from annotations only, and are known for every method
    // before any body is walked — `this.make()` must not depend on the order
    // methods happen to appear in.
    const signatures = declaration.methods.map((method) => {
      const signature = this.methodSignature(method)
      if (method.kind === 'Constructor') {
        info.constructorSignature = signature
      } else {
        info.methods.set(method.name.name, signature)
      }
      return signature
    })

    const previousClass = this.currentClass
    this.currentClass = info
    declaration.methods.forEach((method, index) => {
      this.checkMethodBody(method, signatures[index]!)
    })
    this.currentClass = previousClass
  }

  private classInfo(declaration: ClassDeclaration): ClassInfo {
    // Identity is per declaration, never per name: `class A {}` twice produces
    // two ids, so an alias saved before the second one keeps the first type.
    let id = this.classIds.get(declaration)
    if (id === undefined) {
      id = this.nextClassId
      this.nextClassId += 1
      this.classIds.set(declaration, id)
    }
    let info = this.classes.get(id)
    if (!info) {
      info = {
        id,
        name: declaration.name.name,
        methods: new Map(),
        constructorSignature: undefined,
        fields: new Map(),
      }
      this.classes.set(id, info)
    }
    return info
  }

  private methodSignature(method: MethodDefinition): MethodSignature {
    const params = method.params.map((param) =>
      param.type_annotation
        ? this.resolveAnnotation(param.type_annotation)
        : ANY
    )
    const declaredReturn = method.return_type
      ? this.resolveAnnotation(method.return_type)
      : undefined
    return {
      params,
      declaredReturn,
      ret: declaredReturn ?? ANY,
      node: method,
    }
  }

  private checkMethodBody(
    method: MethodDefinition,
    signature: MethodSignature
  ): void {
    this.env.push()
    method.params.forEach((param, index) => {
      this.env.define(param.identifier.name, signature.params[index] ?? ANY)
    })
    const frame: FunctionFrame = {
      returnTypes: [],
      declaredReturn: signature.declaredReturn,
    }
    this.frames.push(frame)
    const completion = this.inFreshBody(() => this.checkBlock(method.body))
    this.frames.pop()
    this.env.pop()

    this.checkFallthrough(frame, completion)
  }

  private constructorParams(info: ClassInfo): Type[] {
    return info.constructorSignature?.params ?? []
  }

  /**
   * A function body starts reachable no matter where the literal appears, so a
   * closure written after a `return` still infers its own return type.
   */
  private inFreshBody(check: () => Completion): Completion {
    const outer = this.deadCode
    this.deadCode = 0
    const completion = check()
    this.deadCode = outer
    return completion
  }

  // --- Annotations ---------------------------------------------------------

  private resolveAnnotation(annotation: TypeAnnotation): Type {
    switch (annotation.type) {
      case 'NamedType': {
        // Builtin names always win; a class may not take one over.
        const primitive = PRIMITIVE_TYPE_NAMES[annotation.name]
        if (primitive) {
          return primitive
        }
        const binding = this.env.lookup(annotation.name)
        if (binding?.type.kind === 'class') {
          return instanceOf(binding.type.id, binding.type.name)
        }
        this.report(
          DIAGNOSTIC_CODES.unknownTypeName,
          `unknown type '${annotation.name}'`,
          spanOf(annotation)
        )
        return ANY
      }
      case 'ArrayType':
        return arrayOf(this.resolveAnnotation(annotation.element))
      case 'HashType': {
        const key = this.resolveAnnotation(annotation.key)
        if (!isHashable(key)) {
          this.report(
            DIAGNOSTIC_CODES.invalidHashKey,
            `type '${display(key)}' cannot be used as a hash key`,
            spanOf(annotation.key)
          )
        }
        return hashOf(key, this.resolveAnnotation(annotation.value))
      }
      case 'FunctionType':
        return fnOf(
          annotation.params.map((param) => this.resolveAnnotation(param)),
          this.resolveAnnotation(annotation.return_type)
        )
      case 'OptionalType':
        return optional(this.resolveAnnotation(annotation.inner))
    }
  }

  // --- Expressions ---------------------------------------------------------

  private inferExpression(expression: Expression): Type {
    switch (expression.type) {
      case 'Integer':
        return INT
      case 'Boolean':
        return BOOL
      case 'String':
        return STRING
      case 'IDENTIFIER':
        // Validation guarantees every identifier resolves, so a miss here is a
        // builtin used as a value.
        return this.env.lookup((expression as Identifier).name)?.type ?? ANY
      case 'ThisExpression':
        return this.currentClass
          ? instanceOf(this.currentClass.id, this.currentClass.name)
          : ANY
      case 'Array':
        return this.inferArray(expression as ArrayLiteral)
      case 'Hash':
        return this.inferHash(expression as HashLiteral)
      case 'UnaryExpression':
        return this.inferUnary(expression as UnaryExpression)
      case 'BinaryExpression':
        return this.inferBinaryExpression(expression as BinaryExpression)
      case 'IF':
        return this.inferIf(expression as IfExpression).type
      case 'FunctionDeclaration':
        return this.inferFunction(expression as FunctionDeclaration)
      case 'FunctionCall':
        return this.inferCall(expression as FunctionCall)
      case 'Index':
        return this.inferIndexExpression(expression as IndexExpression)
      case 'PropertyExpression':
        return this.inferProperty(expression as PropertyExpression)
      case 'NewExpression':
        return this.inferNew(expression as NewExpression)
      default:
        return ANY
    }
  }

  private inferArray(literal: ArrayLiteral): Type {
    const elements = literal.elements.map((element) =>
      this.inferExpression(element)
    )
    return arrayOf(elements.length === 0 ? ANY : joinAll(elements))
  }

  private inferHash(literal: HashLiteral): Type {
    const keys: Type[] = []
    const values: Type[] = []
    for (const [key, value] of literal.elements) {
      const keyType = this.inferExpression(key)
      if (!isHashable(keyType)) {
        this.report(
          DIAGNOSTIC_CODES.invalidHashKey,
          `type '${display(keyType)}' cannot be used as a hash key`,
          spanOf(key)
        )
      }
      keys.push(keyType)
      values.push(this.inferExpression(value))
    }
    return hashOf(
      keys.length === 0 ? ANY : joinAll(keys),
      values.length === 0 ? ANY : joinAll(values)
    )
  }

  private inferUnary(expression: UnaryExpression): Type {
    const operand = this.inferExpression(expression.operand)
    const operator = operatorText(expression.op)
    const result = inferPrefix(operator, operand)
    if (result === null) {
      this.report(
        DIAGNOSTIC_CODES.operatorType,
        `operator '${operator}' expects 'int', got '${display(operand)}'`,
        spanOf(expression)
      )
      return ANY
    }
    return result
  }

  private inferBinaryExpression(expression: BinaryExpression): Type {
    const left = this.inferExpression(expression.left)
    const right = this.inferExpression(expression.right)
    const operator = operatorText(expression.op)

    if (EQUALITY_OPERATORS.includes(operator)) {
      const verdict = inferEquality(left, right)
      if (!verdict.ok && verdict.reason === 'uncomparable') {
        this.report(
          DIAGNOSTIC_CODES.invalidComparison,
          `values of type '${display(
            verdict.type
          )}' cannot be compared; GcVM raises a runtime error`,
          spanOf(expression)
        )
      } else if (!verdict.ok) {
        this.report(
          DIAGNOSTIC_CODES.mixedEquality,
          `comparing '${display(left)}' with '${display(
            right
          )}' diverges across backends; GcVM raises a runtime error`,
          spanOf(expression)
        )
      }
      return BOOL
    }

    const result = inferBinary(operator, left, right)
    if (result === null) {
      this.report(
        DIAGNOSTIC_CODES.operatorType,
        operatorMessage(operator, left, right),
        spanOf(expression)
      )
      return ANY
    }
    return result
  }

  private inferIf(expression: IfExpression): {
    type: Type
    reachable: boolean
  } {
    this.inferExpression(expression.condition)

    const consequent = this.inBranch(expression.consequent)
    const alternate = expression.alternate
      ? this.inBranch(expression.alternate)
      : undefined

    const values: Type[] = []
    if (consequent.reachable) {
      values.push(consequent.value)
    }
    if (alternate) {
      if (alternate.reachable) {
        values.push(alternate.value)
      }
    } else {
      // No `else` arm: taking the false path yields `null`.
      values.push(NULL)
    }

    const reachable = consequent.reachable || (alternate?.reachable ?? true)
    if (!reachable) {
      // Sticky for the rest of the statement: once one operand cannot
      // complete, neither can the expression around it.
      this.diverged = true
    }
    return { type: values.length === 0 ? NULL : joinAll(values), reachable }
  }

  /**
   * Runs one arm from the entering environment, then keeps whatever it bound:
   * a later reference can observe a binding left behind by either path, the
   * same rule the linter's scope analysis uses.
   */
  private inBranch(block: BlockStatement): Completion {
    this.env.push()
    const completion = this.checkBlock(block)
    const bindings = this.env.popFrame()
    for (const [name, binding] of bindings) {
      const existing = this.env.lookup(name)
      this.env.define(
        name,
        existing ? joinAll([existing.type, binding.type]) : binding.type,
        binding.thisAlias
      )
    }
    return completion
  }

  private inferFunction(node: FunctionDeclaration, selfName?: string): Type {
    const params = node.params.map((param) =>
      param.type_annotation
        ? this.resolveAnnotation(param.type_annotation)
        : ANY
    )
    const declaredReturn = node.return_type
      ? this.resolveAnnotation(node.return_type)
      : undefined

    this.env.push()
    if (selfName) {
      // Without an annotation a self-call is `any`: v1 does no fixpoint
      // iteration, so an unannotated recursive function infers `any`.
      this.env.define(selfName, fnOf(params, declaredReturn ?? ANY))
    }
    node.params.forEach((param, index) => {
      this.env.define(param.identifier.name, params[index] ?? ANY)
    })

    const frame: FunctionFrame = { returnTypes: [], declaredReturn }
    this.frames.push(frame)
    const completion = this.inFreshBody(() => this.checkBlock(node.body))
    this.frames.pop()
    this.env.pop()

    this.checkFallthrough(frame, completion)

    const inferred = joinAll([
      ...frame.returnTypes,
      ...(completion.reachable ? [completion.value] : []),
    ])
    return fnOf(params, declaredReturn ?? inferred)
  }

  private inferCall(call: FunctionCall): Type {
    const args = call.arguments.map((argument) =>
      this.inferExpression(argument)
    )

    const builtin = this.builtinFor(call.callee)
    if (builtin) {
      return this.checkBuiltinCall(call, builtin, args)
    }

    const callee = stripNull(this.inferExpression(call.callee))
    const overloads = members(callee)
    if (overloads.some((member) => member.kind === 'any')) {
      return ANY
    }
    if (overloads.some((member) => member.kind !== 'fn')) {
      this.report(
        DIAGNOSTIC_CODES.notCallable,
        `type '${display(callee)}' is not callable`,
        spanOf(call.callee)
      )
      return ANY
    }

    // A union of function types is callable only where every member agrees:
    // same arity, and every argument acceptable to all of them.
    const signatures = overloads.filter(
      (member): member is FnType => member.kind === 'fn'
    )
    const arities = new Set(
      signatures.map((signature) => signature.params.length)
    )
    if (arities.size > 1) {
      const counts = [...arities].sort((a, b) => a - b)
      this.report(
        DIAGNOSTIC_CODES.arityMismatch,
        `members of '${display(callee)}' disagree on arity (${counts.join(
          ' vs '
        )}); no call satisfies every member`,
        spanOf(call)
      )
      return joinAll(signatures.map((signature) => signature.ret))
    }
    const arity = signatures[0]!.params.length
    if (args.length !== arity) {
      this.report(
        DIAGNOSTIC_CODES.arityMismatch,
        `expected ${arity} argument${arity === 1 ? '' : 's'}, got ${
          args.length
        }`,
        spanOf(call)
      )
      return joinAll(signatures.map((signature) => signature.ret))
    }

    args.forEach((argument, index) => {
      for (const signature of signatures) {
        const expected = signature.params[index]!
        if (!assignable(argument, expected)) {
          this.reportMismatch(argument, expected, spanOf(call.arguments[index]))
          return
        }
      }
    })

    return joinAll(signatures.map((signature) => signature.ret))
  }

  /** A builtin only when the name is not shadowed by a user binding. */
  private builtinFor(
    callee: Expression
  ): InstantiatedBuiltinRequest | undefined {
    if (callee.type !== 'IDENTIFIER') {
      return undefined
    }
    const name = (callee as Identifier).name
    if (this.env.lookup(name)) {
      return undefined
    }
    const signature = BUILTIN_SIGNATURES[name]
    return signature ? { name, signature } : undefined
  }

  private checkBuiltinCall(
    call: FunctionCall,
    request: InstantiatedBuiltinRequest,
    args: Type[]
  ): Type {
    const instance: InstantiatedBuiltin = instantiateBuiltin(
      request.signature,
      args
    )
    if (!instance.variadic && args.length !== instance.params.length) {
      this.report(
        DIAGNOSTIC_CODES.arityMismatch,
        `${request.name} expects ${instance.params.length} argument${
          instance.params.length === 1 ? '' : 's'
        }, got ${args.length}`,
        spanOf(call)
      )
      return instance.ret
    }

    args.forEach((argument, index) => {
      const expected = instance.variadic
        ? instance.params[0]!
        : instance.params[index]!
      if (!assignable(argument, expected)) {
        this.reportMismatch(argument, expected, spanOf(call.arguments[index]))
      }
    })
    return instance.ret
  }

  private inferIndexExpression(expression: IndexExpression): Type {
    const target = this.inferExpression(expression.object)
    const subscript = this.inferExpression(expression.index)
    const verdict = inferIndex(target, subscript, assignable, isHashable)
    if (verdict.ok) {
      return verdict.type
    }
    if (verdict.reason === 'target') {
      this.report(
        DIAGNOSTIC_CODES.invalidIndex,
        `type '${display(target)}' is not indexable`,
        spanOf(expression.object)
      )
    } else {
      this.report(
        DIAGNOSTIC_CODES.invalidIndex,
        `type '${display(subscript)}' cannot index '${display(target)}'`,
        spanOf(expression.index)
      )
    }
    return ANY
  }

  private inferProperty(expression: PropertyExpression): Type {
    const receiver = this.inferExpression(expression.object)
    const name = expression.property.name

    // While collecting, a read off `this` is deliberately `any` unless it names
    // a method: fields must not depend on each other, and the result must not
    // depend on the order the methods are walked.
    if (this.collecting && this.isReceiverAlias(expression.object)) {
      const signature = this.currentClass?.methods.get(name)
      return signature ? fnOf(signature.params, signature.ret) : ANY
    }

    const result = this.resolveProperty(
      receiver,
      name,
      spanOf(expression.property)
    )
    return result
  }

  private resolveProperty(receiver: Type, name: string, span: Span): Type {
    const results: Type[] = []
    for (const member of members(stripNull(receiver))) {
      if (member.kind === 'any') {
        results.push(ANY)
        continue
      }
      if (member.kind !== 'instance') {
        this.report(
          DIAGNOSTIC_CODES.unknownProperty,
          `property '${name}' does not exist on '${display(member)}'`,
          span
        )
        results.push(ANY)
        continue
      }
      const info = this.classes.get(member.id)
      // Fields shadow methods, matching `set_property` writing straight into
      // the instance.
      const field = info?.fields.get(name)
      if (field) {
        results.push(field)
        continue
      }
      const method = info?.methods.get(name)
      if (method) {
        results.push(fnOf(method.params, method.ret))
        continue
      }
      this.report(
        DIAGNOSTIC_CODES.unknownProperty,
        `property '${name}' does not exist on '${display(member)}'`,
        span
      )
      results.push(ANY)
    }
    return results.length === 0 ? ANY : joinAll(results)
  }

  private inferNew(expression: NewExpression): Type {
    const args = expression.arguments.map((argument) =>
      this.inferExpression(argument)
    )
    const callee = stripNull(
      this.env.lookup(expression.callee.name)?.type ?? ANY
    )
    const candidates = members(callee)
    if (candidates.some((member) => member.kind === 'any')) {
      return ANY
    }
    if (candidates.some((member) => member.kind !== 'class')) {
      this.report(
        DIAGNOSTIC_CODES.notConstructable,
        `cannot construct '${display(callee)}'`,
        spanOf(expression.callee)
      )
      return ANY
    }

    // A union of classes is constructable under the same elimination rule as
    // calls: every member must agree on constructor arity and accept every
    // argument, and the result joins the instances.
    const infos = candidates
      .filter((member): member is ClassType => member.kind === 'class')
      .map((member) => this.classes.get(member.id))
    if (infos.some((info) => info === undefined)) {
      return ANY
    }
    const classes = infos as ClassInfo[]
    const result = joinAll(
      classes.map((info) => instanceOf(info.id, info.name))
    )

    const arities = new Set(
      classes.map((info) => this.constructorParams(info).length)
    )
    if (arities.size > 1) {
      const counts = [...arities].sort((a, b) => a - b)
      this.report(
        DIAGNOSTIC_CODES.arityMismatch,
        `constructors of '${display(callee)}' disagree on arity (${counts.join(
          ' vs '
        )}); no call satisfies every member`,
        spanOf(expression)
      )
      return result
    }
    const arity = this.constructorParams(classes[0]!).length
    if (args.length !== arity) {
      this.report(
        DIAGNOSTIC_CODES.arityMismatch,
        `${display(callee)} constructor expects ${arity} argument${
          arity === 1 ? '' : 's'
        }, got ${args.length}`,
        spanOf(expression)
      )
      return result
    }
    args.forEach((argument, index) => {
      for (const info of classes) {
        const expected = this.constructorParams(info)[index]!
        if (!assignable(argument, expected)) {
          this.reportMismatch(
            argument,
            expected,
            spanOf(expression.arguments[index])
          )
          return
        }
      }
    })
    return result
  }

  private reportMismatch(from: Type, to: Type, span: Span): void {
    const fromText = display(from)
    const toText = display(to)
    // Two nominal types can share a name (a class shadowing another); without
    // the hint the message reads "'A' is not assignable to 'A'".
    const hint =
      fromText === toText ? ' (same name, different declaration)' : ''
    this.report(
      DIAGNOSTIC_CODES.typeMismatch,
      `type '${fromText}' is not assignable to type '${toText}'${hint}`,
      span
    )
  }
}

interface InstantiatedBuiltinRequest {
  name: string
  signature: (typeof BUILTIN_SIGNATURES)[string]
}

function operatorText(token: { kind: { type: string } }): string {
  return TOKEN_OPERATORS[token.kind.type] ?? token.kind.type
}

const TOKEN_OPERATORS: Record<string, string> = {
  PLUS: '+',
  MINUS: '-',
  ASTERISK: '*',
  SLASH: '/',
  LT: '<',
  GT: '>',
  EQ: '==',
  NotEq: '!=',
  BANG: '!',
}

function operatorMessage(operator: string, left: Type, right: Type): string {
  const operands = `'${display(left)} ${operator} ${display(right)}'`
  if (operator === '+') {
    return `operator '+' expects 'int + int' or 'string + string', got ${operands}`
  }
  return `operator '${operator}' expects 'int ${operator} int', got ${operands}`
}

/**
 * Checks a parsed and validated program. This is a public low-level API, but
 * behavior on a tree that has not been through parse and validation is
 * undefined; most callers should use `check` instead.
 */
export function checkProgram(
  program: Program,
  _options: CheckOptions = {}
): TypeDiagnostic[] {
  const checker = new Checker()
  checker.run(program)
  return sortDiagnostics(checker.diagnostics)
}
