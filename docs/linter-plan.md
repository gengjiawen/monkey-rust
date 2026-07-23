# Monkey Linter — 设计提案

> 状态：提案（未实施）。本文设计一个 Monkey 语言的 linter，作为 workspace
> package 提供（对标 `monkey-minifier` 的形态），发布为 npm 包，并接入
> playground 编辑器与 VS Code extension 的 diagnostics。

处理管线：

```
source ──wasm analyze_lossless（parse + validation）──▶ tagged JSON result
                                                            │
                         ┌──────────── error（stage/message/span?）
                         │                                  │ ok
                         ▼                                  ▼
                    Diagnostic[]       JSON AST ──scope 分析──▶ 逐规则 walk
                         ▲                                  │
                         └──────────────────────────────────┘
                         │
         CLI / playground / VS Code extension adapters
```

Parser 和 validation 都不在 TypeScript 中重新实现。现有 `parse_lossless()` 只解析
AST，并不会调用 `parser/validation.rs`；因此 v0 需要在
`@gengjiawen/monkey-wasm` 增加一个小的 `analyze_lossless()` API，依次执行 parse
与 validation，再返回带 `status` 的 JSON。成功结果中的 AST 自带 UTF-8 byte
span；失败结果保留 `stage`、`message` 与可选 span。对独立源码文件做 validation
时，predefined globals 应与 fresh interpreter/compiler 一样包含完整 builtin 表。

## 职责边界（先划清楚）

仓库里已经有三层"代码质量"设施，linter 只补中间缺的那层：

| 层           | 归属                                                          | 职责                                                                                     |
| ------------ | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| 硬错误       | parser + `parser/validation.rs`，由 `analyze_lossless()` 串联 | 语法错误，以及 undefined variable、`this` 出现在 method 外、constructor 返回值等语义错误 |
| **静态诊断** | **linter（本提案）**                                          | **合法程序中高置信度有问题或大概率无意为之的模式**                                       |
| 转换         | `monkey-minifier` / `prettier-plugin-monkey`                  | 改写代码（压缩/格式化），不做诊断                                                        |

validation 目前由 interpreter/compiler 显式调用，不是 parser 的组成部分；现有 VS
Code extension 只调用 `parse()`，所以只会报告 parse error。接入 linter 后由新 API
补齐 validation，但 TS 侧不重复实现这些检查。输入有 parse/validation error 时，
把它转换成一条 error 诊断并停止，**不 lint 半棵树**（容错 parse 是非目标）。

## 为什么是 TS 包而不是 Rust crate

- 三个消费者（CLI、playground、VS Code extension）全是 JS 环境。
- 硬语义检查由 Rust 侧 `validation.rs` 完成；wasm analyzer 负责调用它，linter
  规则本身只需要 AST + 作用域信息。
- 规则迭代不该绑上 wasm 构建周期（AGENTS.md 明确提醒过 wasm 产物过期的坑）。
- `monkey-minifier` 是直接先例：同一 AST、同一包形态，其 scope/mangle 的作用域
  分析经验（声明、引用、遮蔽、闭包捕获）可直接迁移。

若未来要把 lint 下沉进编译管线或做原生 LSP server，再评估 Rust 化；本提案不排除
但列为非目标。

## 目标

- npm 包 `@gengjiawen/monkey-lint`，API 小而稳定：
  `lint(source, options?) => { diagnostics: Diagnostic[] }`。
- core 诊断保留可选的 UTF-8 byte span；CLI、CodeMirror、VS Code adapter 各自
  转换成目标坐标系。
- 规则默认零配置即有用；error 级表示高置信度缺陷并使 CLI 非零退出，不承诺
  程序一定中止运行。
- CLI `monkey-lint`，退出码可用于 CI。
- 对涉及运行语义的规则，用 interpreter 与 GC VM 的实际行为建立逐规则 oracle。

## 非目标

- 格式化（`prettier-plugin-monkey` 的职责）与压缩（minifier 的职责）。
- 容错 parse / 对语法错误的文件 lint 半棵树。
- 类型推断、跨函数数据流分析——规则保持在语法 + 作用域层。
- v0 不做配置文件（`.monkeylintrc`）；规则开关走 API options / CLI flag。
- 不在 TS 中重做 parser/validation；wasm 侧只增加上述结构化 analyzer API。

