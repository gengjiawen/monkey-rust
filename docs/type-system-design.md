# Monkey Type System 设计提案

> 状态：已实现（PR #326，待合并）。设计经过四轮评审修订：擦除边界与声明例外、字段初始化模型、null-stripping、对拍范围、AST JSON 兼容性（首轮）；equality 矩阵、union 消解、拒绝方法名赋值、completion 返回推导、索引与泛型实例化（次轮）；any 操作规则、equality 跨 class 修正、字段收集冲突与依赖降级、crates.io 发布影响、builtin 类型名遮蔽、API 收敛（三轮）；同名 class 的声明 identity、`this` alias 字段收集、any 运算结果完备性、builtin 泛型的 any 约束与测试语料（四轮）。
>
> 核心结论：Monkey 增加 TypeScript 风格的可选类型标注语法（`let x: int = 5`、`fn(a: int): int`）。Rust 侧只有 lexer/parser/AST 参与——解析标注并随 JSON AST 导出；类型检查器是新的 TypeScript 包 `packages/monkey-typechecker`，通过 WASM 消费 AST，属于纯建议性静态分析。四个执行后端（interpreter、默认 VM、GcVM、asm）执行**类型擦除**语义：标注不改变任何运行时行为，带标注与去标注的同一程序必须产生逐字节相同的 instructions 与 constants（擦除边界与声明例外见 6.1）。
>
> 关联设计：[JS 风格 Class 设计](./js-style-class-design.md)、[Linter 计划](./linter-plan.md)。

## 目录

