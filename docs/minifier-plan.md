# Monkey Minifier — 设计与实施计划

为本仓库的 Monkey 方言实现一个 source-to-source minifier，用 TypeScript 写成
workspace 新包，并在 playground 里提供在线演示视图。

处理管线：

```
源码 ──wasm parse_lossless──▶ JSON AST ──(变换 pass)──▶ AST ──紧凑 printer──▶ 压缩后源码
                                         mangle / 常量折叠 / 常量传播 / 死代码消除
```

Parser 不重新实现。minifier 消费 `@gengjiawen/monkey-wasm` 新增的
`parse_lossless()` 输出；它和现有 `parse()` 来自同一个 Rust AST，但把 i64 整数
序列化成十进制字符串，避免经过 JavaScript `number` 时丢精度。prettier 插件可以
继续使用原 API，minifier 不直接消费当前的 `parse()`。

## 目标

- 正确性优先，并把检查拆成两个可判定的不变量：
  - v0：`parse_lossless(print(parse_lossless(src)))` 与原 AST 结构相等（忽略 span）。
  - 有变换时：打印结果重新 parse 后与 pass 产出的 canonical AST 结构相等；运行
    等价由**全新 `Compiler` + GC VM** 的 `status/result/stdout` 判定。
    interpreter 在重绑定闭包等场景与 compiler 已有不同语义，因此只作补充兼容
    检查，不是优化的语义权威。
- v0 做空白/注释剥离 + 冗余括号消除；v1 做标识符改名（mangle）；v2 做常量
  折叠、常量传播与死 `let` 消除（可选增量）。
- 发布为 npm 包，API 小而稳定：`minify(source, options)`。
- Playground 演示：新增 `Minify` 输出视图，实时展示压缩结果和字节数统计。

## 非目标（暂时）

- Source map（AST 里有 span，以后想做随时能做）。
- 属性/方法名改名（`obj.prop` 保持原样——和 terser 默认关闭 `mangleProps`
  同一个原因，不安全）。
- 面向 gzip 的启发式优化。
- 一切格式化相关的事——那是 `prettier-plugin-monkey` 的职责。

## 包布局

新包 `packages/monkey-minifier`（`packages/*` 通配自动进 workspace）。npm 包名用
`@gengjiawen/monkey-minifier`——和 `@gengjiawen/monkey-wasm` 一样带 scope，避免
抢注问题；构建/测试配置照抄 `prettier-plugin-monkey`（纯 `tsc` + vitest）。

```
packages/monkey-minifier/
  package.json          # 依赖: @gengjiawen/monkey-wasm；build: tsc + ESM bundle
  tsconfig.json
  src/
    index.ts            # 浏览器/bundler 入口
    node.ts             # Node 入口，显式实例化 Wasm
    core.ts             # minify() 共享管线、选项处理
    cli.ts              # monkey-minify 命令行入口
    types.ts            # AST 类型（复制而来，见下）
    printer.ts          # 紧凑 printer（Phase 1）
    scope.ts            # 作用域分析（Phase 3）
    mangle.ts           # 标识符改名（Phase 3）
    fold.ts             # 常量折叠 / 死代码消除（Phase 4）
    propagate.ts        # 常量传播（Phase 4）
  test/
    printer.test.ts
    roundtrip.test.ts
    differential.test.ts
    mangle.test.ts
```

### Lossless AST 传输与类型

PR 1 先在 parser/wasm 增加一个工具专用的 lossless AST serializer 和
`parse_lossless()` 导出。这是 blocker，不是后续优化：Rust 节点的
`Integer.raw` 是 i64，而现有 TS 镜像写成 `number`；例如
`9007199254740993` 经 `JSON.parse` 会变成 `9007199254740992`，原 AST 和
round-trip AST 两边一起失真时甚至会假通过。