## 包布局

新包 `packages/monkey-linter`（`packages/*` 通配自动进 pnpm workspace）。构建与
发布形态照抄 minifier：`tsc` 出 Node entry/types，esbuild 出 browser ESM bundle，
Vitest 测试，release-please 发布，版本由 `scripts/bump_cargo_packages.ts` 统一。

```
packages/monkey-linter/
  package.json          # @gengjiawen/monkey-lint；依赖 @gengjiawen/monkey-wasm
  tsconfig.json
  src/
    index.ts            # 浏览器/bundler 入口
    node.ts             # Node 入口（实例化或接收已加载的 wasm analyzer）
    cli.ts              # monkey-lint 命令行入口
    core.ts             # lint() 管线：analyze → scope → 规则调度 → 诊断排序
    types.ts            # AST 类型（自 minifier types.ts 复制）+ Diagnostic 类型
    walk.ts             # 通用 AST walker（enter/exit 回调）
    scope.ts            # 绑定/引用分析：声明、引用计数、遮蔽、闭包捕获
    rules/              # 一条规则一个文件，导出 { name, severity, check }
      no-unused-let.ts
      ...
  test/
    rules/              # 每规则 fixture + Vitest 快照
    corpus.test.ts      # examples/*.monkey 全量冒烟
```

Prettier plugin 与 minifier 已经各维护一套 AST 类型，linter 会成为第三个消费者。
v0 仍先 vendor 一份，以免把跨包 AST 类型重构绑进首发；实现稳定后再评估抽取
`@gengjiawen/monkey-ast` 共享包。

## 数据模型

```ts
interface ByteSpan {
  start: number
  end: number
}

interface Diagnostic {
  rule: string // 如 'no-unused-let'
  severity: 'error' | 'warn'
  message: string // 面向人的一句话，含标识符名等上下文
  span?: ByteSpan // UTF-8 byte offset，同 AST span
}

interface LintOptions {
  rules?: Record<string, 'off' | 'warn' | 'error'> // 覆盖默认级别
}

type AnalyzeResult =
  | { status: 'ok'; program: Program }
  | {
      status: 'error'
      stage: 'parse' | 'validation'
      message: string
      span?: ByteSpan
    }
```

AST 与 validation error 已有 UTF-8 byte span；parser error 当前只有字符串，未必能
提供 span，所以 `Diagnostic.span` 必须可选。core 不缓存 line/column，避免同时维护
多套坐标：CLI 从 byte span 派生 1-based 行列；CodeMirror 与 VS Code adapter 转换为
UTF-16 坐标，其中 VS Code 的 line/character 是 0-based。

parse/validation 失败分别映射为不可配置的 `parse-error` / `validation-error`
diagnostic；`LintOptions.rules` 只覆盖真正的 lint rules。

severity 语义：`error` = 高置信度缺陷，并触发 CLI 非零退出；它可能表现为 runtime
failure、错误对象/值，或静默地产生错误结果。`warn` = 大概率无意为之，但证据不足以
作为默认 CI 失败。运行行为由各规则自己的 oracle 说明，不从 severity 反推。

## 规则清单

下文标注"实测"的行为都已在当前 interpreter 与 GcVM 两个后端上跑过验证，
示例可直接转成实现时的 fixture。

### v0（首发，9 条）

| 规则                       | 级别  | 一句话                                   |
| -------------------------- | ----- | ---------------------------------------- |
| `no-unused-let`            | warn  | 绑定从未被引用                           |
| `no-unused-param`          | warn  | 参数从未被引用                           |
| `no-unreachable-code`      | warn  | `return` 之后的语句                      |
| `no-unused-expression`     | warn  | 值不处于被观察位置的纯表达式语句         |
| `no-duplicate-hash-key`    | error | hash 字面量重复 key                      |
| `builtin-arity`            | error | `len` 调用参数个数错误                   |
| `no-shadowed-builtin`      | warn  | 用户绑定遮蔽 builtin                     |
| `no-constant-condition`    | warn  | `if` 条件是 truthiness 固定的标量字面量  |
| `no-literal-type-mismatch` | error | 字面量运算被选定的两个后端 oracle 都拒绝 |

