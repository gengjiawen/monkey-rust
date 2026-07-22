# Monkey Minifier — 设计与实现

本仓库为 Monkey 方言实现了一个 TypeScript source-to-source minifier，作为
workspace package 提供，并准备发布为 npm package；playground 已提供在线 demo。
本文记录当前实现、correctness boundaries 和仍未完成的 release follow-up；Phase
编号只表示功能演进顺序，不再表示多个待提交的 PR。

处理管线：

```
source ──wasm parse_lossless──▶ JSON AST ──optimization passes──▶ AST ──compact printer──▶ minified source
                                         mangling / constant folding /
                                         constant propagation / DCE
```

Parser 不重新实现。minifier 消费 `@gengjiawen/monkey-wasm` 新增的
`parse_lossless()` 输出；它和现有 `parse()` 来自同一个 Rust AST，但把 i64 整数
序列化成十进制字符串，避免经过 JavaScript `number` 时丢精度。prettier 插件可以
继续使用原 API，minifier 不直接消费当前的 `parse()`。

## 目标

- 正确性优先，并把检查拆成两个可判定的 invariant：
  - v0：`canonical(reparse(print(ast))) === canonical(ast)`，其中 `ast` 来自
    `parse_lossless(source)`。
  - 有 optimization pass 时：打印结果重新 parse 后与 pass 产出的 canonical AST
    结构相等；runtime equivalence 由**全新 `Compiler` + GC VM** 判定：success
    比较 `status/result/stdout`，failure 比较 `status/stage/kind/stdout`。
    Interpreter 与 compiler 在 `rebinding + closure` 等场景下的 semantics 已有差异，
    因此不作为 optimization oracle。
- v0 做空白/注释剥离和冗余括号消除；v1 做 identifier mangling；v2 做
  constant folding、constant propagation 和 dead `let` elimination（DCE）。
- 发布为 npm 包，API 小而稳定：`minify(source, options)`。
- Playground demo：提供 `Minify` 输出视图，实时展示 minified output 和 byte
  statistics。

## 非目标（暂时）

- Source map（AST 里有 span，以后想做随时能做）。
- Property/method mangling（`obj.prop` 保持原样——和 terser 默认关闭 `mangleProps`
  同一个原因，不安全）。
- 面向 gzip 的启发式优化。
- 一切格式化相关的事——那是 `prettier-plugin-monkey` 的职责。

## 包布局

新包 `packages/monkey-minifier`（`packages/*` 通配自动进 workspace）。npm 包名用
`@gengjiawen/monkey-minifier`——和 `@gengjiawen/monkey-wasm` 一样带 scope，避免
抢注问题。构建使用 `tsc` 生成 Node entry/types，再用 esbuild 生成 browser ESM
bundle；测试使用 Vitest，并另有 Node/browser package smoke tests。

```
packages/monkey-minifier/
  package.json          # 依赖: @gengjiawen/monkey-wasm；build: tsc + esbuild
  tsconfig.json
  src/
    index.ts            # 浏览器/bundler 入口
    node.ts             # Node 入口，显式实例化 Wasm
    core.ts             # minify() 共享管线、选项处理
    cli.ts              # monkey-minify 命令行入口
    types.ts            # AST 类型（复制而来，见下）
    printer.ts          # compact printer（Phase 1）
    scope.ts            # scope analysis（Phase 3）
    mangle.ts           # identifier mangling（Phase 3）
    fold.ts             # constant folding / DCE（Phase 4）
    propagate.ts        # constant propagation（Phase 4）
  test/
    corpus/
    printer.test.ts
    roundtrip.test.ts
    differential.test.ts
    mangle.test.ts
    fold.test.ts
    node-entry.test.ts
    package.test.ts
    browser-smoke.mjs
    node-smoke.cjs
```

### Lossless AST 传输与类型

parser/wasm 提供工具专用的 lossless AST serializer 和 `parse_lossless()` 导出。
这是 minifier 的 correctness prerequisite：Rust AST 中的 `Integer.raw` 是 i64，
而现有 TS 镜像写成 `number`；例如
`9007199254740993` 经 `JSON.parse` 会变成 `9007199254740992`，原 AST 和
round-trip AST 两边一起失真时甚至会假通过。

