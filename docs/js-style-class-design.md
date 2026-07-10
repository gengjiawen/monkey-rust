# Monkey JS 风格 Class 设计提案

> 状态：Implemented。基础 class、三个执行后端、WASM GC report、Playground、Prettier 和 VS Code grammar 已按本文落地。
>
> 核心结论：Monkey 的基础类使用 `class`、`constructor`、`this` 和 `new`。第一版采用 JS 风格的表面语法，但不实现原型链；运行时使用 class / instance / bound method 三种明确的对象。
>
> 关联设计：[monkey-gc 设计文档](./gc.md)。
>
> 本文保留完整设计取舍，同时作为实现记录。当前 GC 能力以本文第 11 至 13 节、`docs/gc.md`、`gc/gc-report.md` 和 `gc/README.md` 的同步说明为准。

## 目录

1. [背景与结论](#1-背景与结论)
2. [目标与非目标](#2-目标与非目标)
3. [用户语法](#3-用户语法)
4. [语言语义](#4-语言语义)
5. [Grammar、优先级与解析](#5-grammar优先级与解析)
6. [AST 与 Span](#6-ast-与-span)
7. [共享对象模型](#7-共享对象模型)
8. [Interpreter 设计](#8-interpreter-设计)
9. [Compiler 与 Bytecode 设计](#9-compiler-与-bytecode-设计)
10. [默认 VM 调用约定](#10-默认-vm-调用约定)
11. [GcVM 与引用所有权](#11-gcvm-与引用所有权)
12. [GC Playground 演示](#12-gc-playground-演示)
13. [WASM API](#13-wasm-api)
14. [Prettier 支持](#14-prettier-支持)
15. [错误语义](#15-错误语义)
16. [兼容性与已知前置问题](#16-兼容性与已知前置问题)
17. [测试与验收](#17-测试与验收)
18. [实施顺序](#18-实施顺序)
19. [文件改动索引](#19-文件改动索引)
20. [延后能力与备选方案](#20-延后能力与备选方案)

---

## 1. 背景与结论

实现前，Monkey 已有 lexer、parser、AST interpreter、bytecode compiler、默认 `Rc<Object>` VM 和支持环回收的 `GcVM`，但语言值基本不可变：array/hash 构造完成后不能写回，也没有带可变字段的对象。因此，Monkey 源码当时无法稳定构造下面这种对象图：

```text
a.next ──► b
  ▲        │
  └────────┘ b.next
```

基础 class 和可变实例字段补上了这条缺口，Playground 现在可以用纯 Monkey 源码解释 `gc_decref`、`gc_scan` 和 `gc_free_cycles` 为什么存在。

实现采用的最小用户体验是：

```monkey
class Node {
  constructor(value) {
    this.value = value;
  }

  connect(other) {
    this.next = other;
  }
}

let a = new Node("a");
let b = new Node("b");
a.connect(b);
b.connect(a);
```

这里使用 JS 的 `constructor`，没有 `init`。第一版不是完整 JavaScript 对象系统，而是：

- JS 风格 class declaration、method、`constructor`、`this` 和 `new`；
- 每个 instance 有动态、可写的字段表；
- class 有固定的方法表和单独保存的 constructor；
- 读取方法得到绑定 receiver 的 bound method；
- 默认 `Rc` 后端允许强引用环泄漏，`GcVM` 后端使用同样的强边并由 cycle collector 回收。

## 2. 目标与非目标

### 2.1 目标

- 使用接近 JS 的类语法，不增加 Monkey 特有的 `init`。
- 让 interpreter、默认 VM 和 `GcVM` 对同一段 class 源码具有一致语义。
- 支持 constructor、实例方法、动态字段、字段读写和 detached bound method。
- 让纯 Monkey 源码能够构造 self-cycle、双向环和 instance / bound-method 环。
- 保持现有 bytecode 的数值兼容：所有新 opcode 追加在 enum 末尾。
- 保留准确 AST span、WASM JSON AST 和 Prettier round-trip。
- 为 playground 提供可复现、可量化、不会暴露 collector 中间不变量的 GC 报告。

### 2.2 非目标

第一版明确不实现：

- `extends`、`super` 和继承；
- JavaScript prototype chain 或运行时 prototype mutation；
- static method、static field、instance field declaration；
- getter、setter、computed/private method name；
- class expression，例如 `let C = class {}`；
- block/function 内的 local class declaration；
- 通用变量重新赋值，例如 `x = 2`；
- assignment expression 和链式赋值，例如 `a.x = b.x = 1`；
- `obj["field"]` 形式的实例属性访问；第一版仅支持 dot property；
- `delete`、反射、枚举实例字段或修改 class 方法表；
- `new C` 的省略括号形式；
- 对普通 Monkey `fn` 使用 `new`；
- JS constructor 返回 object 时替换 receiver 的规则；
- JS 的动态 `this`、`call`、`apply` 和 `bind`。

local class 延后的主要原因不是 parser，而是当前 compiler 对 free variable 按值捕获。局部 class 的方法若引用 class 自身，会在 class 完成赋值前捕获未初始化 local slot。应在未来有明确的 recursive binding / cell 语义后再开放，不能让 interpreter 和 VM 在这一点上分叉。

## 3. 用户语法

### 3.1 Constructor 和动态字段

```monkey
class Point {
  constructor(x, y) {
    this.x = x;
    this.y = y;
  }

  sum() {
    this.x + this.y;
  }
}

let point = new Point(20, 22);
point.sum();
```

普通 method 保留 Monkey 函数当前的尾表达式返回规则，所以 `sum()` 返回 `42`。constructor 的 body 结果会被忽略，`new Point(...)` 永远返回新 instance。

### 3.2 缺省 constructor

```monkey
class Empty {
  value() {
    42;
  }
}

let empty = new Empty();
```

省略 constructor 等价于零参数 default constructor。`new Empty(1)` 是参数数量错误。

### 3.3 Property assignment 是 statement

```monkey
let point = new Point(1, 2);
point.x = 40;
point.sum();
```

第一版允许 `object.field = value;`，但它是 statement，不是 expression：

```monkey
let result = point.x = 40; // parse error
point.x = other.x = 40;    // parse error
x = 40;                    // parse error
```

这能为 instance 提供必要的可变边，同时不顺带引入完整 assignment 语义。

### 3.4 Bound method

```monkey
class Counter {
  constructor(value) {
    this.value = value;
  }

  current() {
    this.value;
  }
}

let counter = new Counter(42);
let current = counter.current;
current(); // 42
```

`counter.current` 产生 bound method。它强持有 `counter`，所以即使 receiver 没有其他引用，`current()` 仍然能访问原 instance。

### 3.5 `this` 的 lexical capture

```monkey
class Box {
  constructor(value) {
    this.value = value;
  }

  reader() {
    fn() { this.value };
  }
}

let read = new Box(42).reader();
read(); // 42
```

Monkey 只有一种 `fn`，没有 JS regular function / arrow function 的二分。为避免再引入一套动态 receiver 调用规则，method 内的 `this` 可以被嵌套 `fn` 当作 free variable lexical capture。这个行为更接近 JS arrow function，是有意的语言差异。

## 4. 语言语义

### 4.1 Class declaration

- class declaration 第一版只能出现在 `Program.body` 顶层。
- class declaration 不 hoist，遵循现有 compiler 的源码顺序解析：class name 在校验自身 methods 前进入当前 global scope，所以 method 可以引用自身；后续才声明的 global 仍不可引用。
- class name 进入现有 global symbol table；method 在执行时可以通过 global 读取 class 自身。
- class 是运行时值，可以赋给普通变量或作为参数传递：

  ```monkey
  let Type = Point;
  let point = new Type(1, 2);
  ```

- class value 不能被 `OpCall` 调用；`Point()` 报错并提示必须使用 `new`。
- class body 只接受 method definition，不接受 field declaration 或任意 statement。
- `constructor` 是 contextual name，不是全局保留关键字。class body 外仍然可以声明 `let constructor = fn() {};`。
- 最多一个 constructor；重复 constructor 或重复普通 method name 都是错误。
- class 完成构造后，语言层不能新增、覆盖或删除 method。

### 4.2 Constructor

- 必须通过 `new ClassName(...)` 触发。
- 先求值 class callee，再从左到右求值 arguments，然后验证 callee、分配 instance 并调用 constructor。
- `this` 指向刚分配的 instance。
- constructor 和 method 参数数量严格匹配，而不是沿用 JS 忽略多余参数的行为。默认 VM / GcVM 已对普通 closure 做 strict arity；interpreter 当前对参数不足会 panic、对多余参数会忽略，本阶段必须补成同一套显式 arity error。
- 省略 constructor 时只接受零参数。
- constructor 不进入普通 methods map，`instance.constructor` 不会自动存在。
- 第一版拒绝 constructor 自身的任何显式 `return ...;`。当前 Monkey 没有 bare `return;`，所以每个 `ReturnStatement` 都带返回值；嵌套 `fn` 内的 return 不受影响。
- constructor 正常落空时返回 receiver instance；属于 constructor 自身的显式 return 在进入 runtime 前已被拒绝。

### 4.3 `this`

- `this` 是保留关键字，不能作为变量、参数或 class name。
- `this` 只在 method / constructor 以及它们的 nested closure 中有效。
- compiler 把 `this` 放在 callable 的隐藏 local slot 0；显式参数从 slot 1 开始。
- 用户看到的 method arity 不包含隐藏的 `this`。
- 顶层或普通函数中没有可捕获的 method `this` 时，使用 `this` 是 compile/eval error。

### 4.4 Property read

`receiver.name` 按以下顺序解析：

1. receiver 必须是 instance；
2. 先查 instance fields；
3. field 不存在时查 class methods；
4. method 命中时返回新的 bound method；
5. 两者都不存在时报 runtime error。

字段优先于 method，因此允许有意 shadow method：

```monkey
class Example {
  value() { 1; }
}

let example = new Example();
example.value = 42;
example.value; // 42
```

function-valued field 只是普通 function value，不会因 `obj.callback()` 自动得到动态 `this`。只有 class method lookup 会生成 bound method。

### 4.5 Property write

- `receiver.name = value;` 只接受 instance receiver。
- 不存在的字段会创建；已存在的字段会覆盖。
- class method table 不会被改写；写入同名 field 仅产生 shadow。
- 求值顺序为 receiver、value、写入。两个子表达式都只求值一次。
- statement 完成后语义结果为 `null`。compiler 在 `OpSetProperty` 后生成 `OpNull; OpPop`，记录这个结果但不在 operand stack 留下值。

### 4.6 Identity 与显示

- class 与 instance 的 `==` / `!=` 使用 identity，不做字段结构比较。
- class alias 保留 identity；两个分别 `new` 的 instance 即使字段相同也不相等。
- 每次 method property read 都产生独立 bound method；bound method 使用 identity equality，不比较 receiver/method 结构。因此 alias 与自身相等，但两次 `object.method` read 不相等。
- class、instance、bound method 都不可作为 hash key。
- `Display` 必须是非递归、cycle-safe 的 opaque 形式，例如：

  ```text
  [class Node]
  [object Node]
  [bound method Node.connect]
  ```

字段图需要单独的 debug inspector，并使用 visited set，不能让普通 `Display` 展开所有字段。

## 5. Grammar、优先级与解析

### 5.1 新 token

```text
CLASS  -> class
THIS   -> this
NEW    -> new
DOT    -> .
```

`constructor` 继续 lex 为 `IDENTIFIER`。

lexer 同时应修正 identifier continuation：首字符仍为 ASCII letter / `_`，后续允许 digit，使 `Node2` 成为一个 identifier，而不是 `Node` + `2`。

### 5.2 Grammar

```ebnf
program                ::= statement* EOF ;

statement              ::= let_statement
                         | return_statement
                         | class_declaration
                         | set_property_statement
                         | expression_statement ;

class_declaration      ::= "class" IDENTIFIER "{" method_definition* "}" ;

method_definition      ::= IDENTIFIER "(" parameter_list? ")" block_statement ;

set_property_statement ::= property_expression "=" expression ";"? ;

new_expression         ::= "new" IDENTIFIER "(" argument_list? ")" ;

this_expression        ::= "this" ;

property_expression    ::= expression "." IDENTIFIER ;
```

MVP 的 `new` callee 暂限 identifier。这已经支持 class alias 和作为函数参数传入的 class：`new Type()`。`new registry.Type()`、`new (factory())()` 留到后续扩展，避免第一版复制 JS 复杂的 `new` precedence。

保留字第一版也不作为 dot property / method name，因此 `object.class` 或 `class C { new() {} }` 不接受；如未来需要，可单独引入 JS 风格 `IdentifierName`，不改变运行时模型。

### 5.3 Pratt precedence

`(`、`[` 和 `.` 都属于同一个最高 `POSTFIX` precedence，并由 Pratt loop 左结合：

```text
LOWEST
EQUALS
LESS_GREATER
SUM
PRODUCT
PREFIX
POSTFIX       // call, index, property
```

这样 AST 形状稳定为：

```text
new Node(1).connect(other)
Call(Property(New(Node, [1]), connect), [other])

a.b(c).d[0]
Index(Property(Call(Property(a, b), [c]), d), 0)
```

`new` 使用 prefix parser，但该 parser 内部明确消费 `IDENTIFIER (...)`；返回 `NewExpression` 后，再由外层 Pratt loop 消费 property/index/call。因此：

```monkey
new Node(1).next;       // (new Node(1)).next
new Node(1).connect();  // ((new Node(1)).connect)()
new Node;               // parse error: parentheses required
```

### 5.4 Call callee 必须放开

当前 call parser 只接受 identifier 或 function literal。class 实现必须把 callee 放开为任意 expression，把“是否可调用”留到 runtime：

```monkey
node.connect(other);
new Factory().make()();
array[0]();
```

这既是 bound method 的前置条件，也是 Pratt postfix chain 的正确语义。

### 5.5 Property set 不进入 Pratt assignment precedence

`=` 不加入 expression precedence。`parse_expression_statement` 先完整解析左侧 expression；如果下一个 token 是 `=`：

- 左侧必须是 `PropertyExpression`；
- 最外层 property 拆成 `object + property name`；
- 消费 `=` 后解析右侧 expression；
- 构造 `SetPropertyStatement`。

因此 `a.b.c = value` 合法，表示设置 `(a.b).c`；`a = value`、`a[0] = value` 和 expression 内 assignment 都得到定向错误。

### 5.6 Class body 是专用 parser

class body 不能复用普通 block statement parser。它只解析 `name(params) { body }`，并在 parse 阶段完成：

- contextual constructor 标记；
- constructor 数量检查；
- method name 去重；
- class member span；
- `}` / EOF 错误恢复。

顶层限制也应由 parser context 明确执行。普通 block 内遇到 `class` 应报 `class declarations are only allowed at top level`，不能默默当成错误 expression。

## 6. AST 与 Span

### 6.1 AST 节点

建议新增：

```rust
pub enum Statement {
    // existing variants...
    Class(ClassDeclaration),
    SetProperty(SetPropertyStatement),
}

pub enum Expression {
    // existing variants...
    This(ThisExpression),
    Property(PropertyExpression),
    New(NewExpression),
}

pub struct ClassDeclaration {
    pub name: IDENTIFIER,
    pub methods: Vec<MethodDefinition>,
    pub span: Span,
}

pub struct MethodDefinition {
    pub kind: MethodKind,
    pub name: IDENTIFIER,
    pub params: Vec<IDENTIFIER>,
    pub body: BlockStatement,
    pub span: Span,
}

pub enum MethodKind {
    Constructor,
    Method,
}

pub struct SetPropertyStatement {
    pub object: Box<Expression>,
    pub property: IDENTIFIER,
    pub value: Expression,
    pub span: Span,
}

pub struct ThisExpression {
    pub span: Span,
}

pub struct PropertyExpression {
    pub object: Box<Expression>,
    pub property: IDENTIFIER,
    pub span: Span,
}

pub struct NewExpression {
    pub callee: IDENTIFIER,
    pub arguments: Vec<Expression>,
    pub span: Span,
}
```

AST 的 `methods` 必须保持源码顺序。constructor 只通过 `MethodKind` 标记，不能在 AST 中抽到单独字段，否则 formatter 会改变成员顺序，成员间 comment 也难以附着。compiler/runtime lowering 时再把 constructor 与普通 methods 分开。

`ThisExpression` 使用带 span 的 struct，不能做 unit enum variant。当前 AST 通过 serde 输出给 WASM/Prettier，所有节点都需要稳定的 JSON `type`。

建议 JSON type 固定为：

```text
ClassDeclaration
MethodDefinition
SetPropertyStatement
ThisExpression
PropertyExpression
NewExpression
```

### 6.2 统一 span accessor

当前 call span 通过枚举 match 猜 callee 起点，index span 又从 `[` 起算。新增三个 postfix 节点前，应统一提供：

```rust
pub trait Spanned {
    fn span(&self) -> &Span;
}
```

或等价的 `Expression::span()` / `Statement::span()`，避免每增加一个 variant 就复制一组容易遗漏的 match。

括号需要额外处理：AST 当前不会保留 `GroupedExpression`，因此只看 inner expression 的 span 无法得知 `(a + b).value` 是从 `(` 开始。Pratt parser 应保留 parser-local wrapper（例如 `ParsedExpression { expression, cover_span }`；现有 tuple 也可以），其中 `cover_span` 包含被消费的外围括号。call / index / property 构造 span 时使用这个 cover span 的起点，但 inner AST 节点仍保留自己的精确 span。这样不必为了 source range 把 identifier 的 span 错误扩大到 `(identifier)`，也不必新增只服务于括号的公开 AST variant。

所有 span 使用 half-open byte range `[start, end)`：

| 节点         | span                                        |
| ------------ | ------------------------------------------- |
| class        | `class` 起点到最外层 `}` 末尾               |
| method       | method name 起点到 body `}` 末尾            |
| this         | `this` token 本身                           |
| property     | object 起点到 property name 末尾            |
| new          | `new` 起点到 argument list `)` 末尾         |
| set property | object 起点到 `;` 末尾；无分号时到 RHS 末尾 |

parser test 应对新节点执行 `&input[span.start..span.end]` 精确切片断言，并覆盖 `(a + b).value`、`(fn() { 1 })()` 和 `((a)).value`。

### 6.3 Parser 错误传播前置修复

class method body 会放大两个现有问题，实现前必须修复：

- `parse_block_statement` 不能丢弃内部 `parse_statement` error；应使用 `?` 传播或明确同步后累计错误。
- binary expression 解析 RHS 不能 `unwrap()`；坏源码必须返回 parse error，不能 panic。

否则 constructor 中的非法语句可能从 AST 静默消失，产生错误但看似可执行的 class。

### 6.4 共享静态校验

只在 compiler 中依赖 symbol resolution 会让 interpreter 接受 compiler 拒绝的源码。例如：

```monkey
class A {
  make() { new B(); }
}
class B {}
```

本设计维持 Monkey 现有的源码顺序规则，因此上例中的 `B` 是 forward global reference，两个后端都应拒绝。新增一个只依赖 AST 的 `validate_program(program, predefined_globals)` pass，并由 interpreter 与 compiler 在执行/生成 bytecode 前共同调用：

- 按源码顺序维护 global/function-local scopes；`let` name 在校验 RHS 前进入 scope，以保留现有 self-recursive function 行为；class name 在校验自身 methods 前进入 scope；
- 记录 `CallableKind::{Function, Method, Constructor}`，constructor 自身的 `return ...;` 非法，进入 nested `fn` 后恢复普通 return 规则；
- 单独记录 lexical receiver capability。method / constructor 把它设为可用，nested `fn` 继承它，因此 `CallableKind::Function` 不会错误拒绝对外层 `this` 的 capture；
- 顶层、普通 function 或 forward global 中的无效 `this` / identifier 在 interpreter 真正执行用户代码前就成为 `EvalError`，compiler 返回等价 `CompileError`；
- builtin names 由调用方通过 `predefined_globals` 传入，validator 不依赖 `object` crate。

建议实现位于新增的 `parser/validation.rs`，但 `parse()` 仍只负责 syntax；WASM 的纯 AST parse endpoint 不因语义校验改变行为。constructor return 的 interpreter validator 和 `this` 检查都归并到这一 pass，避免三套递归 walker。

## 7. 共享对象模型

interpreter 和默认 VM 继续使用 `object::Object`，新增三种 runtime value：

```rust
pub type ClassRef = Rc<RefCell<ClassObject>>;
pub type InstanceRef = Rc<RefCell<InstanceObject>>;

pub struct ClassObject {
    pub name: String,
    pub constructor: Option<Rc<Object>>,
    pub methods: HashMap<String, Rc<Object>>,
}

pub struct InstanceObject {
    pub class: ClassRef,
    pub fields: HashMap<String, Rc<Object>>,
}

pub struct BoundMethodObject {
    pub receiver: InstanceRef,
    pub method: Rc<Object>,
    pub name: String,
}

pub enum Object {
    // existing variants...
    Class(ClassRef),
    Instance(InstanceRef),
    BoundMethod(Rc<BoundMethodObject>),
}
```

`ClassObject` 在 bytecode 执行 `OpClass` / `OpMethod` 时逐步组装，因此 class ref 本身需要内部可变性。class declaration 完成后不再对语言层暴露这种可变性。

所有边都是强 `Rc`：

```text
Class ──► constructor / methods
Instance ──► Class
Instance ──► field values
BoundMethod ──► Instance + method
```

不能把 instance field 或 bound receiver 改成 `Weak`。例如 `let f = object.method; f();` 要求 `f` 单独保活 receiver；使用 `Weak` 会直接改变语言行为。

### 7.1 `Eq`、`Debug` 与 hash 审计

`Object` 当前依赖 derive 的结构相等。引入可变、可成环的 instance 后，不能继续让 derive 递归比较整个字段图：

- primitive 保持 value equality；
- class / instance 用 `Rc::ptr_eq`；
- bound method 使用 `Rc::ptr_eq` identity；同一 bound method 的 alias 相等，两次独立 property read 得到的 bound method 不相等；
- array/hash 保持现有行为，但其中的 instance 元素按 identity；
- class / instance / bound method 的 `is_hashable()` 返回 false；
- `Debug` / `Display` 避免递归展开 instance fields。

这项审计是防止 `a.next = b; b.next = a; a == b` 或测试失败信息触发无限递归的必要工作。

### 7.2 默认后端的环泄漏是预期对照

默认 interpreter / VM 使用 `Rc<RefCell<_>>`。instance fields 必须强持有值，因此不可达环在这两个后端会泄漏。这不是第一版需要用 `Weak` 绕过的问题，而是 `GcVM` 演示要对比的真实所有权差异。

## 8. Interpreter 设计

### 8.1 Class declaration 求值

求值 `ClassDeclaration` 时：

1. 验证 class 位于顶层；
2. 按源码顺序把每个 method 转成 `Object::Function`，capture 当前 declaration environment；
3. 将 `MethodKind::Constructor` 单独保存；
4. 将普通 method 写入 methods map；
5. 构造 `Object::Class` 并绑定到 class name；
6. statement 返回 `null`。

class symbol 应在 method closure 被调用前可见。因为顶层 environment 是共享 cell，method 对 global class name 的读取发生在执行期，不需要在 class 构造时复制未初始化值。

### 8.2 Property read/write

新增集中 helper，interpreter 和测试都通过它们执行语义：

```rust
fn get_property(receiver: &Rc<Object>, name: &str) -> Result<Rc<Object>, EvalError>;

fn set_property(
    receiver: &Rc<Object>,
    name: String,
    value: Rc<Object>,
) -> Result<(), EvalError>;
```

`get_property` 字段优先；method 命中时构造 `Object::BoundMethod`。`set_property` 使用 `RefCell` 修改 fields。

### 8.3 Method / constructor 调用

`apply_function` 增加两条分支：

- `BoundMethod`：以 method declaration environment 为 outer environment，先绑定 `this`，再绑定显式 params，然后执行 body；
- `Class`：普通 call 路径报错，提示使用 `new`。

`NewExpression` 单独执行：

1. 求值 callee 和 args；
2. 检查 callee 是 class；
3. 分配带空 fields 的 instance；
4. 有 constructor 时以 receiver 调用；无 constructor 时验证零参数；
5. 忽略 constructor body 的普通求值结果；
6. 返回 instance。

constructor return、unbound `this` 和源码顺序 name resolution 都由 6.4 的共享 validator 在求值前完成。interpreter 的 method call environment 仍需让 nested function capture 已绑定的 `this`；validator 只做静态许可判断，不替代 runtime binding。

## 9. Compiler 与 Bytecode 设计

### 9.1 新 opcode

为保持现有 opcode 数值，以下条目必须追加到 `Opcode` enum 末尾：

```text
OpClass
OpMethod
OpGetProperty
OpSetProperty
OpNew
```

建议编码：

| Opcode          | operands                                | 说明                        |
| --------------- | --------------------------------------- | --------------------------- |
| `OpClass`       | class name constant: `u16`              | 创建空 class                |
| `OpMethod`      | method name constant: `u16`, kind: `u8` | 把 closure 安装到栈顶 class |
| `OpGetProperty` | property name constant: `u16`           | 字段读取或创建 bound method |
| `OpSetProperty` | property name constant: `u16`           | 写 instance field           |
| `OpNew`         | explicit argument count: `u8`           | 实例化并进入 constructor    |

`MethodKind` byte 固定为：

```text
0 = ordinary method
1 = constructor
```

即使 constructor 的名字已经是 `constructor`，仍保留显式 kind，避免 VM 依赖字符串比较，并为未来 method kind 扩展留出稳定编码。

### 9.2 Stack effect

```text
OpClass name
  [] -> [class]

OpMethod name kind
  [class, closure] -> [class]

OpGetProperty name
  [receiver] -> [field | bound_method]

OpSetProperty name
  [receiver, value] -> []

OpNew argc
  [class, arg1, ..., argN] -> [instance]
```

`OpNew` 有 constructor 时会暂时 push frame，所以最后一条 stack effect 是 constructor 返回后的可观察结果，不表示 opcode 内同步执行完整 body。

### 9.3 Class declaration lowering

class name 先在 symbol table 定义，使 method body 可以 resolve global self reference。bytecode 大致为：

```text
OpClass "Node"

OpClosure <constructor-fn>
OpMethod "constructor" Constructor

OpClosure <connect-fn>
OpMethod "connect" Method

OpSetGlobal <Node>
OpNull
OpPop
```

整个构造期间 class 留在 operand stack，最终由 `OpSetGlobal` 消费。末尾的 `OpNull; OpPop` 让 class declaration 与 interpreter 一样产生 `null` statement result。method closure 对 global 的引用是运行时 load，不会在 class 尚未写入 global 时捕获空值。

### 9.4 Method compilation

每个 method 使用现有 function compilation scope，但按以下顺序定义 locals：

```text
local 0 = this        // synthetic
local 1 = first explicit parameter
local 2 = second explicit parameter
...
```

因此：

```rust
compiled_function.num_parameters = method.params.len() + 1;
compiled_function.num_locals = 1 + params + local_lets;
```

普通 method 沿用现有尾表达式转 `OpReturnValue` 的行为。嵌套 `fn` resolve `this` 时，把 local slot 0 当作普通 free symbol capture。

这里有一个必须先修复的现有边界：当前 `SymbolTable::resolve` 会用只读查找直接跨过中间 function 找到祖先 local，不能逐层建立 free symbol。method 内嵌两层以上 `fn` 时，最内层 `this` 可能从错误 frame 读取 local 0。实现应让 resolve 在每层 symbol table 递归并逐层 `define_free`，而不是只在最内层 capture；class 验收同时覆盖一层和两层 nested closure。

constructor 使用独立 compilation context：

- 属于 constructor 自身、带 argument 的 `ReturnStatement` compile error；
- 尾表达式不能替换为 constructor 返回值；
- 正常结束固定 emit `OpGetLocal 0; OpReturnValue`；
- 因而 constructor 复用普通 function return 路径，但结果只能是 receiver。

需要 compiler-side `CallableKind::{Function, Method, Constructor}` context stack，而不是单个 boolean，因为 constructor 内 nested `fn` 应恢复普通 function return 规则。MVP 不需要给 runtime `Frame` 增加 kind。

### 9.5 Expression / statement lowering

```text
this
  -> resolve synthetic symbol
  -> OpGetLocal 0 或 nested closure 中的 OpGetFree

receiver.name
  -> compile receiver
  -> OpGetProperty "name"

receiver.name = value
  -> compile receiver
  -> compile value
  -> OpSetProperty "name"
  -> OpNull
  -> OpPop

new ClassName(args)
  -> compile ClassName
  -> compile args left-to-right
  -> OpNew argc
```

debug info 使用完整节点 span：`OpClass` 对应 class span，`OpMethod` 对应 method span，property/new opcode 对应各自 expression/statement span。

## 10. 默认 VM 调用约定

### 10.1 复用普通 Frame

MVP 不增加 runtime `FrameKind`。method 和 constructor 都编译成带隐藏 receiver 参数的普通 closure；VM 在入帧前把 bound/new 调用改写成现有 `[closure, args...]` 布局。constructor 的 compiled body 固定返回 local 0，所以现有 `OpReturnValue` 清理逻辑自然得到 instance。

真正需要区分 callable kind 的位置在 compiler，用于 return 校验和尾部指令生成。这样默认 VM 与 GcVM 都无需维护第二套 return state machine。

### 10.2 Bound method 调用

当前 VM 约定 callee 在 `base_pointer - 1`，第一个参数在 `base_pointer`。调用 bound method 时插入隐藏 receiver：

```text
调用前： [bound_method, arg1, arg2]
改写后： [method_closure, receiver, arg1, arg2]
                         ^ base_pointer
```

VM 先从 bound method clone underlying closure 和 receiver，再右移 arguments、替换 callee slot，以 `explicit_argc + 1` 校验 compiled parameter count 并建立普通 frame。改写后的 closure / receiver stack slots 在整个调用期间分别保活 method 和 instance。

错误信息对用户仍显示显式参数：

```text
wrong number of arguments for Node.connect: want=1, got=2
```

而不是暴露内部 `want=2`。

### 10.3 Constructor 调用

`OpNew` 的布局为：

```text
执行前： [class, arg1, arg2]
改写后： [constructor_closure, instance, arg1, arg2]
                              ^ base_pointer / this slot
```

- instance 强持有 class，constructor closure 有独立 callee stack owner；
- frame 按现有约定持有或借用 constructor 的可执行 metadata；
- instance 从分配完成到 constructor 结束始终由 `this` stack slot 保活；
- constructor 尾部 `OpGetLocal 0; OpReturnValue` 把 instance 放到 return path；
- 现有 return 逻辑清理 constructor closure / this / args，再把同一 instance 压成 `new` 的结果。

省略 constructor 时不 push frame：验证 `argc == 0`，消费 class，直接留下 instance。

### 10.4 `OpCall` 分派

```text
Closure      -> existing function call
BoundMethod  -> insert receiver, call underlying method
Builtin      -> existing/native builtin path
Class        -> error: class must be constructed with new
other        -> existing non-callable error
```

## 11. GcVM 与引用所有权

### 11.1 Gc-native value

`gc::Value` 新增：

```rust
pub struct GcClass {
    pub name: String,
    pub constructor: Option<GcRef>,
    pub methods: HashMap<String, GcRef>,
}

pub struct GcInstance {
    pub class: GcRef,
    pub fields: HashMap<String, GcRef>,
}

pub struct GcBoundMethod {
    pub receiver: GcRef,
    pub method: GcRef,
    pub name: String,
}

pub enum Value {
    // existing variants...
    Class(GcClass),
    Instance(GcInstance),
    BoundMethod(GcBoundMethod),
}
```

它们继续使用 `GcObjectType::MonkeyObject`。统计 UI 需要的分类通过单独 `ValueKind` 提供，不应把语言 variant 全部编码成 collector object type。

### 11.2 Trace

所有 runtime 边都是强 `GcRef` 并参与 trace：

```text
Class.trace:
  constructor?
  every methods[name]

Instance.trace:
  class
  every fields[name]

BoundMethod.trace:
  receiver
  method
```

`with_owned_edges` 必须同步覆盖三种新 value。只补 `trace` 不补 allocation ownership，或反过来，都会造成 refcount 错账。

### 11.3 Mutation ownership protocol

class method 安装和 field overwrite 必须集中到 helper，执行固定顺序：

```text
1. dup(new_edge)
2. mutate/swap map entry
3. leave mutable borrow
4. free(old_edge), if any
5. consume/free opcode stack temporaries
```

不能在 `object_downcast_mut` 后直接写入一个裸 `GcRef`。也不能在持有 mutable downcast borrow 时再次调用可能触发 runtime mutation 的 `dup/free`。

Property get：

- field 命中返回 `heap.dup(field)`；
- method 命中分配 bound method，allocation 为 receiver/method 各持有一条边；
- receiver 的 operand-stack 引用随后被正常消费。

### 11.4 Frame 不拥有第二套引用

GcVM stack slot / globals / constants 继续是显式引用 owner，Frame 只借用 callable metadata。bound/new 改写 callee slot 前，必须先取得 owned closure / receiver 引用；改写后由 closure slot 和 local 0 分别保活它们。Frame 不额外 `dup` this/class，避免清理时双账。

### 11.5 Builtin 已改成 Gc-native

实现前的 GcVM builtin 路径会执行：

```text
GcRef -> object::Object -> BuiltinFunc -> object::Object -> GcRef
```

class 上线后，这条旧桥会产生三个不可接受的问题：

- `first` / `last` 返回深拷贝，破坏 instance identity；
- alias 关系丢失；
- cyclic instance graph 在 export 时无限递归。

实现已经在 `object::builtins` 引入稳定的 `BuiltinId`，包括 `Len / Puts / First / Last / Rest / Push`。builtin registry 同时暴露 name、ID 和默认后端的 `BuiltinFunc`；compiler 仍编码 registry index，默认 VM 取 function，GcVM 把同一项转成 ID。`Value::Builtin(BuiltinId)` 原生操作 `GcRef`：

- `first` / `last` 返回 child 的 `dup`；
- `rest` / `push` 用原 GcRef 建新 array，由 `alloc_value` 持边；
- `len` 直接读取 Gc value；
- `puts` 使用 cycle-safe opaque formatter。

import/export bridge 可以保留给 acyclic compatibility API，但必须明确返回 `Result` 或使用 graph-aware memo；builtin、VM 内部执行和 playground 都不能再依赖它。

### 11.6 Cycle-safe inspection

普通显示 class / instance / bound method 时不展开 fields。若 GC panel 需要显示对象图，提供独立 inspector：

```rust
inspect(reference, max_depth, visited: &mut HashSet<GcId>)
```

遇到已访问节点输出引用标签，不继续递归。array/hash formatter 也需要同样的 visited guard，因为 immutable array 可以通过 instance field 间接参与环。

## 12. GC Playground 演示

### 12.1 Source-driven cycle

推荐内置 snippet：

```monkey
class Node {
  constructor(value) {
    this.value = value;
  }

  connect(other) {
    this.next = other;
  }
}

let makeCycle = fn() {
  let a = new Node("a");
  let b = new Node("b");
  a.connect(b);
  b.connect(a);
};

makeCycle();
```

`makeCycle()` 返回后，两个 instance 没有外部 root，但通过 fields 互相强引用。全局 `Node` class 仍然可达，但 class 不反向持有 instances。

### 12.2 不暂停在 `gc_decref`

`gc_decref` 完成后，引用计数处于 trial-deletion 的临时状态；此时允许用户读对象、继续执行 VM 或单独点击下一阶段会破坏 collector invariant。

Playground 只能原子调用完整 collection：

```text
gc_decref -> gc_scan -> gc_free_cycles
```

然后返回只读 telemetry。不能暴露 `gc_decref()` Monkey builtin，也不能让 UI 暂停 collector 中间阶段。

### 12.3 Collection report

建议 API：

```rust
pub type ValueKindCounts = BTreeMap<ValueKind, usize>;

pub struct HeapSnapshot {
    pub object_count: usize,
    pub tracked_bytes: usize,
    pub by_value_kind: ValueKindCounts,
}

pub struct TrialDeletionStats {
    pub edges_visited: usize,
    pub candidates: usize,
}

pub struct ScanStats {
    pub restored: usize,
    pub garbage_candidates: usize,
}

pub struct FreeCycleStats {
    pub freed: usize,
}

pub struct GcCollectionReport {
    pub before: HeapSnapshot,
    pub after: HeapSnapshot,
    pub trial_deletion: TrialDeletionStats,
    pub scan: ScanStats,
    pub free_cycles: FreeCycleStats,
    pub collected_by_value_kind: ValueKindCounts,
}

impl GcVM {
    pub fn collect_garbage(&mut self) -> GcCollectionReport;
}
```

`ValueKind` 至少稳定区分 `Class`、`Instance`、`BoundMethod`、`Closure`、`Array`、`Hash` 和 `Other`；JSON 使用 lower camel case key。统计职责固定为：

1. `GcRuntime::run_gc_with_stats()` 在一次不可中断的三阶段调用内累计 edge/candidate/restored/freed counters；原有 `run_gc()` 可以忽略返回值并保持兼容。
2. `GcHeap` 在 collection 前按 `GcId` 记录 live `ValueCell` 的 `ValueKind`，并从 `malloc_state.malloc_size` / `gc_object_count()` 构造 snapshot。
3. `GcVM::collect_garbage()` 获取 before，调用一次 `run_gc_with_stats()`，获取 after，并以 collection 前后仍存在的 `GcId` 差集计算 `collected_by_value_kind`。这是提供给 WASM/playground 的唯一 public orchestration；UI 不直接调用单阶段函数。

`GcId` slot 可能在后续 allocation 中复用，所以 kind 差集必须在同一次同步 collection 内完成，不能把 ID 列表留给 JS 延迟解释。

阶段 telemetry 至少包括：

| 阶段           | 指标                                         |
| -------------- | -------------------------------------------- |
| trial deletion | visited edges、candidate count               |
| scan           | restored count、remaining garbage candidates |
| free cycles    | freed object count                           |

UI 核心验收是：

```text
Before: Instance = 2
After:  Instance = 0
Collected by cycle GC: Instance = 2
```

不要用总 object count 精确断言字符串也减少，因为源码 string 常量可能被 bytecode constants 保活。

`tracked_bytes` 只是 Monkey collector 的 accounting proxy，不是浏览器真实 resident memory。WASM linear memory 通常也不会缩页，因此 UI 不能声称“浏览器内存下降”；应表述为“Monkey heap objects reclaimed”。

### 12.4 确定性执行

GC demo 执行流程：

1. 创建 `GcVM`；
2. 把 auto-GC threshold 设为 `usize::MAX`；
3. 以固定 instruction budget 运行源码；超限返回 runtime error，不能让同步 WASM 长时间占住 UI；
4. 获取 before snapshot；
5. 原子执行一次 manual collection；
6. 获取 report / after snapshot；
7. 可选再运行一次，第二次 freed 应为 0。

关闭 auto-GC 只用于教学报告，避免 cycle 在 before snapshot 前已经被阈值触发回收。正常 GcVM 仍保留自动阈值策略。

### 12.5 Playground UI

Playground 已增加独立 `GC` tab 和显式 `Run GC` 按钮：

- 编辑时仍 debounce parse/compile；
- 不在每次按键后自动执行用户程序，避免递归或重程序卡住 UI；
- `Run GC` 使用 WASM API 的 instruction budget；达到 frame 或 instruction limit 时显示 runtime error；
- 显示 program result 的 cycle-safe string；
- 并排显示 before / after snapshot；
- 展示三阶段计数，但不暴露可交互中间堆；
- 增加 `Class cycle (GC)` snippet；
- 错误区分 parse、compile 和 runtime。

## 13. WASM API

`wasm` crate 已增加 `monkey-gc` 依赖，并导出结构化执行入口：

```rust
#[wasm_bindgen]
pub fn run_gc_with_report(source: &str) -> String;
```

这个函数对所有用户源码结果都返回一个可解析的 tagged JSON envelope，不用 JS exception 区分 parse / compile / runtime。成功 contract 固定为：

```json
{
  "status": "ok",
  "result": "null",
  "report": {
    "before": {
      "objectCount": 12,
      "trackedBytes": 1024,
      "byValueKind": { "class": 1, "instance": 2 }
    },
    "after": {
      "objectCount": 10,
      "trackedBytes": 848,
      "byValueKind": { "class": 1, "instance": 0 }
    },
    "phases": {
      "trialDeletion": { "edgesVisited": 8, "candidates": 2 },
      "scan": { "restored": 0, "garbageCandidates": 2 },
      "freeCycles": { "freed": 2 }
    },
    "collectedByValueKind": { "instance": 2 }
  }
}
```

用户源码失败 contract 固定为：

```json
{
  "status": "error",
  "stage": "runtime",
  "message": "property 'next' does not exist on Node",
  "span": { "start": 120, "end": 126 }
}
```

`stage` 只能是 `parse`、`compile` 或 `runtime`；无法定位时 `span` 为 `null`。具体成功数字仅作 schema 示例，测试不固定所有 runtime bookkeeping 对象总数。TypeScript 端为两个 envelope 定义 discriminated union，不接受未标记的 partial report。

Rust 内部执行入口固定为 `Result<GcRunSuccess, GcRunError>`，并携带 stage/message/optional span；`GcVM::run_with_budget` 的新 class/property/call/type/arity/limit 错误必须走这条路径，不能 panic 穿过 WASM 边界。序列化层再把 `Result` 变成上面的 envelope；只有内部 invariant 或 JSON serialization failure 才属于非 contract 的 fatal error。

WASM contract tests 同时覆盖成功、parse error、compile error、runtime type error 和 instruction-limit error。`wasm-pack test --node` 验证 envelope；playground 只根据 `status` 分派 UI。

Rust parser/compiler/runtime 改动后，必须重新执行 `wasm-pack build`；playground 和 Prettier plugin 消费的是 `wasm/pkg`，不是 workspace 中刚修改的 Rust source。

## 14. Prettier 支持

### 14.1 TypeScript AST type

`types.ts` 增加：

- `ClassDeclaration`；
- `MethodDefinition` / `MethodKind`；
- `SetPropertyStatement`；
- `ThisExpression`；
- `PropertyExpression`；
- `NewExpression`。

printer switch 必须覆盖所有新 JSON `type`。

### 14.2 格式

```monkey
class Node {
  constructor(value) {
    this.value = value;
  }

  connect(other) {
    this.next = other;
  }
}
```

- class 不加尾分号；
- method 使用参数 list + block printer；
- methods 保持 AST 源码顺序；
- method 间固定一个空行；
- property set 固定输出分号；
- `this` 原样输出；
- property chain 不添加无意义外围括号；
- `new` 复用 call arguments 的 group/indent 逻辑。

### 14.3 Parentheses

call callee 放开后，不能沿用 index printer 的“总是套括号”做法，也不能完全不看 parent。应增加统一的 parent/child precedence 判断，例如：

```text
(a + b).value
(fn() { 1 })()
(-a).value
new Node().next.value
```

postfix chain 自身不加括号，低优先级 child 作为 postfix object/callee 时才补括号。format 后必须能再次 parse 且不改变 AST 语义。

### 14.4 Comments

至少覆盖：

- class 前 comment；
- constructor body 内 comment；
- methods 之间 comment；
- empty class 内 dangling comment。

empty class/body 已使用 comment-aware 路径，并由 dangling comment 测试保证注释不会丢失。

## 15. 错误语义

建议稳定以下错误类别；最终字符串可按项目现有风格调整，但三个 runtime 必须语义一致。

| 场景                       | 阶段                    | 示例信息                                           |
| -------------------------- | ----------------------- | -------------------------------------------------- |
| local class                | parse                   | `class declarations are only allowed at top level` |
| duplicate constructor      | parse                   | `class Node has more than one constructor`         |
| duplicate method           | parse                   | `duplicate method Node.connect`                    |
| class field declaration    | parse                   | `expected method definition in class body`         |
| property 后缺 name         | parse                   | `expected property name after '.'`                 |
| `new` 无括号               | parse                   | `new expression requires an argument list`         |
| 非 property 赋值           | parse                   | `only instance property assignment is supported`   |
| constructor return value   | compile/eval validation | `constructor cannot return a value`                |
| `this` 无 method context   | compile/eval            | `this is only available inside a method`           |
| method forward global      | compile/eval validation | `undefined variable 'B' in class A.make`           |
| class 普通调用             | runtime                 | `class Node must be constructed with new`          |
| `new` 非 class             | runtime                 | `cannot construct <type>`                          |
| method/constructor arity   | runtime                 | `wrong number of arguments ...`                    |
| 非 instance property read  | runtime                 | `cannot read property 'x' of <type>`               |
| missing property           | runtime                 | `property 'x' does not exist on Node`              |
| 非 instance property write | runtime                 | `cannot set property 'x' of <type>`                |

当前 interpreter 使用 `Result`，默认 VM 多处使用 panic。第一版可以分阶段迁移，但 WASM GC 执行入口必须把用户源码错误转换成可显示的 runtime error，不能依赖 panic 作为产品 API。

## 16. 兼容性与已知前置问题

### 16.1 Bytecode

- 新 opcode 只追加，旧 opcode 数值不变。
- 新 bytecode 不能在旧 VM 上执行，这是正常的 producer/consumer 版本要求。
- compiler debug info 继续按实际 emitted PC 记录，不需要改变现有 side table 设计。

### 16.2 Parser span

实现 class 时一并修复：

- index span 应从 object 起点开始，不是 `[`；
- call/property/new 全部使用统一 span accessor。

Rust span 当前是 UTF-8 byte offset，Prettier/JS location 使用 UTF-16 code unit offset。非 ASCII 文本出现在节点前时可能错位。class PR 至少要增加 Unicode regression test 并记录边界；更完整的方案是在 WASM/plugin boundary 建 byte offset 到 JS offset 的映射。这个问题不应被误认为 class AST 自身的 span 错误。

### 16.3 Public GC export

现有 `gc::eval_source` 返回 `object::Object`。cyclic class graph 不能继续走无 memo 的递归 export。可选收敛方式：

1. 保留旧 API 只支持可导出的 acyclic value，并对 graph value 返回明确 error；
2. 新增 Gc-native execution/inspect result，playground 使用新 API；
3. 后续实现 graph-aware memo exporter。

本提案推荐第 2 项作为 class / playground 主路径。不要为了维持旧 helper 而在 builtin 热路径深拷贝 graph。

## 17. 测试与验收

### 17.1 Lexer

- `class` / `this` / `new` / `.` token 与精确 span；
- `constructor` 仍是 identifier；
- keyword boundary：`className`、`newNode`、`thisValue`；
- identifier digit continuation：`Node2`。

### 17.2 Parser / AST snapshot

- empty class；
- constructor + 多个 methods，保持顺序；
- default constructor；
- this read / nested property read / nested property set；
- `new Node(1).connect(other)`；
- `a.b(c).d[0]` postfix chain；
- bound method source：`let f = node.method; f();`；
- precedence：`-this.value`、`this.value + 1`、`(a + b).value`；
- grouped postfix span：`(a + b).value`、`(fn() { 1 })()`、`((a)).value` 都从最外层 `(` 起算；
- 每个新节点的 source-slice span assertion；
- constructor body 语法错误必须返回 error，不能静默成功或 panic。

negative cases：

- duplicate constructor/method；
- 缺 class name、brace、method paren/body；
- `.` 后缺 name；
- `new` 缺 class/parentheses；
- local class；
- class field declaration；
- forward global in method、普通 function 中未绑定的 `this`；
- `x = 1`、`a[0] = 1`、`let x = a.b = 1`。

### 17.3 Interpreter

- constructor 初始化字段；
- default constructor 和 arity；
- property create/read/overwrite；
- method call 与尾表达式返回；
- detached bound method 保留 receiver；
- bound method identity：`let f = node.method; f == f` 为 true，而 `node.method == node.method` 为 false；
- field shadows method；
- 一层和两层 nested `fn` 都能逐层 capture `this`；
- class/instance identity；
- missing property 和非 instance receiver error；
- class 不可直接 call，普通 fn 不可 `new`；
- constructor explicit return rejection。
- interpreter 对 method / constructor 的少参和多参都返回 strict arity error，不 panic/忽略；
- receiver、property RHS 和 `new` arguments 都按规定顺序且各求值一次；
- `Object` 的 identity/hash/display tests 包含 self-cycle，失败输出不能递归溢出。

### 17.4 Compiler / 默认 VM

- class declaration 精确 instruction test；
- 修复 instruction helper 的完整长度断言，不能只用 `zip` 比较共同前缀；
- method local 0 是 `this`；
- 一层和两层 nested closure 把 `this` 逐层编译成 free symbol；
- property get/set/new instruction sequence；
- class declaration / property set 作为程序最后一条 statement 时结果为 `null`；
- method arity 不向用户暴露 hidden receiver；
- constructor 固定尾部返回 local 0，复用普通 frame 后得到 instance；
- interpreter 与 VM 共用语义 case table。

### 17.5 GcVM

- 与默认 VM 共用全部 class 语义 case；
- self-cycle 被 manual GC 回收；
- 两 instance cycle：`Instance 2 -> 0`；
- 仍被 global/root 持有的 cycle 不被回收；
- field overwrite 释放旧 edge；
- detached bound method 单独保活 receiver；
- instance field 持有自身 bound method 形成的环可回收；
- constructor 内触发 auto-GC 时 receiver 存活；
- class -> constructor/method edges 在 GC 后仍存活；
- 第二次 collection freed count 为 0；
- native `first/last/rest/push` 保留 GcRef identity/alias；
- display/inspect cyclic graph 不递归溢出。

GC 核心 acceptance：

```rust
// auto GC disabled for deterministic before snapshot
vm.run()?;
assert_eq!(vm.stats().by_value_kind.instance, 2);

let report = vm.collect_garbage();
assert_eq!(report.after.by_value_kind.instance, 0);
assert_eq!(report.collected_by_value_kind.instance, 2);
```

### 17.6 Prettier / WASM / Playground

- 完整 class golden format；
- long params / args 换行；
- property set 和 postfix chain parentheses；
- comments 四类；
- format twice idempotent；
- format 结果重新通过 WASM parser；
- WASM success/error envelope contract（含 parse/compile/runtime/limit stage）；
- 为 playground 引入 Vitest + React Testing Library，覆盖 GC tab success/error、Run button 和 stale response handling；
- `Class cycle (GC)` snippet 的 Instance `2 -> 0`。

## 18. 实施顺序

实现按下面的阶段完成；列表保留为变更审阅索引。

### Phase 0：Parser 基础加固

- block error propagation；
- binary RHS 移除 unwrap；
- expression span accessor + parser-local grouped cover span；
- call callee 放开；
- identifier digit、index span 修正。
- 恢复 compiler instruction test helper 的 length assertion。

### Phase 1：Syntax、AST 与 formatter

- lexer tokens；
- class/property/new parser；
- shared semantic validator；
- AST serde/snapshots；
- Prettier types/printer/tests；
- VS Code TextMate grammar 的 `class` / `this` / `new` / method/property coverage；
- rebuild `wasm/pkg` 验证 round-trip。

### Phase 2：共享对象模型与 interpreter

- Class / Instance / BoundMethod；
- identity/display/hash audit；
- interpreter class semantics；
- interpreter tests。

### Phase 3：Compiler 与默认 VM

- append opcodes；
- class/method lowering；
- 修复 multi-level free-symbol 逐层 capture；
- hidden this slot；
- bound/constructor 普通 frame 调用改写；
- compiler/VM parity tests。

完成本阶段后，基础 class 已可用；默认 `Rc` 后端的 cycle leak 是预期状态。

### Phase 4：GcVM

- native `BuiltinId` dispatch；
- GcClass / GcInstance / GcBoundMethod trace；
- mutation ownership helpers；
- class opcodes / 调用栈 ownership；
- cycle-safe inspect；
- GC report/statistics；
- semantic + cycle tests。

### Phase 5：WASM 与 playground

- GcVM WASM dependency / report API；
- rebuild wasm package；
- GC tab、Run button、snippet；
- playground Vitest/RTL test setup；
- package test/lint/build，以及 `wasm-pack test --node`。

## 19. 文件改动索引

实际影响：

| 层            | 主要文件                                                                                                       |
| ------------- | -------------------------------------------------------------------------------------------------------------- |
| lexer         | `lexer/token.rs`, `lexer/lib.rs`, lexer tests/snapshots                                                        |
| AST/parser    | `parser/ast.rs`, `parser/lib.rs`, `parser/precedences.rs`, 新增 `parser/validation.rs`, parser tests/snapshots |
| shared object | `object/object.rs`, `object/builtins.rs`，可新增 `object/class.rs`                                             |
| interpreter   | `interpreter/lib.rs`, `interpreter/interpreter_test.rs`                                                        |
| bytecode      | `compiler/op_code.rs`, `compiler/compiler.rs`, `compiler/symbol_table.rs`, compiler instruction/tests          |
| default VM    | `compiler/vm.rs`, VM tests                                                                                     |
| GcVM          | `gc/lib.rs`, `gc/value.rs`, `gc/vm.rs`, `gc/heap.rs`, `gc/runtime.rs`, GC tests                                |
| WASM/build    | `wasm/Cargo.toml`, `wasm/src/lib.rs`, WASM tests, root `Cargo.lock`                                            |
| Prettier      | `packages/prettier-plugin-monkey/src/types.ts`, `printer.ts`, fixtures/tests                                   |
| Playground    | `packages/playground/package.json`, `src/App.tsx`, report types、styles/components、Vitest/RTL tests           |
| VS Code       | `packages/vscode-extension/syntaxes/monkey.tmLanguage.json`, extension build                                   |
| docs          | 本文、`docs/gc.md`、`gc/gc-report.md`、`gc/README.md`、`packages/playground/README.md`                         |

`wasm/pkg` 是本地 `wasm-pack build` 产物并已被 package 消费；实现时必须重建以验证，但除非仓库策略另行改变，不把该忽略目录提交到 class PR。

实现阶段不做 package/version bump，也不包含 publish。

## 20. 延后能力与备选方案

### 20.1 后续演进

建议顺序：

1. local class + recursive binding cell；
2. class expression；
3. 通用 assignment expression；
4. computed instance property；
5. static members；
6. `extends` / `super`；
7. prototype semantics（只有明确需要时）；
8. constructor object-return override（只有要进一步兼容 JS 时）。

### 20.2 为什么不是 `init`

`init` 常见于教学语言或 clox，但用户语法会变成另一套约定：

```monkey
class Node {
  init(value) { ... }
}
```

本设计的目标明确是 JS 风格，所以 constructor 名称、`new` 和 method syntax 都直接采用 JS 形态。runtime 仍可以用独立 constructor slot 实现，不需要把用户语法改成 `init`。

### 20.3 为什么不是 prototype-first

prototype-first 会同时要求 property lookup chain、constructor prototype、method mutation、receiver call convention 和更多 observable reflection。它不是展示 GC 环所需的最小集合，也会显著扩大 interpreter/default VM/GcVM 的一致性面。

明确的 Class + Instance 模型可以先稳定语法、调用约定和 ownership；未来若加入 prototype，可在 class lookup 后增加 parent/prototype edge，而不推翻基础 instance fields。

### 20.4 为什么不把 method call 编译成专用 `OpInvoke`

`OpInvoke name argc` 可以避免临时 bound method allocation，但无法单独覆盖：

```monkey
let f = object.method;
f();
```

第一版用 `OpGetProperty + OpCall` 先得到统一、可解释的语义。后续可以把直接 `object.method(args)` peephole 优化为 `OpInvoke`，但必须与 bound method 行为等价。

### 20.5 完成定义

本提案的基础 class 完成，不以“parser 能识别 class”为准，而以以下闭环为准：

- 三个执行后端运行同一组 class 语义测试；
- detached method、identity、constructor/new 和错误语义一致；
- GcVM 从纯 Monkey 源码构造并回收双向 instance cycle；
- playground 显示 `Instance: 2 -> 0` 及三阶段原子 telemetry；
- AST/Prettier/WASM round-trip 无旧包污染；
- 当前 GC 文档、playground README 和 VS Code grammar 不再保留与实现冲突的旧限制；
- 全 workspace Rust tests、Prettier tests/build、VS Code build、playground test/lint/build 和 WASM node tests 通过。