#### `no-unused-let`（warn）

`let` / `class` 绑定从未被引用。rebinding 时旧绑定若被新绑定的初始化引用，算已使用。

```
// ✗ total 从未被引用
let total = 1 + 2;
puts("done");

// ✗ class 声明的绑定同样计入
class Point { constructor(x) { this.x = x; } }

// ✓ 不报：旧 x 被新 x 的初始化引用
let x = 1;
let x = x + 1;
puts(x);
```

#### `no-unused-param`（warn）

```
// ✗ b 从未被引用
let add = fn(a, b) { a; };

// ✓ 不报：`_` 前缀显式声明不用
let visit = fn(node, _depth) { node; };
```

#### `no-unreachable-code`（warn）

```
// ✗ return 之后还有语句
let double = fn(x) {
  return x * 2;
  puts("never runs");   // ← 报在这里
};
```

#### `no-unused-expression`（warn）

不能只用“是否为 block 末位”判断，规则应跟踪表达式值是否处于被观察的 value
position：function/method 的尾表达式是隐式返回值；被使用的 `if` 表达式中，各分支
的尾表达式决定 `if` 的值；program 顶层最后一个表达式也是可观察结果。constructor
是例外，它总是返回 `this`，尾表达式的值仍会被丢弃。

调用与 `new` 可能有副作用，一律放过。index/property 读取虽然没有用户 getter，
但可能触发 runtime error，v0 也保守放过。规则只报告确认无副作用、且值没有任何
消费者的表达式语句。

```
// ✗ x + 1 的结果被丢弃（不是 block 末位）
let f = fn(x) {
  x + 1;
  return x;
};

// ✓ 不报：末位表达式语句是隐式返回值
let getName = fn(person) { person["name"]; };

// ✓ 不报：两个分支的值共同决定 if 的值
let sign = fn(positive) { if (positive) { 1; } else { -1; } };
puts(sign(true));

// ✓ 不报：调用可能有副作用
let g = fn() { puts("side effect"); 1; };

// ✗ constructor 的结果固定为 this，尾表达式值不会成为返回值
class Box { constructor() { 1 + 2; } }
```

#### `no-duplicate-hash-key`（error）

```
// ✗ 实测：程序正常运行、h["a"] 得 2，后写静默覆盖先写，没有任何报错——
//    正因为运行时毫无征兆，才需要静态查
let h = {"a": 1, "a": 2};

// ✓ 不报：非字面量 key 静态无法判定
let keyA = "a";
let keyB = "a";
let h2 = {keyA: 1, keyB: 2};
```

#### `builtin-arity`（error）

v0 **只检查 `len` 必须恰好取 1 个参数**，且仅当这个名字未被用户绑定遮蔽时
检查。`puts` 可变参不查；`print` 是 `puts` 的别名（`object/builtins.rs` 中复用
同一个函数），同样不查。

不能把 `first`/`last`/`rest`/`push` 放进同一条确定性规则：GC VM 会严格检查它们
的参数个数，而 interpreter 实现会忽略多余参数，缺少参数时甚至可能 panic。例如
interpreter 中 `first([1], [2])` 得 `1`、`rest([1], [2])` 得 `[]`、
`push([1], 2, 3)` 得 `[1, 3]`。这些调用应先归入 v1 的
`backend-divergent-builtin-arity`，或先统一 runtime 行为后再扩展本规则。

```
// ✗ 两个后端都产生 "builtin len expected 1 argument, got 2" 错误对象/值；
//    这不等同于 runner 报 runtime failure，也不保证立即中止执行。
let n = len([1], [2]);

// ✓ 不报：len 已被用户遮蔽（改由 no-shadowed-builtin 报警）
let len = fn(a, b) { 42; };
let m = len([1], [2]);
```

#### `no-shadowed-builtin`（warn）