lossless JSON 只把 `Integer.raw` 投影为十进制字符串，其余 AST node shapes 保持
不变。现有 `parse()` 的返回格式不变，以免破坏 prettier/playground 消费方。minifier
的 `src/types.ts` 相应使用：

```ts
interface IntegerLiteral extends ASTNode {
  type: 'Integer'
  raw: string
}
```

printer 原样输出已验证的十进制串，numeric passes 用 `BigInt(raw)` 运算，任何时候
都不把 Monkey 整数放进 JS `number`。parser、Wasm 和 minifier tests 直接覆盖
`2^53 + 1`、`i64::MAX` 以及 nested array/hash 中的整数，不通过 JS `number`
中转。

### AST 类型：先复制，不共享

`src/types.ts` 起步时直接复制
`packages/prettier-plugin-monkey/src/types.ts` 里的 AST node types。从 prettier 插件
import 会把它的 `prettier` peerDependency 拖进 minifier 的依赖图，方向也是反的。
这些类型是 serde JSON 输出的手写镜像，变动频率很低，两份拷贝可以接受。

如果以后开始难受，按顺序有两条退路：

1. 抽一个 `packages/monkey-ast` 纯类型包，两边共同依赖。
2. 用 `ts-rs` 从 Rust 的 AST 结构体直接生成 TS 类型，让 `parser/ast.rs` 成为
   唯一事实来源。最符合本仓库 Rust-first 的架构，但 v0 不值得提前做。

### 公共 API

```ts
interface MinifyOptions {
  mangle?: boolean | { reserved?: string[] } // default: true
  fold?: boolean // controls folding, propagation and DCE; default: true
}

interface MinifyResult {
  code: string
}

function minify(source: string, options?: MinifyOptions): MinifyResult // parse 失败抛 SyntaxError
```

字节数统计不进结果结构。调用方按 UTF-8 计算，不能用 UTF-16 code unit 数量的
`source.length`：

```ts
const utf8Bytes = (text: string) => new TextEncoder().encode(text).byteLength
```

`@gengjiawen/monkey-wasm` 是 wasm-pack 的 bundler target，入口会静态 import
`.wasm`；这适合 Vite/Next.js，但 Node 不能直接执行。npm 包因此用条件明确的双
入口：`browser` 指向真正的 ESM bundle（不能把带异步 Wasm 初始化的依赖降成
CommonJS `require`），Node 主入口加载生成的 `*_bg.js` glue，再通过
`WebAssembly.Module`/`WebAssembly.Instance` 同步实例化同一份 Wasm。CLI 复用
Node 入口，从而保持 `minify()` 同步 API；Node 最低版本是 24。

## Phase 1 — Compact printer（v0）

v0 里唯一真正需要动脑的部分。三个设计点：

**不抄 Rust 的 `Display` 实现。** 它是调试辅助，不是代码生成器：infix 全括号
打印（`ast.rs:302`），`if` 条件打印时不带括号——而 parser 强制要求括号
（`parser/lib.rs:397` 的 `expect_peek(LPAREN)`），也就是说 `Display` 的输出
确定不能 re-parse。TS printer 从语法出发重写，structural round-trip test 是
正确性的裁判。

**按优先级决定括号。** 把 `parser/precedences.rs` 的表搬过来
（`Lowest < Equals < LessGreater < Sum < Product < Prefix < Postfix`）。规则：

- 只有 child expression 的 precedence 低于 parent context 时才加括号。
- 所有 infix 运算符都按左结合 parse：同优先级时只给右子树加括号
  （`a - (b - c)`），左子树永远不加。对 `+`/`*` 也统一套用这条——正确且简单，
  可证结合的场景跳过括号属于后期微优化。
- 前缀运算符的操作数：低于 `Prefix` 才加括号（所以是 `-(a + b)`，但 `-a[0]`
  不用加，index 是 `Postfix`）。

AST 不记录冗余括号（parse 时作为 grouped expression 就消掉了），所以 v0 对
括号多的源码天然就有压缩收益。