lossless JSON 只把 `Integer.raw` 投影为十进制字符串，其他节点形状保持不变；不
修改现有 `parse()` 的返回格式，以免破坏 prettier/playground 消费方。minifier 的
`src/types.ts` 相应使用：

```ts
interface IntegerLiteral extends ASTNode {
  type: 'Integer'
  raw: string
}
```

printer 原样输出已验证的十进制串，数值 pass 用 `BigInt(raw)` 运算，任何时候都不
把 Monkey 整数放进 JS `number`。测试必须包含 `2^53 + 1` 和 `i64::MAX`，并直接
断言 lossless AST/输出，不能用两个已失真的 `number` 做比较。

### AST 类型：先复制，不共享

`src/types.ts` 起步时直接复制
`packages/prettier-plugin-monkey/src/types.ts` 里的节点类型。从 prettier 插件
import 会把它的 `prettier` peerDependency 拖进 minifier 的依赖图，方向也是反的。
这些类型是 serde JSON 输出的手写镜像，变动频率很低，两份拷贝可以接受。

如果以后开始难受，按顺序有两条退路：

1. 抽一个 `packages/monkey-ast` 纯类型包，两边共同依赖。
2. 用 `ts-rs` 从 Rust 的 AST 结构体直接生成 TS 类型，让 `parser/ast.rs` 成为
   唯一事实来源。最符合本仓库 Rust-first 的架构，但 v0 不值得提前做。

### 公共 API