```
// ✗ 遮蔽 builtin，此后 len 不再是内置函数
let len = 3;
```

全集 7 个名字：`len`、`puts`、`first`、`last`、`rest`、`push`、`print`。

#### `no-constant-condition`（warn）

v0 只检查 Boolean/Integer/String 这类 truthiness 可由语法直接确定的标量字面量
条件，不把可能包含待求值子表达式的 Array/Hash 算进去，也不做一般常量折叠。

```
// ✗ 字面量条件，分支恒定
if (true) { puts("always"); }

// ✓ 不报：可折叠但不是字面量——常量折叠是 minifier 的职责，linter 不越界
if (1 < 2) { puts("ok"); }
```

#### `no-literal-type-mismatch`（error）

只报两个操作数都是字面量、且选定 oracle 确认 interpreter 与 GC VM 都拒绝的
组合；运算类型表按实测行为定，不按想象定。测试要分别断言各后端的实际错误形态，
不能把 error object/value 与 classified runtime failure 混为一谈。

```
// ✗ 实测 interpreter 与 GC VM 都拒绝这些运算：
let a = 1 + "a";       // unsupported binary operation for 1 and a
let b = true + 1;
let c = "a" - "b";

// ✓ 不报：1 == true 两个后端行为分歧——interpreter 返回 false，
//    GC VM 报 unsupported comparison（实测）。不满足"必然 error"，
//    归 v1 的 backend-divergent-comparison。
let d = 1 == true;
```

### v1 候选

- `backend-divergent-rebinding`（招牌规则，仓库特有）：同一 scope 内 rebind
  一个名字，且旧绑定被两次绑定之间创建的闭包捕获。实测确认分歧：

  ```
  let x = 1;
  let f = fn() { x; };
  let x = 2;
  puts(f());   // interpreter 输出 2，GC VM 输出 1
  ```

  interpreter 的闭包共享 environment，rebind 对已创建的闭包可见；编译后端在
  编译期就把 `f` 体内的 `x` 解析到旧 slot。同一段代码的行为取决于跑在哪个
  后端上，写出来就该 warn。依赖 scope.ts 的捕获分析，v0 打好地基 v1 上。

- `backend-divergent-comparison`：比较运算的后端分歧不只发生在跨类型字面量。
  `1 == true` 会得到 interpreter → `false`、GC VM → runtime error；array/hash
  等同类型值的比较也可能分歧。与
  `no-literal-type-mismatch` 互补：后者只收"两后端都 error"的组合，
  分歧组合归这条。
- `no-self-compare`：把 `x == x` / `x != x` 作为高度可疑的自比较报告，但不宣称
  恒真/恒假。例如 `let x = []; x == x` 在 interpreter 中是 `true`，GC VM 则拒绝
  该比较。
- `backend-divergent-builtin-arity`：在 runtime 行为统一前，报告
  `first`/`last`/`rest`/`push` 的错误参数个数。
- `no-empty-block`：`if (ready) {}` 空分支。

### 明确不做的规则

- 命名风格、行长、缩进类（formatter 职责）；圈复杂度类主观度量。
- `no-duplicate-method`：曾列为候选，实测 class 内重名 method 已是 parse 期
  硬错误。检查发生在 parser 的 `parse_class_declaration()`，内部消息是
  `duplicate method A.m`，wasm API 当前会包装成 `parse error: ...`；不是 validation
  层规则，linter 无需重复。

## CLI

```
monkey-lint <files...>
  --format pretty|json     # pretty 默认：路径:行:列 级别 规则 消息 + 源码行标注
  --rule <name>:<level>    # 覆盖单条规则，如 --rule no-unused-let:off
  --deny-warnings          # warn 也导致非零退出
```

退出码：有 error 诊断 → 1；`--deny-warnings` 时有任意诊断 → 1；否则 0。
JSON 格式给编辑器集成与脚本消费。

## 测试与正确性

- **规则单测**：每条规则一组 `.monkey` fixture（命中 / 不命中 / 边界），断言
  Vitest 快照——与仓库 Rust 侧 insta 快照文化对应。