**Statement separators 与 token boundaries。** `Let`、`Return`、
`SetProperty` 和 expression statement 后补 `;`；`Class` 后**不能**补分号，
`class A {};` 会让 parser 把多出的 `;` 当成一条没有 prefix parser 的 expression
并报错。statement printer 显式决定 adjacent statements 之间是否需要 separator。

词法空格只在删掉后会把两个 token 合并时插入，例如 `let x`、`return x`、
`new Foo`、`class Foo`。输出永远是 `fn(` 和 `}else{`，这里不需要空格。另用
token-boundary fixture 固化 `a- -b`/`a--b`、keyword/identifier boundary、adjacent
statements 等情况；以 `{` 开头的 statement 无歧义地是 hash literal（这个语法没有
通用 block statement）。

**Node coverage。** Statements：`Let`、`Return`、`Class`、`SetProperty`、expression
statement。Expressions：identifier、五种 `Literal`（`Integer`、`Boolean`、`String`、
`Array`、`Hash`）、prefix、infix、`if`/`else`、`fn`、call、index、`this`、property
access、`new`。三个已从源码确认的输出形态：

- `if` 条件必须带括号：`if(c){...}else{...}`。
- 函数一律输出 `fn(params){...}`：`fn name()` 不能 parse（`parser/lib.rs:463`，
  `fn` 后紧跟 `expect_peek(LPAREN)`）。AST 里的 `FunctionDeclaration.name`
  是 `parse_let_statement` 根据 let binding 回填的 recursion metadata
  （`parser/lib.rs:121`）；compiler 在函数内部为它定义 `Function` symbol。printer
  不输出这个字段，但 transformed canonical AST 必须同步维护它。
- string literal 原样输出：lexer 的 `read_string`（`lexer/lib.rs:195`）没有任何
  转义机制，读到下一个 `"` 为止——字符串内容不可能含 `"`，反斜杠是普通字符，
  还可以跨行。printer 直接输出 `"` + 原文 + `"`，零处理。

注释根本进不了 AST，剥离零成本。

## Phase 2 — Playground demo

`packages/playground/src/App.tsx` 已接入 Minify view：

- `OutputView` 包含 `'minify'`，SegmentedControl 在 AST / Bytecode / GC /
  Snapshot / ARM64 旁边提供 **Minify** tab。
- `src/MinifyView.tsx` 使用只读 CodeMirror `Editor` 展示 minified output，并显示
  input/output UTF-8 bytes 和节省百分比。状态类型是
  `MinifyState = idle | ok | invalid`。
- 只在 view 激活时 debounce 计算。dynamic import 和计算结果都携带递增 request
  id；提交状态前再次核对最新 source 和 active view，防止慢请求覆盖新输入。
- 通过动态 `import('../../monkey-minifier/src/index')` 引入，和 Format action
  动态引入 prettier plugin 的方式一致，所以 **demo 不依赖 npm release**。
- Parse error 直接显示在 panel；toolbar 已提供 `Mangle names` switch。
- Playground 已包含 `Constant folding` snippet，展示下一节的 constant
  folding/propagation pipeline 最终把程序压缩为 `print(2);`。

`src/test/MinifyView.test.tsx` 覆盖状态和 UTF-8 byte 统计，`App.test.tsx` 覆盖 lazy
execution、stale request guard 和 mangle switch。

唯一保留的 UI follow-up 是可选的 `runs identically ✓` badge。所需的 output-aware
runner 已经存在；若实现 badge，应分别执行 original/minified program 并比较
`status/result/stdout`，不能退回不捕获 stdout 的 `run_snapshot`。

## Phase 3 — Identifier mangling（v1）

这是实际压缩收益的大头，但只对能解析到明确 binding identity 的 identifier 做
rename。以下名字保持不动，生成名也不能与它们碰撞：

- builtin：完整列表是 `len`、`puts`、`first`、`last`、`rest`、`push`、`print`
  （`object/builtins.rs`）。builtin reference 不改名；用户 `let` 可以 shadow builtin，
  该用户 binding 则是另一个 identity，可以改名。
- Property 和 method 名：`PropertyExpression.property`、
  `SetPropertyStatement.property`、`MethodDefinition.name`。Hash key 是普通表达式，
  也绝不能被当作属性名统一改写。