```ts
interface MinifyOptions {
  mangle?: boolean | { reserved?: string[] } // Phase 3 落地后默认 true
  fold?: boolean // Phase 4 落地后默认 true
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

## Phase 1 — 紧凑 printer（v0）

v0 里唯一真正需要动脑的部分。三个设计点：

**不抄 Rust 的 `Display` 实现。** 它是调试辅助，不是代码生成器：infix 全括号
打印（`ast.rs:302`），`if` 条件打印时不带括号——而 parser 强制要求括号
（`parser/lib.rs:397` 的 `expect_peek(LPAREN)`），也就是说 `Display` 的输出
确定不能 re-parse。TS printer 从语法出发重写，round-trip 测试是正确性的唯一
裁判。

**按优先级决定括号。** 把 `parser/precedences.rs` 的表搬过来
（`Lowest < Equals < LessGreater < Sum < Product < Prefix < Postfix`）。规则：

- 子表达式优先级低于上下文优先级时才加括号。
- 所有 infix 运算符都按左结合 parse：同优先级时只给右子树加括号
  （`a - (b - c)`），左子树永远不加。对 `+`/`*` 也统一套用这条——正确且简单，
  可证结合的场景跳过括号属于后期微优化。
- 前缀运算符的操作数：低于 `Prefix` 才加括号（所以是 `-(a + b)`，但 `-a[0]`
  不用加，index 是 `Postfix`）。

AST 不记录冗余括号（parse 时作为 grouped expression 就消掉了），所以 v0 对
括号多的源码天然就有压缩收益。

**语句分隔与 token 粘连。** `Let`、`Return`、`SetProperty` 和表达式语句后补
`;`；`Class` 后**不能**补分号，`class A {};` 会让 parser 把多出的 `;` 当成一条
没有 prefix parser 的表达式并报错。相邻语句是否需要分隔由 statement printer
显式处理。

词法空格只在删掉后会把两个 token 合并时插入，例如 `let x`、`return x`、
`new Foo`、`class Foo`。输出永远是 `fn(` 和 `}else{`，这里不需要空格。另用
token-boundary fixture 固化 `a- -b`/`a--b`、关键字与标识符、相邻语句等情况；
语句开头的 `{` 无歧义地是 hash 字面量（这个语法没有通用块语句）。

**节点全覆盖。** 语句：`Let`、`Return`、`Class`、`SetProperty`、表达式语句。
表达式：标识符、五种 `Literal`（`Integer`、`Boolean`、`String`、`Array`、
`Hash`）、prefix、infix、`if`/`else`、`fn`、调用、index、`this`、属性访问、
`new`。三个已从源码确认的输出形态：

- `if` 条件必须带括号：`if(c){...}else{...}`。
- 函数一律输出 `fn(params){...}`：`fn name()` 不能 parse（`parser/lib.rs:463`，
  `fn` 后紧跟 `expect_peek(LPAREN)`）。AST 里的 `FunctionDeclaration.name`
  是 `parse_let_statement` 从 let 绑定回填的递归元数据（`parser/lib.rs:121`，
  compiler 在函数内部为它定义 `Function` symbol）；printer 不输出这个字段，
  但变换后的 canonical AST 必须同步维护它。
- 字符串字面量原样输出：lexer 的 `read_string`（`lexer/lib.rs:195`）没有任何
  转义机制，读到下一个 `"` 为止——字符串内容不可能含 `"`，反斜杠是普通字符，
  还可以跨行。printer 直接输出 `"` + 原文 + `"`，零处理。

注释根本进不了 AST，剥离零成本。

## Phase 2 — playground 演示

沿用 `packages/playground/src/App.tsx` 现有的输出视图模式：

- `OutputView`（`App.tsx:107`）加 `'minify'`，SegmentedControl 里在
  AST / Bytecode / GC / Snapshot / ARM64 旁边加一个 **Minify** tab。
- 新建 `src/MinifyView.tsx`，照 `Arm64View` 的样子：只读 CodeMirror `Editor`
  展示压缩结果，加一条统计栏——原始字节数 → 压缩后字节数和节省百分比。状态
  类型 `MinifyState = idle | ok | invalid`，对齐 `Arm64BuildState`；两端字节数都用
  `TextEncoder` 算 UTF-8 bytes。
- 视图激活时才计算，带 debounce，抄 `debouncedArm64Compile` 的 effect 写法
  （`App.tsx:328`）。动态 import 和计算结果都携带递增 request id，并在提交状态
  前确认它仍对应最新 source、当前视图仍激活；否则首次加载较慢时旧请求可能覆盖
  新输入。
- 通过动态 `import('../../monkey-minifier/src/index')` 引入——和 Format 按钮
  引 prettier 插件的先例完全一致（`App.tsx:281`），所以 **demo 不依赖发 npm**。
- Parse 错误时面板里展示错误信息，和其他视图一致。

后续增量（不阻塞第一个 demo PR）：

- Phase 3 落地后在工具栏加 mangle 开关 `Switch`，访客能实时看到改名效果。
- "runs identically ✓" 徽章：在下面测试章节的 output-aware runner 落地后，分别
  执行原始和压缩版并对比 `status/result/stdout`，把正确性主张变成看得见的演示；
  不能只用当前不捕获输出的 `run_snapshot`。
- 如果现有 snippet 演示效果不够好，加一个专门的 `Minify` snippet
  （闭包 + class 的组合最能展示）。

测试放 `src/test/MinifyView.test.tsx`，沿用现有视图测试的写法。

## Phase 3 — 标识符 mangle（v1）

实际压缩收益的大头，但只改名能解析到明确 binding identity 的标识符。以下名字
保持不动，生成名也不能与它们碰撞：

- builtin：完整列表是 `len`、`puts`、`first`、`last`、`rest`、`push`、`print`
  （`object/builtins.rs`）。builtin 引用不改名；用户 `let` 可以 shadow builtin，
  该用户 binding 则是另一个 identity，可以改名。
- 属性名和方法名：`PropertyExpression.property`、
  `SetPropertyStatement.property`、`MethodDefinition.name`。Hash key 是普通表达式，
  也绝不能被当作属性名统一改写。
- class declaration 的 binding 及解析到它的引用。虽然 class 名参与普通词法解析，
  GC VM 会在 class、instance、bound method 的最终值和 `puts` 输出中显示它；直接
  把 `class LongName` 改成 `class a` 会改变可观察结果。同名的后续 `let` 重绑定是
  独立 identity，不受此限制。
- `this`、用户传入的 `options.mangle.reserved`，以及所有未解析/外部标识符。遇到
  analyzer 无法精确建模的节点时 fail closed，不对相关 scope 改名。

`scope.ts` 不是只收集字符串的传统 hoist scope，而是按
`Compiler::compile_stmt` 的遍历顺序构建 binding/reference graph。语义权威是每次
都从干净状态创建的 standalone compiler；不能用 interpreter environment 反推，
因为例如重绑定后闭包读取的值，两套引擎目前并不一致。需要逐条镜像这些规则：

1. 只有 program、函数字面量和每个 method body 创建 symbol scope；`if`/`else`
   block 不创建 scope，class declaration 自身也不创建额外 scope。
2. 普通 `let` 先解析/编译 RHS，再定义一个新 slot。同名 `let` 是新的 binding，
   只遮蔽它之后的引用；更早创建的闭包继续捕获旧 binding。分析器可先创建一个
   pending identity，但在 RHS 结束前不能把它放进普通可见名字表。
3. 直接形如 `let f=fn(...){...}` 的 RHS 是特例：parser 回填的
   `FunctionDeclaration.name` 会在函数内部先定义递归 self symbol；它与 pending
   `f` 绑定关联，但不让 `f` 在 RHS 的其他位置提前可见。进入函数后先定义这个
   self symbol，再按参数列表顺序定义参数；重复参数或与 self 同名的参数以最后
   定义的 slot 为引用目标。
4. class binding 在编译 methods **之前**定义，因此方法可引用本 class。method
   scope 先有隐式 `this`，再按顺序定义参数；嵌套函数按正常闭包规则捕获 `this`。
5. compiler 依次编译 `if` 的 condition、consequent、alternate，两个 branch 共用
   当前 symbol table；即使运行时只走一个 branch，其中的 `let` 也会按这个编译
   顺序影响另一个 branch 和后续引用。分析器必须复现这个行为，不能分别 clone
   两份词法环境再 merge。
6. builtin 是初始 symbol，用户声明可按上述源码顺序 shadow；找不到 binding 的
   identifier 保持原拼写，并加入生成名禁用集合，防止意外 capture。

`mangle.ts` 按 graph 中的 identity 改 declaration 和所有已绑定 reference，而非
按字符串全局替换。第一版优先保证可靠：按引用频率给可改 binding 分配全程序唯一
的短名（`a`–`z`，再到 `a0`…），并跳过关键字、builtin、reserved、class 名、未
解析名和所有保留原名的 binding。以后再基于 interference/liveness 证明安全地在
不相交 scope 复用短名。

改直接绑定函数字面量的 `let` 时，还要同步其 `FunctionDeclaration.name` 元数据；
printer 仍只输出 `fn(`，重新 parse 会根据新 let 名回填同样的 metadata。fixture
至少覆盖连续重绑定、RHS 读取旧 binding、自递归、重复参数、builtin shadow、
闭包捕获、两个 if branch、隐式 `this`、class 自引用和未解析 identifier。

## Phase 4 — 压缩 pass（v2，可选）

每个都是自包含的 AST→AST pass，但集成顺序固定为：常量折叠与常量传播交替到
不动点 → 重建 binding/effect 分析 → 死 `let` 消除到不动点 → 再分析 →
mangle → printer。不能宣称任意顺序；折叠为传播制造 literal 初始化式，传播为
折叠解锁卡在 identifier 上的表达式，两者共同暴露死绑定，删除又会改变引用
频率和后续 slot 编号。任一分析不确定时保留原节点。

### 常量折叠

整数算术、字符串拼接、布尔/比较运算、前缀运算严格镜像 production GC VM，而
不是 interpreter。整数从 lossless `raw` 用 `BigInt` 计算：

- 加、减、乘和负号按 i64 two's-complement 回绕，用
  `BigInt.asIntN(64, value)`。启用此 pass 前先把 GC VM 对应操作改成显式
  `wrapping_add/sub/mul/neg`，消除 Rust debug/release overflow 行为差异。
- 除法用 BigInt 的向零截断；除数为 0、以及 `i64::MIN / -1` 都保留原表达式，让
  VM 产生原来的 runtime error。
- AST 没有负整数字面量。负结果必须构造成 `UnaryExpression(-, Integer(abs))`；
  `i64::MIN` 的绝对值超出 lexer 可接受的正 i64，无法这样打印，因此放弃该次
  折叠，绝不生成非法 literal。
- 只有操作数类型和 VM 结果都能证明时才折叠；字符串只折叠 `+`，比较/`!` 也只
  覆盖 GC VM 已定义的常量类型组合。每个溢出边界和错误路径都写 differential
  fixture。

`if` 是 expression，而它的 branch 是任意 `BlockStatement`，语言又没有可打印的
`null` literal，所以不能通用地拿选中 block 替换整个 `if`。v2 只折叠可表示子集：

- condition 已被证明为无副作用的常量；
- 被选 branch 恰好只有一个表达式语句，可直接替换为该 expression；
- consequent 和 alternate 都不含会修改当前 compiler symbol table 的声明，否则
  删除未执行 branch 仍可能改变另一个 branch 或后续引用的 binding；
- constant false 且没有 `else` 时保留原 `if`，因为 AST 中没有 Null literal。

例如 `let x=if(true){let y=1;y};` 不能折叠成一个 block，必须保留。

还有一个 parser 元数据边界：`let f=fn(){...}` 会给函数体注入递归 self binding，
而 `let f=if(true){fn(){...}}` 里的嵌套函数是匿名的。即使 condition 是常量，也
不能把后者折叠成直接 RHS 函数字面量，否则打印后重新 parse 会改变闭包捕获。

### 常量传播

折叠只处理“表达式内部全是常量”的情况；`let a=1+1; let b=a+1; print(a)` 里
`b` 的 RHS 和 `print` 的实参都卡在 identifier 上，折叠自身永远打不开。传播
pass 把“初始化式在折叠后已是 literal”的 binding 的每处引用替换成该 literal
的深拷贝（后续 pass 原地改写节点，多处共享同一节点会被重复改写），从而让
上面的程序最终压成 `print(2);`，对齐 terser 在等价 JavaScript 上的输出。
传播自己不删语句：引用清零的 binding 交给死 `let` 消除按它的规则处理。

安全论证建立在 compiler 的 slot 语义上：每条 `let` 都分配新 slot（重绑定
不是赋值），slot 恰好写入一次，因此解析到某 binding 的引用读到的值恒等于
该 binding 初始化式的值——闭包也一样，`let v=1; let g=fn(){v}; let v=2;`
中 `g()` 返回 1，已用探针在真实 GC VM 上验证。唯一反例是 `if` arm 里的
`let`：两个 branch 共用符号表（Phase 3 规则 5），`let` 之后的引用会解析到
它，但 arm 未执行时 slot 从未写入，引用处读到 null——
`let v=1; if(1>2){let v=2;}; puts(v);` 输出 `null`。scope 分析给这类
binding 打 `conditional` 标记，传播一律跳过；函数/方法体是新的执行边界，
body 顶层 `let` 每次调用都执行，标记在 callable 边界重置。

其余护栏：

- 只传播四种折叠后形态：integer/boolean/string literal 与
  `UnaryExpression(-, Integer)`。array/hash 字面量每次求值分配新值，复制到
  多个位置会改变别名关系，直接排除。
- `new` 的 callee 语法上必须是 identifier，不可替换；该引用保留后 binding
  自然存活。
- 体积护栏：保留 binding 的成本是 `let x=<lit>;` 的 7 个固定字符加每处引用
  1 字符（mangle 后），内联成本是每处 `width` 字符；
  `refs × (width−1) > 7 + width` 时放弃，例如 `"hello"` 引用三次就保留
  绑定。

传播与折叠互相解锁——传播把 literal 塞进卡在 identifier 上的表达式，折叠
再把它算成新的 literal 初始化式——所以两者交替运行到不动点，之后才进入死
`let` 消除。终止性：每轮成功传播至少替换掉一个 identifier 节点，而两个
pass 都不会新建 identifier，identifier 总数严格递减、有下界。三个危险案例
（conditional `let`、重绑定 + 闭包捕获、`new` callee）各有 differential
fixture 钉住。

### 死 `let` 消除（去掉无用变量）

一个 binding identity 零引用，并且 initializer 被证明为“无副作用且 total/no
throw”时，才删除整条 `let`。仅检查 `FunctionCall`/`New` 不够：属性/index 读取、
类型不匹配的运算和除零同样会报 runtime error；删除它们会把错误程序变成成功
程序。

实现一个保守的 effect summary（至少区分 `hasEffects` 与 `mayThrow`），初版采用
白名单证明安全，而不是列一份黑名单猜纯度：

- integer/boolean/string literal 是 total；array/hash 只有在所有子表达式 total、
  且 hash key 已证明可 hash 时才是 total。
- prefix/infix 只有在静态已知的 operand 类型受 GC VM 支持、且整数除法排除两个
  错误条件时才是 total。
- `FunctionCall`、`New` 一律不可删；`PropertyExpression` 和 `IndexExpression`
  默认 `mayThrow`，即使没有 getter 也不能当作安全读取。未知 identifier/节点也
  默认不可删。
- 函数字面量的**创建**不执行 body，所以 runtime effect 分析把 body 当叶子；但
  body 中若有未解析名字或其他 compiler/analyzer diagnostic，必须给 initializer
  加不可变换标记，不能借删除改变 compile status。

典型边界：

```
let unusedHelper = fn(x) { puts(x) };   // 可删：定义时什么都不执行
let x = compute();                       // 不可删：初始化有调用，可能有副作用
let y = 42;                              // 可删：纯字面量，零引用
let z = 1 / 0;                           // 不可删：会报错
let p = 1.missing;                       // 不可删：属性读取会报错
```

删除会级联：删掉一条 `let` 可能让另一条失去唯一引用者，所以每轮基于 binding
identity 重新分析并迭代到不动点。DCE 在 mangle **之前**运行，让最终引用频率和
命名建立在实际保留的程序上。

`let` 还有两个 compiler/VM 特有的保守边界：

- function/method 或 `if` branch 的末尾 `let` 是隐式返回值的 barrier；删除它会
  暴露前一条表达式，所以必须保留。
- VM 进入 callable 时会一次性预留全部 local slots。若其中有空 `if` arm 或以
  `let` 结尾、因而可能不产生栈值的 arm，现有 bytecode 会让后续 pop/赋值读到
  预留 slot；此时改变任意 local 数量都可能改变现有运行结果。这样的 callable
  整体禁用 local DCE，并让嵌套 callable 各自独立判断。

## 测试策略

语料先行：`App.tsx` 里的 playground snippets、`examples/*.monkey`、以及从
parser/compiler 各 `*_test.rs` 里提取的程序。interpreter 用例只有在 standalone
compiler 也接受时才进入语义语料；两者结果冲突时以 compiler + GC VM 为准。统一
收进 `test/corpus/`，下面所有套件共用。

1. **Lossless contract**：单独测试 `parse_lossless()` 的 `Integer.raw` 是十进制
   string，覆盖 `9007199254740993`、`i64::MAX`、嵌套 array/hash 中的整数；禁止
   通过 JS `number` 中转。
2. **Round-trip**（printer 主力）：定义一个只删除 `span`/comment 附属字段的
   canonicalizer。v0 比较 `canonical(parse_lossless(print(ast)))` 与原 AST；有
   pass 时比较 reparsed AST 与 pass 返回的 canonical transformed AST。不能只比
   运行结果，因为两棵同样被错误打印的程序可能碰巧返回同一个值。
3. **Differential 执行**：原始版和压缩版分别用全新的 `Compiler` 编译，再交给
   GC VM 执行。当前 `run_snapshot` 只有 `{status,result}`，`puts`/`print` 最终走
   `println!`，所以它不是完整 oracle；PR 1 就补齐下面的测试载具，而不是等 DCE：
   - 给 GC VM 的 builtin 执行路径注入 output sink，避免换成语义不同的
     interpreter，也避免共享的 thread-local 缓冲；
   - 新增 `run_snapshot_with_output`（或等价内部 helper），成功和失败 envelope 都
     返回已产生的 `stdout`，从而保留“输出后报错”的可观察顺序；
   - 为 compile/runtime error 暴露稳定的 structured `kind`。成功时比较
     `status + result + stdout`；失败时比较 `status + stage + kind + stdout`。
     diagnostic message、变量名和 span 不纳入等价定义，因为 mangle/printer 会
     合法地改变内部名字与源码位置。
     differential corpus 使用确定终止、不会贴近 instruction budget 的程序；执行
     上限测试独立进行，不把优化后少执行几条指令当作语言语义差异。
4. **幂等性**：`minify(minify(src)).code === minify(src).code`，分别覆盖每组
   options。
5. **Mangle 安全性**（Phase 3 起）：连续重绑定、RHS 旧值、自递归、重复参数、
   闭包捕获、builtin shadow、if 两分支污染、`this`、未解析名、class/属性各写
   fixture，全部过 structural + differential 套件；另断言 class/instance 的
   可见名称没有变化。
6. **Fold/Propagate/DCE 安全性**（Phase 4 起）：覆盖 i64 边界、除零、
   `i64::MIN / -1`、不可表示的 if、传播的三个危险案例（conditional `let`、
   重绑定 + 闭包捕获、`new` callee），以及“无调用但会报错”的 initializer
   （错误 operand 类型、property/index）。每次删除前后都跑 output-aware
   differential。

全语料的压缩率在测试输出里打印成报告，不做断言，这样改进 printer 永远不会
"挂"在一个过期数字上。

## CI、发布、文档接线

- CI：`.github/workflows/rust.yml` 已有 playground 测试；给 scoped 包加 build、
  Vitest、Node 发布入口和 Vite browser 入口冒烟，并在声明的最低 Node 24 上
  运行。playground 的测试步骤顺带覆盖 `MinifyView`。
- 版本：按 `AGENTS.md`，功能 PR 不带版本号变更。
  `scripts/bump_cargo_packages.ts` 会在 release PR 中同步 minifier 版本、
  Wasm 最低版本和 playground 的 minifier range；要发 npm 时仍需补
  release-please / publish 配置。
  playground demo 不依赖发布。
- 文档：更新 `AGENTS.md` 的项目结构一节（它连 vscode extension 都还没写进去），
  新包加 README。

## 里程碑 / PR 切分

| PR  | 内容                                                                  | 预估     |
| --- | --------------------------------------------------------------------- | -------- |
| 1   | lossless AST 导出 + 脚手架 + printer + structural/output differential | 2 天     |
| 2   | Playground `Minify` 视图 + UTF-8 字节统计 + stale request guard       | 0.5–1 天 |
| 3   | compiler-order binding 分析 + mangler（+ playground 开关）            | 2–3 天   |
| 4   | 明确 i64 VM 语义 + 常量/受限 `if` 折叠 + 常量传播 + 保守死 `let` 消除 | 2–3 天   |
| 5   | CLI `bin` + 发布接线 + 文档                                           | 0.5 天   |

PR 1+2 合起来交付可见的 demo；之后全是增量。