1. [背景与结论](#1-背景与结论)
2. [目标与非目标](#2-目标与非目标)
3. [用户语法](#3-用户语法)
4. [类型文法与解析](#4-类型文法与解析)
5. [AST 与 Span](#5-ast-与-span)
6. [擦除语义与后端影响](#6-擦除语义与后端影响)
7. [类型检查语义](#7-类型检查语义)
8. [Checker 架构](#8-checker-架构)
9. [工具链与生态影响](#9-工具链与生态影响)
10. [诊断与错误语义](#10-诊断与错误语义)
11. [兼容性](#11-兼容性)
12. [测试与验收](#12-测试与验收)
13. [实施顺序](#13-实施顺序)
14. [文件改动索引](#14-文件改动索引)
15. [延后能力与备选方案](#15-延后能力与备选方案)
16. [完成定义](#16-完成定义)

---

## 1. 背景与结论

Monkey 目前是完全动态类型的语言。所有类型错误在运行时才暴露，且三个 runtime 的报错行为存在已知分歧（例如混合类型 `==`：interpreter 返回 `false`，GcVM 报错）。linter 的 `no-literal-type-mismatch` 规则是仓库里最接近类型检查的能力，但它只检查字面量操作数——变量一旦介入就沉默。

本提案引入一个渐进式（gradual）类型系统：

- **语法**：类型标注处处可选。未标注的**参数**默认 `any`，未标注的 `let` 绑定从 RHS 推导（见 7.3）；与现在完全一致的程序继续完全合法。
- **检查器在 TS 侧**：复用 linter/minifier 已验证的架构（WASM 出 JSON AST + TS 分析），不在 Rust 侧新增 checker crate。
- **擦除语义**：标注只服务于静态分析和工具链。interpreter/VM/GcVM/asm 不读取标注，bytecode、执行结果、错误行为全部不变（唯一声明例外：interpreter 对函数值的 source-reflective 渲染/相等，函数嵌套于 array/hash 时同样可被间接观察，见 6.1）。

选择这个组合的原因：

- Monkey 没有循环（迭代靠递归）、没有通用变量重赋值（`x = 2` 是 parse error，re-`let` 是 shadowing）。这两个性质使 checker 不需要流敏感分析和循环不动点，实现复杂度显著低于常规语言。
- `parser/validation.rs` 已经是四个前端共享的 post-parse 语义 pass，且 `analyze_lossless` 已经把 parse + validate 的错误 envelope 输出到 JS 侧（validation error 带 span；parse error 目前是无 span 的字符串，checker 原样透传、不依赖其 span）。checker 挂在这条链路之后，可以假设所有 identifier 已解析、`this` 位置合法。
- 每个 token 和 AST 节点都带字节级 `Span`，playground 已有 UTF-8 byte span 到编辑器坐标的转换（`sourceSpan.ts`）。诊断定位的基础设施是现成的。

## 2. 目标与非目标

### 2.1 目标

- TypeScript 风格标注语法：`let` 绑定、函数参数、函数/方法返回值。
- 类型标注处处可选；无标注程序在源码层面全部继续合法，运行时行为与现在一致。AST JSON 形状本次属 breaking 的内部契约变更（对无标注程序同样变化，见第 11 节），不做不变承诺。
- 类型名是软关键字：不新增保留字，`let int = 5` 仍然合法。
- 严格的擦除保证：同一程序带/不带标注编译出逐字节相同的 instructions 与 constants，四个后端执行结果一致（边界与声明例外见 6.1）。
- checker 输出多条带 span 的结构化诊断（不同于 validation 的 fail-fast 单错误）。
- 渐进式检查：`any` 是合法类型且与一切双向兼容；标注越多检查越精确。
- 检查规则以**实测运行时语义**为准（延续 linter-plan 的原则），并覆盖 builtin 签名与 arity。
- Prettier round-trip、minifier、linter、VS Code grammar、playground 与新语法同步。

### 2.2 非目标

第一版明确不实现：

- Rust 侧类型检查器；REPL / CLI 执行路径的类型门禁（checker 永不阻止程序运行）。
- 运行时类型强制或基于类型的代码生成/优化（asm 去 SMI tag、GcVM unboxing 属于远期）。
- 用户可见的泛型语法（`fn<T>` 等；builtin 签名内部使用受限泛型，见 7.6）。
- union 类型的用户语法（`int | string`；checker 内部使用 union 表示，见 7.3）。
- 类型别名（`type Point2 = ...`）、interface/record 声明。
- class 字段声明语法（字段类型从方法体中 `this` 及其简单 alias 的 `.x = ...` 赋值收集，见 7.8）。
- `null` 字面量、null 判断与 flow narrowing（`strictNull` 因此默认关闭，见 7.5）。
- float、char 等新的值类型。
- 对 `debugger` 语句、GC、快照等既有能力的任何行为修改。

## 3. 用户语法

### 3.1 let 标注

```monkey
let version: int = 1 + (50 / 2) - (8 * 3);
let name: string = "The Monkey programming language";
let flags: [bool] = [true, false];
let ages: {string: int} = {"Anna": 24, "Bob": 99};
let x = 5; // 仍然合法，类型推导为 int
```

### 3.2 函数参数与返回值

```monkey
let add = fn(a: int, b: int): int { a + b; };

// 参数标注彼此独立可选；未标注即 any
let mixed = fn(a: int, b) { a; };

// 函数类型作为参数
let apply = fn(f: fn(int): int, x: int): int { f(x); };
```

返回值标注使用 `:` 而非 `->`，与参数标注风格一致，并且不需要新增 token。

### 3.3 class 方法

```monkey
class Point {
  constructor(x: int, y: int) {
    this.x = x;
    this.y = y;
  }

  sum(): int {
    this.x + this.y;
  }
}

let point: Point = new Point(20, 22);
```

class 名即类型名（nominal）。constructor 不写返回类型（其"返回值"恒为 receiver instance，与 class 设计一致）。

### 3.4 可空类型

```monkey
let maybe: int? = first([1, 2, 3]); // first 对空数组返回 null
```

`T?` 表示 `T` 或 `null`。数组索引、hash 取值、无 `else` 的 `if` 表达式、`first`/`last`/`rest` 都会产生可空类型（见 7.5）。

### 3.5 完整类型形态一览

| 写法                  | 含义                                                                               |
| --------------------- | ---------------------------------------------------------------------------------- |
| `int` `bool` `string` | 基础类型                                                                           |
| `any`                 | 动态类型，与一切双向兼容                                                           |
| `null`                | 只有 `null` 一个值的类型（`puts` 的返回类型）                                      |
| `Point`               | class 实例类型（nominal）                                                          |
| `[T]`                 | 元素为 `T` 的数组                                                                  |
| `{K: V}`              | key 为 `K`、value 为 `V` 的 hash；`K` 语义上限定 `int` / `bool` / `string` / `any` |
| `fn(T1, T2): R`       | 函数类型；type position 的返回类型**必写**                                         |
| `T?`                  | `T` 或 `null`                                                                      |
| `(T)`                 | 分组，如 `(fn(int): int)?`                                                         |

## 4. 类型文法与解析

### 4.1 新 token

只新增一个：

```text
QUESTION -> ?
```

`:`（COLON）、`[` `]`、`{` `}`、`(` `)`、`fn` 全部复用现有 token。

### 4.2 软关键字

`int`、`bool`、`string`、`any`、`null` 不进入 `lookup_identifier` 的关键字表，继续 lex 为 `IDENTIFIER`。parser 仅在 type position 把它们解释为类型名；其余位置照旧是普通标识符。因此存量程序（包括 `let int = 5;`）零破坏。class 名同理——type position 的任意 `IDENTIFIER` 都解析为 `Named` 类型，名字是否存在、是否为 class 由 TS checker 判定，parser 不做语义校验。

**类型名解析优先级**：type position 的 `int` / `bool` / `string` / `any` / `null` **恒指 builtin 类型**。parser 允许 `class int {}`（class 名只是普通 IDENTIFIER），但这样的 class 实例类型在标注里无法引用——写 `: int` 得到的仍是 primitive，只能退回 `any`；checker 对 class 名与 builtin 类型名同名的声明发 `reserved-type-name` warning。

### 4.3 Grammar

```ebnf
let_statement     ::= "let" IDENTIFIER type_annotation? "=" expression ";"? ;

type_annotation   ::= ":" type ;

parameter         ::= IDENTIFIER type_annotation? ;

function_literal  ::= "fn" "(" parameter_list? ")" type_annotation? block_statement ;

method_definition ::= IDENTIFIER "(" parameter_list? ")" type_annotation? block_statement ;
                      (* constructor 不接受返回类型标注 *)

type              ::= postfix_type ;
postfix_type      ::= primary_type "?"* ;
primary_type      ::= IDENTIFIER
                    | "[" type "]"
                    | "{" type ":" type "}"
                    | "fn" "(" type_list? ")" ":" type
                    | "(" type ")" ;
type_list         ::= type ( "," type )* ;
```

### 4.4 解析要点

- **类型文法是独立子文法，但不创建第二套 parser**。它由同一 `Parser` 上的递归下降方法解析，共用 lexer、token 游标、ParseError 与 span 基础设施，但不进入 Pratt 表达式解析器。type position 由前置的 `:`（或类型内部结构）唯一确定，`[int]` 与数组字面量、`{string: int}` 与 hash 字面量不存在歧义。
- Monkey 不区分 typed/untyped loader：类型标注是同一种 `.monkey` 源码中的可选语法，parser 无需模式开关；只在明确的 type position 调用 `parse_type*`。
- **函数类型的返回类型必写**（`fn(int): int`、无返回值写 `fn(): null`）。这消除了嵌套场景的贪婪吸附歧义：`{fn(int): int: bool}` 里第一个 `:` 归函数类型、第二个归 hash 分隔符，无需回溯。函数**字面量**的返回标注仍然可选（可推导）。
- `?` 为 postfix、可叠加解析但 `T??` 归一化为 `T?`。`?` 绑定最紧：`fn(int): int?` 的 `?` 修饰返回类型；要表达可空函数用 `(fn(int): int)?`。
- constructor 带返回类型标注是 parse error（对齐"constructor 不能 return 值"的既有规则）。
- 类型语法错误使用现有 ParseError 通道，给出定向信息，例如 `expected type after ':'`、`function type requires a return type`。

## 5. AST 与 Span

### 5.1 新增节点

```rust
#[serde(untagged)] // 与 Statement / Expression 同模式
pub enum TypeAnnotation {
    Named(NamedType),
    Array(ArrayType),
    Hash(HashType),
    Function(FunctionType),
    Optional(OptionalType),
}

pub struct NamedType {
    pub name: String,          // "int" | "bool" | "string" | "any" | "null" | class 名
    pub span: Span,
}

pub struct ArrayType {
    pub element: Box<TypeAnnotation>,
    pub span: Span,
}

pub struct HashType {
    pub key: Box<TypeAnnotation>,
    pub value: Box<TypeAnnotation>,
    pub span: Span,
}

pub struct FunctionType {
    pub params: Vec<TypeAnnotation>,
    pub return_type: Box<TypeAnnotation>,
    pub span: Span,
}

pub struct OptionalType {
    pub inner: Box<TypeAnnotation>,
    pub span: Span,
}
```

各 struct 沿用现有节点的 `#[serde(tag = "type")]` 模式，JSON `type` tag 即 struct 名：`NamedType`、`ArrayType`、`HashType`、`FunctionType`、`OptionalType`。enum 自身必须是 `untagged`——若在 enum 上打 tag，得到的会是 variant 名 `Named`/`Array`，与既有 JSON 风格不符。分组括号不产生 AST 节点，只影响外层节点的 cover span（沿用 class 设计中 grouped expression 的处理方式）。

### 5.2 现有节点变更

```rust
pub struct Let {
    pub identifier: IDENTIFIER,                  // 从 Token 重构为 IDENTIFIER（见下）
    pub type_annotation: Option<TypeAnnotation>, // 新增
    pub expr: Expression,
    pub span: Span,
}

pub struct Param {                               // 新增
    pub identifier: IDENTIFIER,
    pub type_annotation: Option<TypeAnnotation>,
    pub span: Span,
}

pub struct FunctionDeclaration {
    pub params: Vec<Param>,                      // 原 Vec<IDENTIFIER>
    pub return_type: Option<TypeAnnotation>,     // 新增
    pub body: BlockStatement,
    pub span: Span,
    pub name: String,
}

pub struct MethodDefinition {
    pub kind: MethodKind,
    pub name: IDENTIFIER,
    pub params: Vec<Param>,                      // 原 Vec<IDENTIFIER>
    pub return_type: Option<TypeAnnotation>,     // 新增；kind = Constructor 时恒为 None
    pub body: BlockStatement,
    pub span: Span,
}
```

`Let.identifier: Token` 是 `ast.rs` 里注释多年的历史遗留（"rust can't do precise type with enum"）。本次下游 JSON shape 反正要同步，是把它重构为 `IDENTIFIER` 的最便宜时机，随 2a 一并完成。

`Object::Function` 内嵌 `Vec<IDENTIFIER>` params，随 `Param` 机械更新（interpreter 绑参、compiler 定义 symbol 处取 `param.identifier`）。

### 5.3 序列化

- `Option<TypeAnnotation>` 为 `None` 时序列化为 `null` 字段；TS 侧类型为 `TypeAnnotation | null`。
- 新节点无数值字段，不涉及 `stringify_integer_literals` 的 i64 精度重写。
- `Display` 实现同步输出标注（`let x: int = 5;`、`fn f(a: int): int { ... }`），保持"Display 输出可再 parse"的现状。注意 interpreter 的函数值渲染与结构相等复用 AST 的 Display/Eq，由此产生的可观察差异在 6.1 中作为声明例外处理。

### 5.4 Span 规则

所有 span 沿用 half-open byte range `[start, end)`：

| 节点             | span                                     |
| ---------------- | ---------------------------------------- |
| `NamedType`      | 类型名 token 本身                        |
| `ArrayType`      | `[` 起点到 `]` 末尾                      |
| `HashType`       | `{` 起点到 `}` 末尾                      |
| `FunctionType`   | `fn` 起点到返回类型末尾                  |
| `OptionalType`   | inner 起点到 `?` 末尾                    |
| `Param`          | 参数名起点到标注末尾（无标注时即参数名） |
| 带分组括号的类型 | cover span 从最外层 `(` 起算             |

parser test 对每种类型节点执行 `&input[span.start..span.end]` 精确切片断言。

## 6. 擦除语义与后端影响

### 6.1 擦除保证

这是本提案的硬性约束：

1. **bytecode 恒等**：`let x: int = 5;` 与 `let x = 5;` 的 instructions 与 constants 逐字节相同；不含 debug info 的编译产物（`compile` 输出、strip-debug 快照）逐字节相同。compiler 不读取 `type_annotation` / `return_type`，不新增 opcode。**debug info 不在恒等范围内**：`PcSpan` 记录的是绝对字节偏移，插入标注必然移动后续 span——两份产物的 debug info 应各自准确映射回各自的源码，单独校验，不做互相恒等断言。debug info 里的非 span 字段（`local_bindings`、`free_names`）仍在恒等范围内。asm 后端同理：emit 出的指令逐条相同，但 `.s` 里逐行回显源码的 `//` 注释属于 debug info，比较时先剥离。
2. **执行恒等**：同一程序带/去标注在 interpreter、默认 VM、GcVM、asm 四个后端产生相同的计算结果、控制流与错误行为。**声明例外——函数值的 source-reflective 操作**：interpreter 的 `Object::Function` 直接内嵌 AST 的 params 与 body，其 `Display` 渲染（`puts(fn(x: int) { x })` 会连标注一起输出）与结构 `==`（比较 params/body AST，标注参与相等判定）都会观察到标注。函数值渲染本就后端分歧（VM/GcVM 输出 `[closure function]`，asm 输出 `[function]`），本设计不为此引入运行时擦除层，改为专项测试固定该例外；擦除恒等语料不打印、不比较**任何包含函数值的值**——函数嵌套在 array/hash 里同样会被 Display/`==` 间接观察到。
3. **checker 不是门禁**：类型检查不通过的程序照常可以执行。是否在 playground / CI 中把诊断当作 error 是消费方的策略，不属于语言语义。

### 6.2 各后端实际改动

| 后端                       | 改动                                        | 原因                                                 |
| -------------------------- | ------------------------------------------- | ---------------------------------------------------- |
| interpreter                | 机械（绑参处取 `param.identifier`）         | `Object::Function` 内嵌 params                       |
| compiler                   | 机械（定义 symbol 处取 `param.identifier`） | 遍历 AST                                             |
| 默认 VM (`compiler/vm.rs`) | **零**                                      | 只消费 bytecode 与 `CompiledFunction`，标注到不了 VM |
| GcVM (`gc/vm.rs`)          | **零**                                      | 同上                                                 |
| asm (`asm/lower.rs`)       | 机械（match 新 AST 形状，忽略标注）         | 直接遍历 AST                                         |
| `parser/validation.rs`     | 机械（遍历骨架适配 `Param`；不校验类型）    | 类型语义完全归 TS checker                            |

## 7. 类型检查语义

以下规则全部实现在 TS checker 中。基调：**以实测运行时语义为准，宁可漏报不误报**（unsound-but-useful，与 TypeScript 同一取向）。

### 7.1 类型全集

```text
Type ::= Int | Bool | String | Null | Any
       | Array(Type) | Hash(Type, Type)
       | Fn(params: Type[], ret: Type)
       | Class(id, displayName)        // class 值本身，new 的操作数
       | Instance(id, displayName)     // new 出来的实例，按声明 identity nominal
       | Union(Type[])                 // 内部表示，无用户语法；T? = Union(T, Null)
```

每个 `ClassDeclaration` 在 checker 内获得唯一且不透明的 `ClassId`；`displayName` 只用于诊断展示，不参与类型相等。Monkey 允许同名 class 按源码顺序 shadow：后一个 `class A` 绑定新的 `ClassId`，而在此之前保存的 alias（`let Old = A`）仍携带旧 identity。类型标注中的 class 名同样按源码顺序解析到当时可见的声明；builtin 类型名仍按 4.2 的优先级恒指 primitive。解析不到的类型名报 `unknown-type-name`，该标注按 `any` 继续参与检查。与值层面的 forward-global 规则一致，标注不能前向引用后声明的 class（自引用不受影响——class 名先于自身 methods 进入 scope），因此两个 class 相互引用对方的标注 v1 不可表达，留空退回 `any` 即可。这样 `new Old()` 与 `new A()` 即使都显示为 `A`，也不会被 checker 合并成同一 nominal 类型。

`Union` 归一化：扁平、去重、含 `Any` 即坍缩为 `Any`；class/instance 成员按 `ClassId` 去重，而不是按显示名去重。

**Union 消解（elimination）通用规则**：对静态类型为 union 的值执行任何操作（运算符、`==`、调用、索引、属性访问、`new`）时，先做 null-stripping（见 7.5），然后操作必须对**剩余每个成员**都合法，结果类型 = 各成员结果类型的 join；任一成员不合法即报该操作对应的诊断。调用与 `new` 额外要求全体成员 arity 一致，实参须对每个成员的对应参数位都 assignable。7.4 的运算符条目、7.7 的调用、7.8 的属性访问都是本规则在具体操作上的实例。

```monkey
let f = if (c) { fn(x: int): int { x; } } else { fn(x: string): string { x; } };
f(1); // error: 实参 int 对成员 fn(string): string 不合法

let xs = if (c) { [1] } else { ["a"] };
xs[0]; // 合法：xs 是 [int] | [string]，两成员都可按 int 下标索引，结果 (int | string)?
```

### 7.2 兼容性（assignability）

`assignable(from, to)` 规则，按序判定：

1. `from = Any` 或 `to = Any` → 兼容（gradual 核心）。
2. 完全相同 → 兼容。
3. `to = Union(...)` 且 `from` 兼容其任一成员 → 兼容。
4. `from = Union(members)` → 先执行 null-stripping（见 7.5），剩余每个成员都兼容 `to` 才兼容。
5. `Array(a) → Array(b)`：`a` 兼容 `b` 即可（协变）。Monkey 数组不可变（无索引赋值，`push` 返回新数组），协变是 sound 的。
6. `Hash` 同理协变；`Fn` 参数逆变、返回协变，arity 必须相等。
7. `Instance(aId, _) → Instance(bId, _)`：`aId == bId`（按声明 identity nominal，无继承）。`Class` 的“完全相同”同样比较 `ClassId`，不比较显示名。

### 7.3 推导与 join

- 字面量：`5: int`、`true: bool`、`"a": string`。
- `join(T, U)`：`T == U` 时为 `T`，任一为 `Any` 时为 `Any`，否则 `Union(T, U)`。
- 数组字面量：元素类型逐一 join。`[1, 2]: [int]`；`[1, "a"]: [int | string]`；`[]: [any]`。
- hash 字面量：key、value 分别 join。`{"name": "Anna", "age": 24}: {string: string | int}`（union 成员按首次出现顺序排列）。
- `if` 表达式：有 `else` 时 join 两分支；无 `else` 时 `join(consequent, null)`（即 `T?`）。分支值为 block 尾表达式，与运行时尾值规则一致。
- `let`：有标注时校验 RHS assignable 并以**标注**为绑定类型；无标注时以推导结果为绑定类型。re-`let` shadowing 直接覆盖绑定类型。

示例（这是 `join` 采用 union 而非坍缩为 `any` 的动机）：

```monkey
let x: int = if (c) { 1 } else { "a" };
// error: type 'int | string' is not assignable to 'int'
```

### 7.4 运算符

延续 `no-literal-type-mismatch` 的实测语义表，但操作数扩展为任意表达式的静态类型：

| 运算        | 要求                                                   | 结果             |
| ----------- | ------------------------------------------------------ | ---------------- |
| `+`         | `int + int` 或 `string + string`                       | `int` / `string` |
| `-` `*` `/` | `int + int`                                            | `int`            |
| `<` `>`     | `int + int`                                            | `bool`           |
| `==` `!=`   | 独立 equality 矩阵（见下方条目，不复用 assignability） | `bool`           |
| prefix `-`  | `int`                                                  | `int`            |
| prefix `!`  | 任意                                                   | `bool`           |

- **Any 操作规则**（统一豁免）：任一操作数为 `any` 时检查一律通过。结果类型：单一重载的运算符直接取其结果（`any - 1`、`any * any` → `int`；`any < 1` → `bool`；prefix `-` → `int`）；`+` 的另一侧为 `int` / `string` 时分别得到 `int` / `string`，另一侧为 `any` 或其他具体类型时降级为 `any`（`any + true`、`any + [1]` → `any`），保证统一豁免下结果类型完备。比较与 `!` 恒 `bool`。索引 `any[...]`、调用 `any(...)`、`new any(...)`、属性 `any.prop` 全部合法且结果 `any`（7.7/7.8 的 callee/receiver 条目是该规则的实例）。这条规则是 gradual 的落地面：hello.monkey 的 `getName(person)`、未标注 fibonacci 的 `x - 1` 与 `fibonacci(x-1) + fibonacci(x-2)` 全靠它零诊断。
- 操作数为 `Union`：先套用 7.1 的 union 消解通用规则（null-stripping 后逐成员检查，结果取各成员结果的 join），再对每个具体成员组合应用上面的 `Any` 规则。例如 `(int | string) + int` 报错——运行时可能命中 `string + int`，所有后端都会报错；`any + (int | string)` 得 `int | string`；`any + (int | bool)` 因 `any + bool` 的结果为 `any`，join 后坍缩为 `any`；而 `int? + 1` 剥离 `null` 后按 `int + int` 通过。
- `==` / `!=` 不复用 assignability，使用独立的 **equality 矩阵**（null-stripping 后判定，任一侧 `Any` 豁免）：
  - 可比较类别：`int`、`bool`、`string`、`null`、`Class`、`Instance`。两侧同类别 → 通过，结果 `bool`。`Instance` 与 `Class` 按 identity 比较且**不要求同名 class**——GcVM 的匹配臂只看类别（对任意 Instance/Instance 直接 identity，不检查所属 class），`new A() == new B()` 在四个后端都合法、恒 `false`，checker 如实放行（要提示可日后作为 lint 规则）。
  - **`Array` / `Hash` / `Fn` 一律拒绝**，报 `invalid-comparison`，且该拒绝**优先于 `Any` 豁免**——`[1] == x` 在 `x: any` 时照报：GcVM 对数组/哈希/函数操作数不论另一侧是什么都会报错，已知一侧是容器就足以断定。运行时依据（`gc/vm.rs` 的 `execute_comparison`）：GcVM 只支持标量、`null` 与 class/instance/bound method，`[1] == [1]` 是 runtime error；而 interpreter 与默认 VM 走 `Object::PartialEq` 结构比较、asm 深比较/恒等——同一表达式三种行为，静态拒绝是唯一能对齐最严后端（GcVM）的选择。`Fn` 另有一层：静态类型无法区分 closure（GcVM 报错）与 bound method（GcVM identity 合法），只能保守全拒。
  - 类别不同 → `mixed-equality`：GcVM 报错、其余后端静默 `false`，checker 对齐更严格的 GcVM，同时实现了 linter-plan 里提议的 `backend-divergent-comparison`。
- `if` 条件不限制类型（运行时 truthiness 对一切值有定义，仅 `false` 与 `null` 为假）。

**索引**（合法性规则；结果类型见 7.5 的表）：

- `array[i]`：目标 `[T]` 时下标静态类型必须为 `int`（`any` 豁免），否则 `invalid-index`（运行时 `index operator not supported`）。
- `hash[k]`：目标 `{K: V}` 时下标须可哈希且 assignable 到 `K`，否则 `invalid-index`。注意异型 key（`{"x": 1}[1]`）运行时**不报错**、恒 miss 返回 `null`——此处 checker 严于运行时（与 7.8 拒绝未知属性同一取向：几乎总是拼写/类型错误），确需动态 key 时把目标或下标标 `any`。
- 目标为其他具体类型（`string`、`int`、`Fn` 等）→ `invalid-index`（运行时同样报错）。`any` 豁免的结果类型分两种：目标为 `any` → 结果 `any`（不是 `any?`——运行时形状未知，包一层可空毫无信息量）；目标具体、仅下标为 `any` → 豁免下标检查，结果仍按 7.5 的表取 `T?` / `V?`（`{string: int}` 配 `any` 下标 → `int?`）。

### 7.5 null 策略

隐式可空来源，checker 如实建模：

| 表达式                             | 类型   |
| ---------------------------------- | ------ |
| `array[i]`（`array: [T]`）         | `T?`   |
| `hash[k]`（`hash: {K: V}`）        | `V?`   |
| 无 `else` 的 `if` 取值             | `T?`   |
| `first(a)` / `last(a)`（`a: [T]`） | `T?`   |
| `rest(a)`                          | `[T]?` |
| `puts(...)`                        | `null` |

**Null-stripping（v1 唯一模式，对 null 乐观）**：在 assignability（7.2 规则 4）、运算符（7.4）、调用实参、索引与属性访问检查之前，先从操作数的 union 中剥离 `Null` 成员再判定，因此 `int? + 1` 不报错。这是一条**有意的 unsound 规则**：

```monkey
let xs: [int] = [];
first(xs) + 1; // 静态通过；运行时 null + 1 在所有后端都是 type error
```

选择它的原因：语言目前没有 `null` 字面量、没有判空手段、没有 narrowing，严格处理会淹没所有惯用代码（`first(arr) + 1`）。可空信息仍完整保留在类型表示与诊断展示中（`int?` 不坍缩为 `int`），只是不作为报错依据。`strictNull` 作为选项占位，与 `null` 字面量、narrowing 一起列入延后（见 15.1）。

### 7.6 builtin 签名

builtin 使用受限的内部泛型（单类型变量 `T`，调用点实例化），签名以 `object/builtins.rs` 实测行为为准：

```text
len   : (string | [T]) -> int
puts  : (...any) -> null        // print 为同一 BuiltinId 的别名，签名相同
first : ([T]) -> T?
last  : ([T]) -> T?
rest  : ([T]) -> [T]?           // 空数组返回 null，不是 []
push  : ([T], T) -> [T]
```

arity 静态检查（`puts`/`print` 变长豁免），与 linter 的 `builtin-arity` 规则一致；后续该 lint 规则可由 checker 结果替代。

**泛型实例化**：收集 `T` 在全部约束位点上的实参类型，`T = join(所有约束)`，再回代校验非泛型结构。因此 `push([1], "a")` 合法：`T = int | string`，结果 `[int | string]`——对齐运行时的异构数组语义（`builtins.rs` 的 `push` 不检查元素类型）；`push(1, "a")` 报 `type-mismatch`（第一参不是数组）。`len` 的实参须为 `string`、数组或 `any`。

`any` 命中含类型变量的结构约束时，该结构内无法从运行时形状观察到的类型变量约束为 `any`；调用结束后仍未约束的内部类型变量也默认实例化为 `any`。因此 `first(x)` / `last(x)` 在 `x: any` 时返回 `any`（`Union(Any, Null)` 按 7.1 坍缩为 `Any`），`rest(x)` 返回 `[any]?`（`T` 实例化为 `any`，非泛型的数组外壳与可空外壳都保留），`push(x, 1)` 在 `x: any` 时令 `T = join(any, int) = any`，结果为 `[any]`，不能仅从第二参推成 `[int]`。这一规则只负责泛型变量实例化；非泛型外壳仍按统一 `any` 豁免通过。

### 7.7 函数、闭包与递归

- 函数字面量：参数取标注（缺省 `any`）；返回类型取标注，否则从 body 按 **completion 模型**推导。block 自底向上归纳 `{ returnTypes, fallthrough }`：`return e;` 将 `e` 的类型记入 returnTypes，其后同 block 语句**不可达**、不参与推导；`if` 语句两分支都必 return 时其后同样不可达；可 fallthrough 的 block 值 = 尾表达式语句的类型（尾语句非表达式语句时为 `null`；尾 `if` 无 `else` 时并入 `null`）。函数返回类型 = join(全部可达 `return` 的类型，可达的 fallthrough 类型)。因此 `fn(): int { return 1; "s"; }` 推导为 `int`（尾串不可达，不误报 `int | string`）；`fn(flag: bool): int { if (flag) { return 1; } "s"; }` 推导为 `int | string`（fallthrough 可达，对标注 `int` 正确报 `type-mismatch`）。guard 写法 `fn(flag: bool): int { if (flag) { return 1; } }` 推导为 `int?`：只可能 fallthrough 的隐式 `null` 并入 join 而不是单独与标注比对，依 7.5 的乐观 null 策略对标注 `int` 放行；`fn(): int { }`（body 无任何 `return`）没有可并入的返回路径，仍报 `type-mismatch`。Monkey 无循环，结构归纳一遍即定，无需不动点。
- 调用：callee 静态类型必须是 `Fn`、`Any` 或 bound method；arity 严格相等；实参逐个 assignable。callee 为 `Any` 时实参不检查。callee 为 union 时按 7.1 的消解规则：全体成员可调用、arity 一致、实参对每个成员都合法，返回类型取各成员返回的 join。
- 闭包捕获自由变量在定义处的**绑定 identity**；同一作用域后续同名 `let` 创建新绑定，不改写旧闭包。interpreter 通过递归快照声明时的环境 frame（其中的运行时值仍共享）实现这一语义，compiler/VM 通过不同 local/free symbol slot 实现。两条执行路径由 re-`let` 回归锁定，避免出现 checker 仍按旧类型检查、interpreter 却读到新类型值的跨后端分歧。
- **递归**：`let f = fn(...)` 的命名回填使 `f` 在 body 内可见。若返回类型已标注，body 内 `f` 具有完整类型；未标注时 body 内自调用的返回类型按 `any` 处理（不做不动点迭代），推导继续。即：未标注的递归函数得到 `fn(...): any`，标注返回类型即可获得精确检查。fibonacci 例子未标注时全程 `any` 静默，标注 `: int` 后获得完整校验。

### 7.8 class

两遍检查（class 仅存在于顶层，parser 已保证）。两遍都以 `ClassId` 为 property map 的 key，不能以 class 名字符串为 key。进入 pass 1 前先为每个 `ClassDeclaration` AST 节点分配并记录唯一 `ClassId`；pass 1 按源码顺序遇到声明时绑定该节点已记录的 identity，再处理其 methods。pass 2 重放同一套绑定顺序并复用各节点已有的 `ClassId`，绝不重新分配 identity。因此同名 class shadowing 与 `let` 保存的 class alias 在两遍中解析一致：

**Pass 1 —— 收集**。对每个 class 建立 property map：

- 方法签名：参数类型取标注（缺省 `any`）；返回类型**只取标注**，未标注记为 `any`。pass 2 检查某方法 body 时仍对其内部 `return`/尾表达式做局部推导并与标注核对，但**其他方法**看到的未标注返回类型始终是 `any`——不做跨方法不动点迭代，与 7.7 的递归规则同一取向。因此 `this.value = this.make();` 在 `make` 未标注时把字段并入 `any`，给 `make(): int` 补标注即可恢复精确；依赖图推导列入延后（见 15.1）。
- 字段集合：扫描**所有**方法体（不只 constructor）中 receiver 为 lexical `this` 或其简单 alias 的 `.x = expr;` 赋值目标，字段类型 = 各赋值 RHS 类型的 join。alias 信息随作用域中的 binding 保存：`this` 初始为 alias；`let self = this`、`let other = self` 继续传播；re-`let` 以新 RHS 覆盖 alias 标记；参数与其他表达式默认不是 alias；嵌套 `fn` 可按普通 lexical capture 读取外层 alias。只追踪 RHS 恰为 `this` 或 alias identifier（分组不改变 AST），不追踪经 array/hash、函数调用、属性等复合值传播的 identity，也不把“类型恰为 `Instance(C)`”等同于 `this` alias。赋值目标与方法名同名的**不入字段集合**（该赋值点在 pass 2 报 `assign-to-method`——若入了集合，写入检查会先命中字段，该诊断永不触发）。RHS 推导中对 `this`/alias 的 `<receiver>.<field>` 读取一律按 `any` 处理，不做字段间依赖求解（`this.y = this.x;` 得 `y: any`，`x` 仍由其字面赋值得 `int`），结果与方法遍历顺序无关；依赖图不动点列入延后（见 15.1）。**v1 不建模初始化状态**：读取"可能尚未赋值"的字段在运行时抛 MissingProperty（四后端一致），而不是得到 `null`，所以把这类字段提升为 `T?` 是错误建模。checker 只做名字级检查——`prop` 在 property map 中即通过，不判断读取点之前是否已赋值。例如 `Node` 只在 `connect` 里写 `this.next`，`read` 里读 `this.next` 静态通过；先 `read` 后 `connect` 的调用顺序在运行时抛 MissingProperty，checker 不捕获（documented unsound）。definite-assignment 分析列入延后（见 15.1）。

**Pass 2 —— 检查**：

- `new C(args)`：callee 静态类型为 `Class(classId, C)` 时按该 identity 的 constructor 签名检查 arity 与实参（缺省 constructor 即零参）；为 `Any` 时不检查；为 union 时按 7.1 消解（与 7.7 的调用规则同构）：剥离 `null` 后全体成员须为 class、constructor arity 一致、实参对每个成员都合法，结果为各成员 `Instance` 的 join，任一成员不是 class 则报 `not-constructable`。class 作为值传递（`let Type = Point;`）会完整保留 `ClassId`，`new Type(...)` 照常获得相同 identity 的 `Instance`。
- `expr.prop` 读取：receiver 为 `Instance(classId, C)` 时，在该 identity 的 property map 中让 `prop` 依"字段优先于方法"的运行时顺序解析；字段命中得字段类型，方法命中得绑定后的 `Fn` 类型；两者皆无 → `unknown-property` 诊断（对应运行时 MissingProperty）。receiver 为 `Any` → 结果 `Any`。receiver 为其他具体类型 → 诊断（运行时必错）。receiver 为 union → 按 7.1 消解（每个成员分别解析该属性，结果取 join）。
- `expr.prop = value;` 写入：receiver 同上。`prop` **先查方法集合**：命中方法名 → `assign-to-method` 诊断（pass 1 的字段集合已排除方法名，保证此分支可达）；再查字段：命中 → 校验 assignable。运行时语义是字段只 shadow **该实例**的方法（`set_property` 直接写实例 fields），class 级 property map 表示不了单实例 shadow：把 map 中该项改成 join 会污染其他实例的类型，不改又漏报被 shadow 实例后续的 `a.value()`。v1 直接拒绝（这几乎总是失误）；确需 shadow 把 receiver 标 `any`，实例级精化列入延后（见 15.1）。**不在 map 中 → `unknown-property` 诊断**。运行时允许外部创建新字段，但这几乎总是拼写错误；确需动态字段时把 receiver 标为 `any`。
- `this` 的类型：方法/constructor 及其嵌套 `fn` 内为携带当前 `ClassId` 的 `Instance`（lexical capture 与运行时一致）；简单 alias 保留同一类型与上文的 alias 标记。
- class/instance/bound method 不可作为 hash key：`{K: V}` 的 `K` 与 hash 字面量的 key 静态类型限定 `int | bool | string | any`，违者 `invalid-hash-key` 诊断（前置的是各后端 "key ... is not hashable" 的 runtime error——四个入口在构建/索引前都有 hashability 检查，`object.rs` 的 `Hash` panic 只是其后的内部兜底）。

### 7.9 语句

| 语句            | 规则                                                                        |
| --------------- | --------------------------------------------------------------------------- |
| `let`           | 见 7.3                                                                      |
| `return`        | 参与所在函数返回类型推导/校验；constructor 内的 return 已被 validation 拒绝 |
| `class`         | 见 7.8                                                                      |
| `obj.prop = v;` | 见 7.8；语句自身无类型                                                      |
| `debugger;`     | 恒合法，无类型效果                                                          |
| 表达式语句      | 推导即可，结果丢弃                                                          |

## 8. Checker 架构

### 8.1 包结构

```text
packages/monkey-typechecker/
  src/index.ts        // browser 入口：initWasm + checkSource（对齐 linter 的入口模式）
  src/node.ts         // Node 入口：直接实例化 wasm
  src/check.ts        // Program 遍历、两遍 class 收集、诊断聚合
  src/infer.ts        // 表达式推导
  src/types.ts        // Type 表示、assignable、join、归一化、display
  src/env.ts          // 作用域链（对齐 validation.rs 语义）
  src/builtins.ts     // builtin 签名表
  src/diagnostics.ts  // Diagnostic 结构与 code 常量
  test/
```

### 8.2 API

```ts
export interface TypeDiagnostic {
  code: string // kebab-case，如 "type-mismatch"
  message: string
  span?: { start: number; end: number } // UTF-8 byte offset，与 AST span 同制；个别失败无 span
  severity: 'error' | 'warning'
}

export interface CheckOptions {
  // v1 为空；strictNull 等未来选项在此扩展
}

export interface CheckResult {
  diagnostics: TypeDiagnostic[] // 空数组即通过
}

// 双入口共享同一实现：包根供 browser/bundler（wasm 由宿主打包），
// `./node` 在运行时经 WebAssembly API 同步实例化同一份 wasm。
export function check(source: string, options?: CheckOptions): CheckResult

// 进阶入口：
// checkWithAnalyzer(analyze, source, options) 注入自备的 analyze_lossless 绑定；
// checkProgram(program, options) 跳过 analyze，输入必须已过 parse + validation，
// 对未验证的树行为未定义。
```

`check` 内部调用 `analyze_lossless`：parse / validation 失败**折叠为单条诊断**（code 为 `parse-error` / `validation-error`，message 原样透传，span 若有则保留）并停止检查——单一 envelope 让 playground / 编辑器只处理一种结果形态，不必分支。两者干净时才运行类型检查，因此 checker 全程可假设：identifier 必然可解析、`this` 位置合法、class 无重复成员。

### 8.3 作用域语义对齐

`env.ts` 必须复刻 `validation.rs` 的源码顺序规则，避免与 Rust 侧判定分叉：

- `let` 的 RHS 在绑定进入 scope **之前**检查（`let x = x + 1;` 里右侧 `x` 指向外层/前一个绑定）；
- 命名函数在自身 body 内可见（自递归）；
- class 名先于自身 methods 进入 scope；
- forward global 引用非法（validation 已拒绝，checker 不会遇到）。

### 8.4 AST 类型来源

checker 不再手写第四份 AST mirror。AST 节点定义随 wasm 包本体发布：`wasm/src/ast_types.d.ts` 经 `typescript_custom_section` 追加进生成的 `monkey_wasm.d.ts`，linter / minifier / prettier-plugin / checker 都从 `@gengjiawen/monkey-wasm` 做 `import type`，类型与 parser 构建天然同版本。

## 9. 工具链与生态影响

AST JSON shape 变更（`TypeAnnotation` 五种节点、`Param`、`Let.identifier` 形状、两个新 `Option` 字段）触及所有 JS 消费方。`debugger` 语句（#306）刚完整走过一遍这条同步链，各包的改动位置可直接参照该 PR。

| 消费方                   | 需要的改动                                                                                                                                                                                                                   |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `wasm`                   | 新增 `src/ast_types.d.ts`，经 `typescript_custom_section` 并入生成的 d.ts，是新节点定义的唯一权威；`wasm-pack build` 重建 `wasm/pkg`（AGENTS.md：playground 消费的是 pkg 而非 Rust source）                                  |
| `monkey-linter`          | walker/scope 安全跳过 `TypeAnnotation` 子树；不新增规则                                                                                                                                                                      |
| `monkey-minifier`        | **剥离**全部标注（对体积是纯收益；printer 不需要学会打印类型）；differential test 保证剥离前后运行结果一致（语料排除函数值的 source-reflective 渲染/比较，同 6.1 边界——剥离标注本身就会改变 interpreter 对函数值的 Display） |
| `prettier-plugin-monkey` | printer 打印标注：`let x: int = 5;`（`:` 后一空格）、`fn(a: int): int`、长类型跟随现有参数换行组；format 两次幂等、格式化结果可再 parse                                                                                      |
| `vscode-extension`       | TextMate grammar 高亮 type position；诊断接入见 Phase 5                                                                                                                                                                      |
| `playground`             | AST 树视图为通用 JSON 渲染，自动生效；类型诊断面板见 Phase 5                                                                                                                                                                 |

## 10. 诊断与错误语义

诊断 code 固定为 kebab-case，v1 集合：

| code                 | 触发                                                                               | 示例信息                                                                                                                |
| -------------------- | ---------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `type-mismatch`      | assignable 失败（let 标注、实参、字段写入、返回值）                                | `type 'string' is not assignable to type 'int'`                                                                         |
| `operator-type`      | 运算符操作数不满足 7.4                                                             | `operator '+' expects 'int + int' or 'string + string', got 'int + string'`                                             |
| `mixed-equality`     | `==`/`!=` 两侧类别不同（见 7.4 equality 矩阵）                                     | `comparing 'int' with 'string' diverges across backends; GcVM raises a runtime error`                                   |
| `invalid-comparison` | `==`/`!=` 操作数为 Array/Hash/Fn（GcVM 运行时报错，其余后端行为各异）              | `values of type '[int]' cannot be compared; GcVM raises a runtime error`                                                |
| `arity-mismatch`     | 调用/`new` 参数个数不符                                                            | `Point constructor expects 2 arguments, got 3`                                                                          |
| `not-callable`       | callee 静态类型不可调用                                                            | `type 'int' is not callable`                                                                                            |
| `not-constructable`  | `new` 的 callee 不是 class                                                         | `cannot construct 'fn(int): int'`                                                                                       |
| `unknown-property`   | 实例属性读写未命中 property map                                                    | `property 'nmae' does not exist on 'Point'`                                                                             |
| `assign-to-method`   | 对实例的方法名赋值（字段只 shadow 单实例，class 级类型无法表示）                   | `assigning to method 'value' shadows it only on this instance of 'Counter'; annotate the receiver as 'any' if intended` |
| `unknown-type-name`  | 标注引用了不存在的类型名                                                           | `unknown type 'Pointt'`                                                                                                 |
| `reserved-type-name` | class 名与 builtin 类型名同名（warning：运行时合法，但其实例类型无法在标注中引用） | `class 'int' shadows a builtin type name; annotations cannot refer to it`                                               |
| `invalid-hash-key`   | hash key 类型不可哈希                                                              | `type '[int]' cannot be used as a hash key`                                                                             |
| `invalid-index`      | 索引目标/下标类型错误                                                              | `type 'string' is not indexable`                                                                                        |
| `parse-error`        | 源码未通过 parse（8.2 的折叠规则，message 来自 parser）                            | `expected a type, got: start: 7, end: 8, kind: =`                                                                       |
| `validation-error`   | 源码未通过 validation（同上，message 来自 validation.rs）                          | `undefined variable 'nope'`                                                                                             |

信息风格对齐现有 runtime error 词汇（`is not assignable`、`wrong number of arguments` 等），使同一问题的静态诊断与运行时报错可以相互印证。code 集合视为半稳定 API：playground/vscode 按 code 分类展示，新增只追加。

## 11. 兼容性

- **源码级**：无破坏。标注可选 + 软关键字，存量程序全部照常 parse、运行时行为不变。但 AST **形状**对无标注程序同样变化（`Param` 替换 `Vec<IDENTIFIER>`、`Let.identifier` 重构、新增 `null` 字段），见下一条。
- **bytecode**：不新增 opcode，不改编码。快照体系（`.mbc`）不受影响。
- **AST JSON**：形状变更，是 `wasm` 与各 JS 包之间的内部契约，需同一批版本联动升级；已发布到 npm 的包（linter、minifier 等）按各自发版流程 bump。
- **Rust API**：`FunctionDeclaration.params` / `MethodDefinition.params`（`Vec<IDENTIFIER>` → `Vec<Param>`）、`Let.identifier`、`Object::Function` 的公开字段形状同批 breaking。这些 crate 由 release workflow 的 `cargo workspaces publish` 发布在 crates.io（README 挂着 monkey-interpreter 的 crates.io badge），对下游是 semver breaking：相应 PR 须按 conventional commits 标注 `!`/`BREAKING CHANGE`，由 release-please 提升主版本，不能只当 workspace 内部联动。
- **Span 制式**：诊断沿用 UTF-8 byte offset，UTF-16 转换继续留在 TS 消费端（`sourceSpan.ts`），与既有 lint/validation 诊断一致。
- **REPL / CLI**：不接入 checker，行为不变。

## 12. 测试与验收

### 12.1 Lexer / Parser

- `?` token 与精确 span；软关键字仍是 IDENTIFIER（`let int = 5;`、`let any = fn(any) { any; };` 均合法）。
- 各类型形态的 AST snapshot + source-slice span 断言：`int?`、`[int]`、`{string: [int]}`、`fn(fn(int): int, int): [string]`、`(fn(int): int)?`。
- negative：`let x: = 5`、`fn(a:) {}`、`fn(int)` 作类型缺返回、constructor 带返回标注、`let x: {int} = ...`。

### 12.2 擦除恒等（核心验收）

- 新增 compiler 测试：同一程序带满标注与完全去标注两个版本，断言 instructions **逐字节相同**、constants 相同、strip-debug 产物相同。debug info 不做互相恒等断言（span 偏移必然不同），改为分别断言两版的 `PcSpan` 精确映射回**各自**源码。
- 现有 compiler/VM/GcVM/asm 全部快照必须零变化（parser 快照除外）。
- e2e：带标注程序在四个后端与去标注版输出一致（含 error 场景）；语料不打印、不比较任何包含函数值的值（含嵌套于 array/hash）。6.1 的声明例外配专项测试固定：`puts(fn(x: int) { x })` 按后端分别断言——interpreter 输出含标注的源码渲染，VM/GcVM 输出 `[closure function]`，asm 输出 `[function]`。

### 12.3 Checker

- 每条 7.x 规则的正反用例；重点回归：`examples/hello.monkey` 原样通过且**零诊断**（异构 hash 经 union + any 参数不触发误报）。
- 递归：fibonacci 未标注静默、标注 `: int` 后对 `return "a"` 报 `type-mismatch`。
- class：字段收集覆盖全部方法体（constructor 外赋值同样入 map，无 `T?` 提升）与 `this` 的简单/传递 alias（`let self = this; let other = self; other.x = 1;`）、未标注方法的跨方法返回降级 `any`（`this.value = this.make();`）、对方法名赋值报 `assign-to-method`（含方法体内 `this.<方法名> = ...`，验证字段收集的排除规则未吞掉该诊断）、字段间依赖降级（`this.y = this.x;` 得 `any`）、`unknown-property` 拼写捕获、`new` alias、同名 class shadowing 保留不同 `ClassId`（旧 class alias 的实例不能赋给新 class 标注）、`class int {}` 报 `reserved-type-name`。
- equality：`xs == xs`（`xs: [int]`）与 `f == f`（`f: fn(): int`）报 `invalid-comparison`，配 GcVM 运行时报错的对照用例；跨类别报 `mixed-equality`；`new A() == new B()` 零诊断（四后端合法、恒 `false`）。
- union 消解：`let f = if (c) { fn(x: int): int { x; } } else { fn(x: string): string { x; } }; f(1);` 报错；`let xs = if (c) { [1] } else { ["a"] }; xs[0];` 通过且类型为 `(int | string)?`。两例都由分支推导内部 union，不依赖 v1 尚未支持的 union 用户语法。
- any 与 builtin 泛型：`any + true`、`any + [1]` 通过且结果为 `any`；`let c = true; let x: any = 0; let y = if (c) { 1 } else { "s" }; x + y;` 按 RHS 的内部 union 成员检查，结果为 `int | string`；`first(x)`（`x: any`）结果为 `any`，`push(x, 1)` 结果为 `[any]`。
- 返回推导 completion：`return` 后不可达语句不参与 join（`fn(): int { return 1; "s"; }` 零诊断）；条件 return 与 fallthrough 合并（`fn(flag: bool): int { if (flag) { return 1; } "s"; }` 报 `type-mismatch`）。
- **对拍（oracle）测试**：套用 minifier 的 differential test 模式，但正向断言只对 **sound 子集语料**成立——全量标注、接口处无 `any`、不依赖 null-stripping（不对可空值直接运算）、不读取可能未初始化的字段。TypeScript oracle 使用 WASM 暴露的 GcVM runner，断言该语料上 checker 零 error ⇒ GcVM 不出现 type/arity/property 类 runtime error；tree-walking interpreter 由 Rust 侧同语义回归覆盖，特别锁定闭包捕获后 re-`let` 的 binding identity。gradual 语料（含 `any` 边界、null-stripping、字段初始化盲区）单独归类，只做反向断言：checker 报 error 的用例抽样验证运行时确实可触发对应错误。

### 12.4 工具链

- prettier：标注 golden format、两次幂等、format 结果再 parse 后 AST 等价、标注内注释不丢失。
- minifier：剥离后再运行的 differential 对比（语料同 6.1 边界，排除含函数值的渲染/比较）；输出不含任何标注。
- linter：带标注源码上现有 9 条规则行为不变。
- wasm contract test：新 AST 节点 JSON round-trip；`wasm-pack test --node`。

## 13. 实施顺序

沿用仓库单一关注点小 PR 的节奏；PR 标题用英文：

**Phase 0 — 本设计文档**：`docs: add type system design doc`（即本文）。

**Phase 1 — 共享 AST 类型**：`refactor(wasm): ship the AST typings with the wasm package`。把三份手写 `types.ts` 收敛为 `wasm/src/ast_types.d.ts`，随 `@gengjiawen/monkey-wasm` 生成的 d.ts 发布，linter/minifier/prettier 改为 `import type`，零行为变化。

**Phase 2 — Rust 语法**：

- 2a `feat(parser): parse type annotations on let bindings` —— `?` token、`TypeAnnotation` 节点族、新增 `parser/type_parser.rs`（在同一 `Parser` 上实现 `parse_type*`，不创建第二套 parser）、`Let` 变更（含 `identifier` 重构为 `IDENTIFIER`）、Display、快照。
- 2b `feat(parser): parse parameter and return type annotations` —— `Param`、`return_type`、`Object::Function` 及 interpreter/compiler/asm/validation 机械适配、**擦除恒等测试**（12.2）。

**Phase 3 — wasm 与工具链同步**：

- `chore(wasm): rebuild with type annotation AST`（含 `ast_types.d.ts` 更新）；
- `feat(minifier): strip type annotations`；
- `feat(prettier-plugin): print type annotations`；
- `feat(vscode): highlight type annotations`（grammar）。

**Phase 4 — checker 包**（4a–4c 可与 Phase 2/3 并行开发，先按全 `any` 推导实现，标注读取在 Phase 3 后接通）：

- 4a `feat: scaffold monkey-typechecker with core inference` —— 包结构、Type/env/join/assignable、字面量与运算符、let/if/index、诊断输出；
- 4b `feat(typechecker): functions, calls and builtin signatures` —— 函数类型、arity、闭包、递归规则、builtin 泛型签名；
- 4c `feat(typechecker): class types` —— 两遍收集、property map、`new`/property 检查、hash key 检查；
- 4d `feat(typechecker): consume source annotations` —— 接通标注、`unknown-type-name`、对拍测试。

**Phase 5 — 展示面**：

- `feat(playground): show type diagnostics`（对齐 lint 面板模式，`sourceSpan.ts` 转坐标）；
- `feat(vscode): report type diagnostics with real spans`（顺带替换现在硬编码 `Range(0,0)` 的报错路径）。

依赖：P0 → P2 → P3 → P4d → P5；P1 在 P3 前完成即可；P4a–4c 可并行。合计约 11–13 个 PR。

## 14. 文件改动索引

| 层                           | 主要文件                                                                                                                                                |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| lexer                        | `lexer/token.rs`（QUESTION）、`lexer/lib.rs`、lexer tests/snapshots                                                                                     |
| AST/parser                   | `parser/ast.rs`、`parser/lib.rs`、新增 `parser/type_parser.rs`（同一 `Parser` 的类型子文法）、parser tests/snapshots；`parser/validation.rs` 仅机械适配 |
| shared object                | `object/object.rs`（`Object::Function` 的 params 形状）                                                                                                 |
| interpreter / compiler / asm | `interpreter/lib.rs`、`compiler/compiler.rs`、`compiler/symbol_table.rs`、`asm/lower.rs` 的机械适配；`compiler/vm.rs`、`gc/` 零改动                     |
| 擦除测试                     | `compiler/compiler_test.rs`（bytecode 恒等）、各后端 e2e                                                                                                |
| WASM                         | `wasm/src/ast_types.{rs,d.ts}`（typescript_custom_section）；重建 `wasm/pkg`                                                                            |
| 共享 AST types               | `wasm/src/ast_types.d.ts` 并入 wasm pkg 的 d.ts；linter/minifier/prettier 迁移 import                                                                   |
| checker                      | 新增 `packages/monkey-typechecker` 全部                                                                                                                 |
| minifier                     | `src/printer.ts`（剥离）、differential tests                                                                                                            |
| prettier                     | `src/printer.ts`、fixtures/tests                                                                                                                        |
| VS Code                      | `syntaxes/monkey.tmLanguage.json`；Phase 5 的 `src/extension.ts`                                                                                        |
| playground                   | Phase 5 的诊断面板、`src/lint.ts` 同款接入                                                                                                              |
| docs                         | 本文；README 语言特性一节                                                                                                                               |

## 15. 延后能力与备选方案

### 15.1 后续演进（按建议顺序）

1. `null` 字面量 + 判空（`x == null` 或 `isNull`）+ if 分支 narrowing + `strictNull` 选项——三者必须一起落地才可用，落地后可收紧 7.5 的 null-stripping；
2. 字段 definite-assignment 分析：追踪 constructor（含条件分支内赋值）与方法调用顺序下的初始化状态，把 7.8 的 MissingProperty 盲区转为静态诊断；
3. 跨方法返回类型与字段间依赖推导：按调用/字段依赖图做 SCC 不动点，消除 7.8 的两处 `any` 降级（未标注方法的跨方法返回、`this.y = this.x;` 的字段读取）；
4. 实例级类型精化：alias-aware 的流分析，精确建模"字段只 shadow 单个实例的方法"，放宽 7.8 的 `assign-to-method`；
5. union 用户语法 `A | B`（内部表示已就绪，只差 parser）；
6. 字符串字面量 key 的 hash 推导为 record 形状（捕获 `person["nmae"]` 拼写错）；
7. class 字段声明语法与字段标注；
8. 类型别名；
9. linter 消费 checker 结果的 type-aware 规则（`no-unused-expression` 白名单扩展等，见 linter-plan）;
10. 基于类型的后端优化：asm 对已证明 `int` 的运算去 SMI tag、GcVM 标量 unboxing——需要先把 checker 结果回传 Rust，属独立提案。

### 15.2 为什么 checker 在 TS 而不是 Rust

- 消费方（playground、vscode、linter）全在 JS 生态，TS 实现零胶水直达；Rust 实现则要经 wasm 导出诊断、编译周期也更重。
- linter/minifier 已验证"wasm AST + TS 分析"架构，walker/scope/差分测试模式全部现成。
- 擦除语义下 Rust 执行路径不需要类型信息；将来若做 15.1-7 的优化，可再评估把稳定后的规则移植/下沉，届时 TS 版本就是可对拍的参考实现。

### 15.3 为什么返回类型用 `:` 而非 `->`

`->` 需要新增 token 与 lexer 改动，且与仓库"JS 风格表面语法"的既定方向（class 提案 20.2 同一逻辑）不符。TS 风格 `fn(a: int): int` 与参数标注同构，用户只需学一个符号。

### 15.4 为什么擦除而非运行时强制

运行时强制要求四个后端同步实现断言语义，直接违背"三个 runtime 语义一致"的维护原则中最昂贵的一条；且 gradual 系统里 `any` 边界的运行时检查语义（cast 插入）复杂度极高。擦除把一致性面缩减为零，也让 checker 可以独立演进。

### 15.5 为什么软关键字而非保留字

保留 `int`/`string` 等会破坏存量程序与"标注可选"的承诺（不写标注的用户也被迫避开这些名字）。type position 由 `:` 前缀唯一标定，软关键字无歧义成本。

### 15.6 为什么不做完整 Hindley–Milner 推导

HM 的收益在于跨函数全量推导，但 Monkey 的函数是一等值且大量作为参数传递，HM 需要 let-多态与 occurs check，实现与报错质量成本都高。gradual + 局部推导（7.7 的递归规则即唯一妥协点）能以小得多的实现覆盖教学语言的主要价值：标注即文档、拼写与类型错误提前暴露。且 `any` 默认与 HM 的全量推导哲学冲突——二者只能选一。

## 16. 完成定义

本提案的类型系统完成，以以下闭环为准：

- 标注语法在 lexer/parser/AST/JSON 全链路可 parse、可打印、span 精确；
- 擦除恒等测试（12.2）在四个后端全部通过，存量快照零变化；
- `examples/hello.monkey` 原样零诊断；对拍测试建立并通过；
- checker 覆盖 7.x 全部规则，诊断带 span 且一次输出多条；
- prettier round-trip、minifier 剥离差分、linter 回归、wasm contract tests 通过；
- playground 与 VS Code 展示类型诊断，位置精确到 span；
- README 与本文档状态行更新为 Implemented。