- class declaration 的 binding 及解析到它的 references。虽然 class 名参与普通词法
  解析，GC VM 会在 class、instance、bound method 的最终值和 `puts` 输出中显示它；
  直接把 `class LongName` 改成 `class a` 会改变 observable result。同名的后续 `let`
  rebinding 是独立 identity，不受此限制。
- `this`、用户传入的 `options.mangle.reserved`，以及所有 unresolved/external
  identifiers。unresolved name 保留原 spelling 并加入生成名禁用集合；遇到 analyzer
  无法精确建模的 unknown node 时，整个 pass fail closed。

`scope.ts` 不是只收集字符串的传统 hoist scope，而是按
`Compiler::compile_stmt` 的遍历顺序构建 binding/reference graph。Semantic
authority 是每次都从干净状态创建的 standalone compiler；不能用 interpreter
environment 反推，因为例如 rebinding 后 closure 读取的值，两套引擎目前并不一致。
需要逐条 mirror 这些规则：

1. 只有 program、function literal 和每个 method body 创建 symbol scope；`if`/`else`
   block 不创建 scope，class declaration 自身也不创建额外 scope。
2. 普通 `let` 先解析/编译 RHS，再定义一个新 slot。同名 `let` 是新的 binding，
   只遮蔽它之后的 reference；更早创建的 closure 继续 capture 旧 binding。Analyzer
   可先创建一个 pending identity，但在 RHS 结束前不能把它放进普通可见名字表。
3. 直接形如 `let f=fn(...){...}` 的 RHS 是特例：parser 回填的
   `FunctionDeclaration.name` 会在函数内部先定义 recursive self symbol；它与 pending
   `f` 绑定关联，但不让 `f` 在 RHS 的其他位置提前可见。进入函数后先定义这个
   self symbol，再按参数列表顺序定义 parameters；duplicate parameters 或与 self
   同名的 parameter 以最后定义的 slot 为 reference target。
4. class binding 在编译 methods **之前**定义，因此 methods 内的 class reference
   可以正常 resolve。method scope 先有 implicit `this`，再按顺序定义 parameter；
   nested function 按正常 closure capture 规则捕获 `this`。
5. compiler 依次编译 `if` 的 condition、consequent、alternate，两个 branch 共用
   当前 symbol table；即使运行时只走一个 branch，其中的 `let` 也会按这个编译
   顺序影响另一个 branch 和后续 references。Analyzer 必须复现这个行为，不能分别 clone
   两份词法环境再 merge。
6. builtin 是初始 symbol，用户声明可按上述源码顺序 shadow；找不到 binding 的
   identifier 保持原拼写，并加入生成名禁用集合，防止意外 capture。

`mangle.ts` 按 graph 中的 identity 改 declaration 和所有已绑定 reference，而非
按字符串全局替换。当前策略按 reference frequency 给可改 binding 分配全程序唯一
的短名（`a`–`z`，再到 `a0`…），并跳过 keyword、builtin、reserved、class 名、
unresolved name 和所有保留原名的 binding。以后可基于 interference/liveness
证明安全地在不相交 scope 复用短名。

改直接绑定 function literal 的 `let` 时，还要同步其 `FunctionDeclaration.name`
metadata。printer 仍只输出 `fn(`，重新 parse 会根据新 let 名回填同样的 metadata。
Tests 覆盖连续 rebinding、RHS 读取旧 binding、self-recursion、duplicate parameters、
builtin shadow、closure capture、两个 if branch、implicit `this`、class reference 和
unresolved identifier。

## Phase 4 — Optimization passes（v2）

每个都是自包含的 AST→AST pass，但 integration order 固定为：constant folding 与
constant propagation 交替到 fixed point → DCE 到 fixed point（每轮发生删除后重跑
scope analysis）→ mangling（使用 fresh scope analysis）→ printer。不能宣称任意
顺序：
folding 为 propagation 制造 literal initializer，propagation 为 folding 解锁卡在
identifier 上的 expression；两者共同暴露 dead binding，删除又会改变 reference
frequency 和后续 slot 编号。任一 analysis 不确定时保留原 node。