- **corpus 冒烟**：`examples/*.monkey` 与各包现有 fixture 全量过 linter，输出
  进人工审阅的快照。作用是让规则漂移与意外假阳性可见，不要求零诊断：例如
  `examples/hello.monkey` 中已有未使用绑定，会被 `no-unused-let` 合理报告。
- **逐规则双后端 oracle**：`builtin-arity` 的 fixture 断言两个后端都产生对应的
  error object/value；`no-literal-type-mismatch` 则分别断言 interpreter 与 GC VM
  的实际拒绝路径。oracle 明确区分 error object/value、classified runtime failure
  与静默错误结果，不用笼统的“报错”代替。`1 == true` 是 divergence fixture，
  不能放进要求两个后端一致的 type-mismatch 表。
- **与 minifier DCE 交叉验证**（可选、后置）：minifier dead-let elimination
  与 linter 对同一份**当前 AST** 做同一轮 scope analysis 时，DCE candidate 应被
  `no-unused-let` 判为 unused。不要比较完整的 fixed-point 删除集：例如
  `let a = 1; let b = a;` 初始只报告 `b`，删除 `b` 后下一轮才会让 `a` 变成 dead。

## 集成

1. **CI**（`rust.yml` build job，照 minifier 的 step 形态），分别运行：

   ```sh
   pnpm --filter @gengjiawen/monkey-lint build
   pnpm --filter @gengjiawen/monkey-lint test
   ```

2. **playground**：`Editor.tsx` 挂 `@codemirror/lint` 的 `linter()` +
   `lintGutter()`，编辑时实时出 squiggle（防抖用已有 lodash.debounce），span
   映射先复用现有 `sourceSpan.ts` 的 byte→UTF-16 转换；供 VS Code 使用时再提取
   成共享工具或保留等价 adapter。不要新开 pane——诊断属于编辑器本体。
3. **VS Code extension**：现有实现只调用 `mod.parse()`，失败时把整条诊断标在
   `(0, 0)..(0, 1)`；接入后改用 analyzer。分析失败时输出一条 parse/validation
   error，成功时才输出 lint diagnostics，不在失败 AST 上继续 lint。linter 暴露
   可注入已加载 analyzer 的内部入口（如 `lintWithAnalyzer()`），复用 extension
   已创建的 wasm 实例，避免 bundle 内再实例化一份。
4. **发布**：仓库使用单一的根 release-please release，不为 linter 新建独立
   release-please package 配置。在 `.github/workflows/release-please.yml` 增加 linter
   的 build/npm publish steps，并显式扩展 `scripts/bump_cargo_packages.ts`，同步
   linter 版本及其 `@gengjiawen/monkey-wasm` 依赖范围。

## 演进路线

- **v0**：结构化 wasm analyzer API、包骨架、walker/scope、上表 9 条规则、CLI、
  测试与 CI。
- **v1**：playground 与 VS Code extension 接入；`backend-divergent-rebinding`
  与 `backend-divergent-builtin-arity` 等进阶规则。
- **v2**：行内禁用指令（`// monkey-lint-disable-next-line <rule>`——
  `parse_lossless` 不含注释，需对原始 source 做行级扫描，不改 parser）；
  `--fix`（基于 span 的文本编辑，不走 printer，避免重排无关代码）。

## 开放问题

已确认 `print` 是 `puts` 别名；比较与 builtin arity 都存在后端分歧，结论已落在
对应规则说明里。剩余：

- extension 打包 linter 后体积影响（wasm 已在 bundle 里，linter 纯 JS，预计
  可忽略；打包时确认与现有 wasm 实例复用同一份，避免双实例）。
- `no-unused-expression` 对 index/property 访问的保守豁免要不要在 v1 收紧：
  MethodKind 只有 Constructor/Method、没有 getter，property 访问本身无副作用，
  只是可能 runtime error——届时看误报数据再定。
- runtime 是否先统一 `first`/`last`/`rest`/`push` 的 arity 行为；若统一，相关检查
  可从 divergence 规则并回 `builtin-arity`。
- AST 类型在第三份实现稳定后是否抽成 `@gengjiawen/monkey-ast`，以及由哪个包
  负责生成/维护类型。