### Constant folding

整数算术、字符串拼接、布尔/比较运算、前缀运算严格镜像 production GC VM，而
不是 interpreter。整数从 lossless `raw` 用 `BigInt` 计算：

- 加、减、乘和负号按 i64 two's-complement wrap，用
  `BigInt.asIntN(64, value)`。GC VM 已显式使用
  `wrapping_add/sub/mul/neg`；minifier 只 mirror 这项既有语义，本实现不需要修改
  gc crate。
- division 使用 BigInt 的 truncation toward zero；divisor 为 0、以及
  `i64::MIN / -1` 都保留原 expression，让 VM 产生原来的 runtime error。
- AST 没有 negative integer literal。负结果必须构造成
  `UnaryExpression(-, Integer(abs))`；`i64::MIN` 的绝对值超出 lexer 可接受的正 i64，
  无法这样打印，因此放弃该次 fold，绝不生成非法 literal。
- 只有 operand type 和 VM result 都能证明时才 fold；string 只 fold `+`，比较和
  `!` 也只覆盖 GC VM 已定义的 constant type 组合。Overflow boundaries 和 error
  paths 由 unit/differential fixtures 覆盖。

下面是直接经过 `minify(source, { fold: true, mangle: false })` 的实际结果；这些
program 没有 binding，因此单独展示的是 constant folding 效果：

| Source                       | Output                  |
| ---------------------------- | ----------------------- |
| `40 + 2`                     | `42;`                   |
| `"mon" + "key"`              | `"monkey";`             |
| `if (true) { 1 } else { 2 }` | `1;`                    |
| `9223372036854775807 + 2`    | `-9223372036854775807;` |

相反，`1 / 0` 和 `(-9223372036854775807 - 1) / -1` 会保留原 expression，让 GC
VM 产生原有 runtime error。

`if` 是 expression，而它的 branch 是任意 `BlockStatement`，语言又没有可打印的
`null` literal，所以不能通用地拿 selected block 替换整个 `if`。v2 只 fold
可表示子集：

- condition 已被证明是 side-effect-free constant；
- selected branch 恰好只有一个 expression statement，可直接替换为该 expression；
- consequent 和 alternate 都不含会修改当前 compiler symbol table 的声明，否则
  删除未执行 branch 仍可能改变另一个 branch 或后续 reference 的 binding resolution；
- constant false 且没有 `else` 时保留原 `if`，因为 AST 中没有 Null literal。

例如 `let x=if(true){let y=1;y};` 不能 fold 成一个 block，必须保留。

还有一个 parser metadata boundary：`let f=fn(){...}` 会给 function body 注入
recursive self binding，而 `let f=if(true){fn(){...}}` 里的 nested function 是
anonymous。即使 condition 是 constant，也不能把后者 fold 成直接 RHS function
literal，否则打印后重新 parse 会改变 closure capture。

### Constant propagation

Folding 只处理 expression 内部全是 constant 的情况。Playground 的
`Constant folding` snippet 故意展示完整 pipeline：

```monkey
let a = 1 + 1;
let b = a + 1;
print(a)
```

处理过程是：

1. constant folding 先得到 `let a=2;`；
2. constant propagation 把 `a` 的 reference 替换为 `2`；
3. 下一轮 folding 把 `let b=2+1;` 变成 `let b=3;`；
4. DCE 删除 zero-reference bindings `a` 和 `b`。

最终输出：

```monkey
print(2);
```

Propagation pass 把“initializer 在 folding 后已是 literal”的 binding reference
替换成该 literal 的独立 clone。后续 pass 会原地改写 node，因此不能让多个位置
共享同一个 AST object。Propagation 本身不删 statement：reference 清零的 binding
交给 DCE。

安全论证建立在 compiler 的 slot semantics 上：每条 `let` 都分配新 slot（rebinding
不是赋值），slot 恰好写入一次，因此解析到某 binding 的 reference 读到的值恒等于
该 binding initializer 的值——closure 也一样，`let v=1; let g=fn(){v}; let v=2;`
中 `g()` 返回 1，已用探针在真实 GC VM 上验证。唯一反例是 `if` arm 里的
`let`：两个 branch 共用符号表（Phase 3 规则 5），`let` 之后的 reference 会解析到
它，但 arm 未执行时 slot 从未写入，reference site 读到 null——
`let v=1; if(1>2){let v=2;}; puts(v);` 输出 `null`。Scope analysis 给这类
binding 打 `conditional` 标记，propagation 一律跳过；函数/方法体是新的执行边界，
body 顶层 `let` 每次调用都执行，标记在 callable 边界重置。

其余护栏：

- 只 propagate 四种 folded shape：integer/boolean/string literal 与
  `UnaryExpression(-, Integer)`。array/hash literal 每次求值分配新值，复制到
  多个位置会改变 aliasing，直接排除。
- `new` 的 callee 语法上必须是 identifier，不可替换；保留该 reference 后 binding
  自然存活。
- Size guard：保留 binding 的成本是 `let x=<lit>;` 的 7 个固定字符加每处 reference
  1 字符（mangle 后），inline 成本是每处 `width` 字符；
  `refs × (width−1) > 7 + width` 时放弃，例如 `"hello"` 有三个 references 时就保留
  binding。

例如关闭 mangling 后，下面的 binding 会因为 size guard 保留，而不是复制三份
`"hello"`：

```monkey
let s="hello";puts(s);puts(s);puts(s);
```

Propagation 与 folding 互相解锁，所以两者交替运行到 fixed point，之后才进入
DCE。Termination proof：每轮成功 propagation 至少替换掉一个 identifier node，
而两个 pass 都不会新建 identifier，identifier 总数严格递减、有下界。三个危险案例
（conditional `let`、rebinding + closure capture、`new` callee）各有 differential
fixture 钉住。

### Dead `let` elimination（DCE）

一个 binding identity 的 reference count 为 0，并且 initializer 被证明为
side-effect-free and total（不会 throw）时，才删除整条 `let`。仅检查
`FunctionCall`/`New` 不够：property/index read、invalid operand type 和 division by
zero 同样会报 runtime error；删除它们会把 error program 变成 success program。

当前实现用保守的 `isPureTotal` allowlist 同时证明“没有 side effect”和“不会
throw”，而不是分别维护 `hasEffects` / `mayThrow` summary，也不是用 denylist 猜
purity：

- integer/boolean/string literal 是 total；array/hash 只有在所有 child expressions
  都 total，且 hash key 已证明 hashable 时才是 total。
- prefix/infix 只有在静态已知的 operand 类型受 GC VM 支持、且整数除法排除两个
  错误条件时才是 total。
- `FunctionCall`、`New` 一律不可删；`PropertyExpression` 和 `IndexExpression` 都是
  potentially throwing，即使没有 getter 也不能当作安全读取。unresolved identifier /
  unknown node 也默认不可删。
- function literal 的**创建**不执行 body，所以 runtime effect proof 把 body 当叶子；
  但 body 中若有 unresolved name 或其他 compiler/analyzer diagnostic，必须给
  initializer 加不可变换标记，不能借删除改变 compile status。

典型边界：

```monkey
let unusedHelper = fn(x) { puts(x) };   // removable: creating a closure is pure/total
let x = compute();                       // retained: call may have side effects
let y = 42;                              // removable: pure/total literal, no references
let z = 1 / 0;                           // retained: division by zero throws
let p = 1.missing;                       // retained: property read may throw
```

删除会级联：删掉一条 `let` 可能让另一条失去唯一 reference，所以每轮基于 binding
identity 重新运行 analysis 并迭代到 fixed point。DCE 在 mangling **之前**运行，让
最终 reference frequency 和命名建立在实际保留的 program 上。

`let` 还有两个 compiler/VM 特有的保守边界：

- function/method 或 `if` branch 的末尾 `let` 是 implicit return value barrier；
  删除它会暴露前一条 expression，所以必须保留。
- VM 进入 callable 时会一次性预留全部 local slots。若其中有空 `if` arm 或以
  `let` 结尾、因而可能不产生栈值的 arm，现有 bytecode 会让后续 pop/赋值读到
  预留 slot；此时改变任意 local 数量都可能改变现有运行结果。这样的 callable
  整体禁用 local DCE，并让嵌套 callable 各自独立判断。

## Test strategy

Fixtures 按用途组织；当前没有强制所有 suite 共用同一个 corpus：

- `test/corpus/core.monkey` 和 `classes.monkey`、`examples/hello.monkey` 以及少量 inline
  programs 用于 structural round-trip；
- `differential.test.ts` 使用一组专门的、确定终止的 inline programs；
- printer、mangling、folding、propagation 和 DCE 的窄边界由各自 unit tests 覆盖。

Standalone compiler + GC VM 是 semantic authority。Interpreter 与 compiler 已有
已知差异，因此不作为 optimization oracle。

1. **Lossless contract**：parser test 直接断言 `Integer.raw` 是 decimal string，
   覆盖 `9007199254740993`、`i64::MAX` 和 nested array/hash；Wasm/minifier tests
   继续钉住 API boundary 和 printer output，全程不通过 JS `number` 中转。
2. **Structural round-trip**：canonicalizer 只删除 `span` 和 `comments`。Printer-only
   case 比较原 AST 与重新 parse 的 AST；transformed case 按 production order 执行
   folding ↔ propagation fixed point、DCE、mangling，再比较 transformed AST 与
   reparsed AST。它能抓到 runtime differential test 可能看不出的 structural drift。
3. **Output-aware differential test**：original/minified program 分别创建全新
   `Compiler`，生成 snapshot 后交给 GC VM。`run_snapshot_with_output` 在
   success/error envelope 中都保留已产生的 `stdout`，error 还提供稳定的
   `stage`/`kind`。成功时比较
   `status + result + stdout`；失败时比较 `status + stage + kind + stdout`。
   Diagnostic message、变量名和 span 不属于 equivalence contract。Instruction
   budget tests 独立运行，不把优化后少执行几条 instruction 当作 semantic change。
4. **Idempotence**：四种 boolean option 组合都断言
   `minify(minify(src)).code === minify(src).code`。
5. **Mangling safety**：unit/structural/differential tests 合起来覆盖连续 rebinding、
   RHS 旧值、self-recursion、duplicate parameters、closure capture、builtin shadow、
   两个 `if` branch、`this`、unresolved name、class/property/method，并钉住
   class/instance 的 visible name。
6. **Folding/propagation/DCE safety**：unit tests 覆盖 i64 boundary、division by zero、
   `i64::MIN / -1`、不可表示的 `if` 和 pure/total allowlist；end-to-end differential
   programs 覆盖 conditional `let`、rebinding + closure capture、`new` callee，以及
   property/index read 和 invalid operand type 等 potentially-throwing initializer。

Compression ratio 不是 correctness assertion，也不会写进 test output。Playground
直接展示当前 source 的 UTF-8 byte delta，避免维护会过期的 golden ratio。

## CI、release 与 docs

- `.github/workflows/rust.yml` 已在最低 Node 24 上运行 minifier build、Vitest、Node
  entry smoke、Vite/browser smoke，并由 playground test 覆盖 `MinifyView`。
- 按 `AGENTS.md`，feature PR 不做 version bump。
  `scripts/bump_cargo_packages.ts` 会在 release PR 中同步 minifier version、Wasm
  dependency range 和 playground minifier range。
- npm release 仍需给 release-please workflow 增加 minifier build/publish step；
  playground demo 直接 import workspace source，不依赖 package 发布。
- `AGENTS.md` 已加入 minifier package，package 自带 README 和 CLI usage。

## Implementation status

| Area                                                                | Status             |
| ------------------------------------------------------------------- | ------------------ |
| Lossless AST、package scaffold、compact printer                     | Implemented        |
| Structural round-trip、output-aware differential runner             | Implemented        |
| Playground Minify view、UTF-8 stats、stale request guard            | Implemented        |
| Scope analysis、identifier mangling、playground switch              | Implemented        |
| Constant folding、constant propagation、DCE                         | Implemented        |
| CLI、Node/browser entries、package smoke tests、README              | Implemented        |
| `runs identically ✓` playground badge                               | Optional follow-up |
| release-please build/publish step for `@gengjiawen/monkey-minifier` | Follow-up          |
