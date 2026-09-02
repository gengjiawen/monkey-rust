# Monkey Playground Debugger（`debugger` 关键字 + 栈/堆检视）— 设计提案

> 状态：已实施（§10 未来方向除外）。本文设计一个教学用 debugger：给 Monkey 语言加入
> `debugger` 关键字，程序在 GC VM 上执行到该语句时记录一份“调用栈 + 堆”的
> 快照。playground 新增 Debugger tab，并排展示帧内命名槽位与堆对象；槽位中的
> 引用以 `ref #n` chip 呈现，hover/click 时联动高亮对应堆节点。
>
> 关联设计：[compiler-debug-info-design.md](./compiler-debug-info-design.md)
> （pc→span 映射，本文在其上扩展变量和 capture 名称）、
> [bytecode-snapshot-design.md](./bytecode-snapshot-design.md)（`.mbc` 格式，
> DebugInfo 扩展需要 bump 版本）、
> [gc-playground-trial-deletion-teaching-plan.md](./gc-playground-trial-deletion-teaching-plan.md)
> （GC tab 与 Mermaid 堆图的教学先例）。
>
> 命名说明：本文的“快照”指 debugger 命中时的运行时状态记录
> （`DebuggerHit`），与 `.mbc` bytecode snapshot、insta 测试 snapshot 无关。

## 处理管线

```text
source ──lexer/parser──▶ AST: Statement::Debugger { span }
         ──compiler──▶ bytecode: OpDebugger
                       + DebugInfo { pc_spans, local_bindings, free_names }
         ──GcVM.run──▶ 每次执行到 OpDebugger：
                        copy-out DebuggerHit { frames, globals, heap }（不持有 GcRef）
         ──wasm run_gc_with_debugger──▶ tagged JSON envelope
                                         │
                             playground Debugger tab
                             ├─ 左：调用栈、局部变量、captures、globals
                             ├─ 右：有预算上限的堆图
                             └─ ref chip ↔ 堆节点联动高亮；Hit 1/N 前后翻页
```

## 1. 背景、目标与非目标

playground 目前能展示 AST、bytecode、GC 回收报告、`.mbc` 快照与 ARM64 汇编，
但没有任何视图能回答教学中最常见的问题：**函数调用发生时，参数和局部变量
放在哪？复合值放在哪？两者怎么关联？**

GC tab 只在程序结束后展示一次回收报告；bytecode tab 是静态反汇编。缺的是
“程序执行到某一时刻”的运行时状态切面。

目标：

- 语言加入 `debugger;` 语句。它具有**透明 completion**：除 GC VM 记录快照外
  没有运行时效果，加入或删掉它不改变程序、块、函数的结果或 stdout。
- 在 GC VM 上，每次命中 `debugger` 记录有明确预算上限的状态：调用栈各帧的
  callee、命名局部变量、captures、当前帧临时栈、全局变量与堆对象图。
- playground 新增 **Debugger** tab：命中点之间前后翻页，左侧调用栈、右侧
  堆图；引用 chip 与堆节点联动高亮，点击命中或帧可高亮源码。
- interpreter、两个 VM、asm 后端、Prettier、minifier、linter 和 VS Code
  extension 都显式处理新语法。

非目标（见 §10 未来工作）：

- 真正的断点暂停/继续/单步（pause–resume 会话）。v0 是**录制式**：一次同步
  执行录下所有命中点，事后浏览。
- 条件断点、按行打断点（只支持显式 `debugger;` 语句）。
- interpreter（树遍历求值器）的状态检视——它没有可枚举的栈和堆。
- 从 `.mbc`、stripped bytecode 或 Snapshot tab 启动 Debugger；Debugger tab
  始终从当前源码现场编译。
- 从左侧 DOM 槽位跨栏画到右侧 SVG 节点的实体连线。v0 只做引用 chip 与节点
  高亮，避免把滚动、resize、fullscreen 坐标同步引入首版。

## 2. 核心决策

### 2.1 录制式而非暂停式

wasm 侧当前所有执行入口都是同步函数（`run_gc_with_report` 等）。真暂停需要
把 `GcVM` 装进一个可跨 JS 调用存活的 wasm-bindgen session 对象，并处理
“源码一改 session 就作废”的生命周期问题。录制式则完全复用现有模式：

- 一次 `run_gc_with_debugger(source)` 调用返回 envelope，无状态残留；
- playground 的 10,000 指令预算（`PLAYGROUND_GC_INSTRUCTION_BUDGET`）限制
  执行成本；命中、对象、边与摘要另有独立预算；
- 录制结果可以前后翻页，比单向 continue 更适合教学。

若未来要做 live 单步，再引入持久 session；本文的 copy-out 数据结构仍可复用。

### 2.2 跑在 GC VM 上

只有 `gc` crate 的 VM 有真实可枚举堆（`GcHeap` + `GcId`）；compiler VM 的
“堆”是 Rust `Rc`，无法枚举。playground 现有 GC runner、错误分类 envelope
和 Mermaid 堆图也都在 GC VM 路径上。

Compiler VM 与 asm 后端仍必须认识该语法，保证 `debugger` 不改变普通执行
结果；只有 GC VM 在 `OpDebugger` 上额外记录快照。

### 2.3 透明 completion

`debugger` 是语句，不产生新的 completion，也不清空之前的 completion。
可以把它理解为 StatementList 中的“透明项”：

| 源码                               | 结果   |
| ---------------------------------- | ------ |
| `1; debugger;`                     | `1`    |
| `if (true) { 1; debugger; }`       | `1`    |
| `fn() { 1; debugger; }();`         | `1`    |
| `debugger;`                        | `null` |
| `fn() { let x = 1; debugger; }();` | `null` |

这不同于“让 `debugger` 返回 `Null`”：后者会在 interpreter 中覆盖 block result，
也会让 compiler 在尾部补 `OpNull`，从而违反“删掉 debugger 结果不变”。

### 2.4 标量内联、用户可见对象建节点

该 GC VM 里所有值都经 `alloc_value` 进入堆，包括 Integer。若全部建节点，堆图
会被标量和 VM 基础设施淹没。Debugger 的投影按用户教学语义分类：

| 类别        | Kind                                                              | 呈现方式                                             |
| ----------- | ----------------------------------------------------------------- | ---------------------------------------------------- |
| 内联值      | Integer、Boolean、Null、Builtin                                   | 在槽位或父对象 member 中显示；`heap_id = None`       |
| 用户对象    | String、Array、Hash、Closure、Class、Instance、BoundMethod、Error | 建堆节点；槽位显示 `ref #n`                          |
| VM 基础设施 | CompiledFunction、Other                                           | 不建节点；函数名等必要信息并入拥有它的 Closure label |

数组元素、哈希值、实例字段如果是内联值，写入父节点的 `members`；如果是用户对象，
生成堆内边。只有两端都进入节点表的边才能出现在 `HeapView.edges`，JSON 中不得有
dangling edge。

### 2.5 copy-out、确定性 GC 与对象身份

`DebuggerHit` 在命中时立即物化为字符串、整数和普通 Vec，不保存任何 `GcRef`。
否则快照会额外 root 对象并改变后续 GC 行为。

Debugger runner 使用 fresh VM，并与 report runner 一样把自动 GC threshold 设为
`usize::MAX`。这样同一源码的命中视图不随分配阈值漂移；尚未被显式 cycle
collection 清掉的无根环可能仍出现在堆中，这是“当前已分配堆”的真实状态。

`GcId` 是可复用的 slot id：它在单个 hit 内唯一；对象未释放时跨 hit 保持相同
ID，但不能仅凭两个 hit 中数字相同就断言是同一次分配。切换 hit 必须清除 UI
选择；未来做 hit diff 前需引入 allocation generation/serial。

## 3. 语言层与 JS 工具链

### 3.1 Rust 语法与 interpreter

| 文件                   | 改动                                                                                                        |
| ---------------------- | ----------------------------------------------------------------------------------------------------------- |
| `lexer/token.rs`       | `TokenKind::DEBUGGER`；`lookup_identifier` 加 `"debugger"`；`Display` 输出 `debugger`                       |
| `parser/ast.rs`        | `Statement::Debugger(DebuggerStatement)` 与 `{ span: Span }`；同步所有 `Display`、`span()` exhaustive match |
| `parser/lib.rs`        | `parse_statement` 加分支：吃掉 token、可选分号，产出语句                                                    |
| `parser/validation.rs` | statement match 加空分支；它在任何语句位置合法                                                              |
| `interpreter/lib.rs`   | `eval_statement` 可返回 `Null` 作为不可观察的直接结果，但 block/program loop 遇到该 variant 时不覆盖 result |

`debugger` 只能出现在语句位置；`let x = debugger;`、`return debugger;` 是 parse
error。一个只含 debugger 的空 completion block 最终仍按现有规则得到 `null`。

新增关键字会使旧代码不能再把 `debugger` 用作 identifier，这是有意的语言兼容性
变化，需要在 README/变更说明中记录。

### 3.2 JS/TS AST 消费者

不能只补类型和顶层 walker。所有 statement dispatcher 都要有显式
`DebuggerStatement` 分支，避免 default 分支把它误当 expression：

| 包                                                          | 改动                                                                                                                          |
| ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `packages/prettier-plugin-monkey/src/`                      | AST 类型和 printer 输出 `debugger;`，保证格式化幂等                                                                           |
| `packages/monkey-minifier/src/`                             | types/node/printer、scope、fold、propagate、statement traversal/classification 全部处理；把 debugger 视为不可删除的副作用语句 |
| `packages/monkey-linter/src/`                               | types/walk、scope、statement-list rules（含 `no-unused-expression`）显式跳过；v0 不新增诊断                                   |
| `packages/vscode-extension/syntaxes/monkey.tmLanguage.json` | keywords 加 `debugger`                                                                                                        |
| `packages/vscode-extension/src/extension.ts`                | wasm module 类型缩成 extension 实际使用 exports 的 `Pick<>`，避免未来新增 wasm export 破坏手写返回对象                        |
| playground `AstTreeView`                                    | 通用 JSON 树，无需改动                                                                                                        |

## 4. 编译层与 DebugInfo

### 4.1 `OpDebugger` 与各后端 completion

`compiler/op_code.rs` 将 `OpDebugger` **追加在枚举末尾**，保持既有 opcode 字节
值不变；它无操作数，并加入 `DEFINITIONS`。

- Compiler 在 `Statement::Debugger` 上 `emit(OpDebugger)`，通过既有
  `DebugInfo::add_pc_span` 记录该语句 span。
- 普通 program 顺序编译即可；`OpDebugger` 不改变 VM operand stack 或
  `last_popped`。
- expression block 与函数体不能再仅检查“最后 opcode 是否为 `OpPop`”。编译器
  按 AST 找到最后一个非 debugger 语句，并先编译它及尾部所有 debugger。若该
  语句贡献值，expression block 让值跨过尾部 `OpDebugger` 留在栈上，函数/普通
  方法则在 debugger 之后追加 `OpReturnValue`；若它不贡献值，expression block
  在 debugger 之后追加 `OpNull`，函数/普通方法追加 `OpReturn`。构造器仍在执行
  尾部 debugger 后按既有规则返回 `this`。if 的每个分支使用同一 helper；只有
  debugger 的 block 走“不贡献值”路径。
- 非 GC `compiler/vm.rs` 的 `OpDebugger` 只前进 ip，无栈效果。
- GC VM 的 `OpDebugger` 在当前 ip 采集快照后继续，无栈效果。
- `asm/lower.rs` 将 `lower_statement -> bool` 改为
  `StatementCompletion::{Value, Empty, Transparent}`。program/block 只在 Value
  或 Empty 时更新 completion；Debugger lower 成注释
  `// debugger (no-op in AOT build)` 并返回 Transparent，不发射 `brk` 或指令。

### 4.2 Definition ledger 与变量名

`SymbolTable.symbols` 继续负责按当前可见名字解析；另加按定义顺序保存的 ledger。
每次 `define` 同时追加 symbol，因此同名重绑定不会丢失旧 slot。ledger 满足：

- entry index 与 symbol slot 一致；
- 参数（方法含 `this`）在前，函数体 `let` 按编译顺序在后；
- globals 和 locals 都按 slot 输出，不按名字排序；
- UI 允许同名多次出现，通过 `[slot n]` 区分。

DebugInfo 扩展为：

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingDebugInfo {
    pub name: String,
    pub slot: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugInfo {
    pub pc_spans: Vec<PcSpan>,
    /// 参数、this 与所有 let；严格按 slot 递增。main 级为空。
    pub local_bindings: Vec<BindingDebugInfo>,
    /// 与 GcClosure.free / OpGetFree index 对齐。
    pub free_names: Vec<String>,
}
```

函数显示名不进入 DebugInfo：`object::CompiledFunction.name` 已保存普通函数名与
`Class.method` 名称。main 显示 `main`，空函数名显示 `<anonymous>`。匿名函数仍可
用当前帧的 `current_span` 定位执行位置，不承诺额外的定义处 span。

函数编译离开 scope 前，从当前 ledger copy `local_bindings`，从
`free_symbols` 按 capture index copy `free_names`。全局绑定由最外层 ledger
生成，并通过改名后的 `GcVM::set_global_bindings(Vec<BindingDebugInfo>)` 传入；
现有 `global_names` 字段和 setter 一并改名，不能再从最终 HashMap 反推。

### 4.3 `.mbc` v2

DebugInfo wire format 增加 local bindings 与 free names，因此：

- `FORMAT_VERSION` 从 1 bump 到 2；reader 仍只接受当前版本。
- 每个 DebugInfo 依次写 `pc_spans`、按 slot 排序的 `local_bindings`、按 capture
  index 排序的 `free_names`。
- function DebugInfo reader 校验 local slot 严格递增、唯一，且必须小于
  `num_locals`；编译器产生的完整 metadata 数量等于 `num_locals`。
- bytecode 校验阶段检查每个 `OpClosure(constant_index, num_free)` 的 `num_free`
  与对应 `free_names.len()` 一致；同一函数常量若被不一致地构造则拒绝。
- main DebugInfo 的 `local_bindings`、`free_names` 必须为空；字符串和 entry count
  沿用 snapshot reader 的“不得超过剩余输入”资源约束。
- `strip_debug` 继续移除整个 debug section；Snapshot tab 的 layout 注解同步
  描述新字段。

Debugger tab 不读取 `.mbc`，因此不存在“strip 后退化为无名槽位”的 UI 路径。
保留 v2 metadata 是为了 Bytecode/DebugInfo round-trip 完整性及未来消费者。

## 5. 运行时快照

### 5.1 对外数据结构

建议在 `gc/debugger.rs` 定义以下 copy-out 类型：

```rust
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebuggerHit {
    pub index: usize,              // 从 1 起
    pub span: Option<Span>,        // 当前 debugger; 的 pc→span
    pub frames: Vec<FrameView>,    // [0] = main，末尾 = 当前帧
    pub globals: Vec<SlotView>,
    pub heap: HeapView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameView {
    pub name: String,
    pub current_span: Option<Span>,
    pub callee: Option<ValueView>, // main 为 None；其余为 base_pointer - 1
    pub locals: Vec<SlotView>,
    pub captures: Vec<CaptureView>,
    pub temporaries: Vec<StackSlotView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub name: String,
    pub slot: usize,
    pub initialized: bool,
    pub value: Option<ValueView>, // initialized=false 时必须为 None
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureView {
    pub name: String,
    pub index: usize,
    pub value: ValueView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackSlotView {
    pub slot: usize,              // VM stack 的绝对 slot
    pub value: ValueView,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueView {
    pub kind: ValueKind,
    pub display: String,
    pub heap_id: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapObjectView {
    pub id: usize,
    pub kind: ValueKind,
    pub label: String,
    pub members: Vec<HeapMemberView>, // 被内联的标量元素/字段
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeapView {
    pub objects: Vec<HeapObjectView>,
    pub edges: Vec<HeapEdgeView>,
    pub omitted_objects: usize,
    pub omitted_edges: usize,
}
```

`HeapMemberView` 保存结构关系及有界 display；`HeapEdgeView` 保存 from/to id 与
既有 `EdgeRelation`。所有集合使用上述顺序输出，保证测试和 UI 稳定。

### 5.2 初始化状态

预填充为 Null 的栈槽不能同时表示“尚未执行声明”和“变量真实值为 null”。因此：

- `Frame` 增加与 `num_locals` 等长的 initialized bitset；创建 frame 时将前
  `num_parameters` 项置为 true（方法的 `this` 已计入参数），其他为 false；
- `OpSetLocal` 成功写入后置位对应 bit；
- `GcVM` 增加 global initialized bitset，`OpSetGlobal` 成功写入后置位；
- snapshot 对所有 debug metadata 中的槽位输出 `SlotView`。未初始化时不读取
  预填充 Null 作为用户值，而是输出 `initialized=false, value=None`；
- `load_bytecode` 重建 frame bitset 并清空 debugger 命中；为匹配 REPL，已有
  globals 及其 initialized bit 保留。

名称来自编译期，bitset 和 set opcode 上的一次置位是该功能新增的少量运行时
开销；文档不再声称变量展示完全零运行时开销。

### 5.3 帧、roots 与堆采集

`OpDebugger` 命中时按下列步骤采集：

1. 未达 `MAX_DEBUGGER_HITS` 时继续；否则只递增 `dropped_hits`。
2. 帧按 main→current 输出。非 main 的 callee 从 `base_pointer - 1` 读取；locals
   按 `local_bindings` slot 输出；captures 从 `frame.cl.free` 按 index 与
   `free_names` 配对。
3. 只有当前帧输出 `[base + num_locals, sp)` temporaries，且保留绝对 stack slot；
   suspended caller 的 temporaries 输出空 Vec，避免把 callee/arguments 错归类。
4. Globals 按 definition ledger 的 slot 顺序输出，包含同名重绑定和未初始化项。
5. 用户对象 roots 的稳定顺序为：globals；每个 frame 的 callee、locals、
   captures；最后是当前 frame temporaries。先从 roots 做确定性 BFS，再按 GcId
   加入尚未访问的用户对象，直到对象预算。
6. 对入选对象枚举边：标量目标写入 source node members；用户对象目标只有在
   两端节点均入选时才输出 edge。按稳定的 `EdgeRelation` 顺序截断并累计 omitted。
7. 将完全 copy-out 的 `DebuggerHit` 推入 VM，继续执行下一条指令。

有界摘要必须在构造过程中限深、限元素、限字符，不能先构造完整字符串再截断。
执行这一约束的 `BoundedText` 现位于 `gc/display.rs`：`value_to_string` 用同一个
构建器，但预算大得多（`MAX_VALUE_DISPLAY_CHARS` 64 KiB / `MAX_VALUE_DISPLAY_DEPTH`
64，见 `gc.md` §7.5），因此调试器仍需下面这组更紧的常量，不能改调 `value_to_string`。
默认常量：

```rust
const MAX_DEBUGGER_HITS: usize = 25;
const MAX_DEBUGGER_OBJECTS: usize = 100; // 每个 hit
const MAX_DEBUGGER_EDGES: usize = 250;   // 每个 hit
const MAX_DEBUGGER_DISPLAY_CHARS: usize = 64;
const MAX_DEBUGGER_SUMMARY_DEPTH: usize = 2;
const MAX_DEBUGGER_MEMBERS: usize = 8;   // 每个容器/对象
```

若 root 指向因预算未收录的对象，`ValueView` 仍保留 `heap_id`；UI 将其标成
“未收录”，但不会生成不存在端点的 edge。

`omitted_objects` 只统计因对象预算未收录的“用户对象”；`omitted_edges` 统计因
对象/边预算未输出的、原本连接两个用户对象的投影边。按设计内联的标量以及被
有意隐藏的 CompiledFunction/Other 不计入 omitted，避免把呈现策略误报成截断。

### 5.4 Runner outcome 与生命周期

`GcVM` 对外提供：

```rust
pub fn take_debugger_hits(&mut self) -> (Vec<DebuggerHit>, usize /* dropped */)
```

`load_bytecode` 清空 hits、dropped count 和旧 frame 初始化状态，避免 REPL 后续
bytecode 读到历史命中；global values/init bits 按现有 REPL 生命周期保留。

Runner 不能复用会在 `map_err` 时丢弃 VM 的普通 `Result` 形态：

```rust
pub enum GcDebuggerRunOutcome {
    Ok {
        result: String,
        stdout: String,
        hits: Vec<DebuggerHit>,
        dropped_hits: usize,
    },
    Error {
        error: GcClassifiedRunError,
        stdout: String,
        hits: Vec<DebuggerHit>,
        dropped_hits: usize,
    },
}

pub fn run_source_with_debugger_classified(
    input: &str,
    instruction_budget: usize,
) -> GcDebuggerRunOutcome
```

执行路径为：parse → compile → 从 compiler ledger 取得 global bindings → fresh
`GcVM` → `set_global_bindings` →
`set_capture_output(true)` → `set_gc_threshold(usize::MAX)` → run。无论 VM run 成功
还是 runtime error，都先读取 result/error，再 drain stdout 与 hits，最后构造
outcome。parse/compile error 发生在 VM 创建前，返回空 stdout/hits 和 dropped=0。

指令预算沿用 `PLAYGROUND_GC_INSTRUCTION_BUDGET`（10,000）。execution limit
仍返回已录 hits，形成“跑到一半仍可检视”的体验。

## 6. wasm API

`wasm/src/lib.rs` 新增：

```rust
#[wasm_bindgen]
pub fn run_gc_with_debugger(input: &str) -> String
```

它把 `GcDebuggerRunOutcome` 映射成与现有 runner 风格一致的 tagged JSON：

```jsonc
// 成功
{
  "status": "ok",
  "result": "3",
  "stdout": "",
  "hits": [
    {
      "index": 1,
      "span": { "start": 42, "end": 51 },
      "frames": [],
      "globals": [],
      "heap": {
        "objects": [],
        "edges": [],
        "omittedObjects": 0,
        "omittedEdges": 0
      }
    }
  ],
  "droppedHits": 0
}

// 失败；runtime error/limit 保留此前命中与 stdout
{
  "status": "error",
  "stage": "runtime",
  "kind": "executionLimit",
  "message": "...",
  "span": { "start": 52, "end": 60 },
  "stdout": "...",
  "hits": [],
  "droppedHits": 0
}
```

该 API 始终从源码现场编译，不接受 snapshot bytes，也不走 `run_snapshot`。

## 7. Playground UI

### 7.1 接线与数据校验

- `OutputView` 加 `'debugger'`，SegmentedControl 加 **Debugger**。
- 状态机为 `idle | running | ok | error`，使用 request id 防竞态；源码变化后回
  `idle`，切换 hit 时清除 hover/pinned heap id。
- toolbar 在该 tab 下显示 **Run**，调用新的 wasm runner。
- `debuggerReport.ts` 对 envelope 做运行时校验：status/stage/kind、hit index、
  slot/index、initialized/value 不变量、object id 唯一、edge 两端存在及 omitted
  count 非负。无效 JSON 进入 error state，不部分渲染。

建议文件职责：

| 文件                                 | 职责                                                                |
| ------------------------------------ | ------------------------------------------------------------------- |
| `src/debuggerReport.ts`              | envelope 类型与运行时校验                                           |
| `src/debuggerRunner.ts`              | `run_gc_with_debugger` 包装                                         |
| `src/DebuggerView.tsx`               | hit/frame/slot/capture 状态、源码联动、左右布局                     |
| `src/DebuggerHeapGraphView.tsx`      | Debugger heap → Mermaid source、40-node 选择和高亮 class            |
| `src/MermaidGraphCanvas.tsx`         | 从现有 HeapGraphView 抽出的 render/theme/error/copy/fullscreen 画布 |
| `src/test/debuggerReport.test.ts` 等 | parser、视图、翻页与联动测试                                        |

`HeapGraphView` 保留 GC report adapter，只把 Mermaid 渲染画布下沉，不强行把
trial-deletion fate/decision 数据改造成一个虚假的通用 node schema。

### 7.2 布局与交互

```text
┌─ Hit 2 / 3  ◀ ▶ ───────── span: 高亮对应 debugger; ─────────────┐
│ ┌─ 调用栈 ─────────────────┐  ┌─ 堆 ─────────────────────────┐ │
│ │ ▸ makePoint（当前）       │  │ #7 Array                     │ │
│ │   callee: ref #5          │  │   [0] = 3                    │ │
│ │   x: 3        [slot 0]    │  │   [1] = 2       ◀ highlighted│ │
│ │   y: 2        [slot 1]    │  │ #9 Instance(Node)            │ │
│ │   p: [ref #7] [slot 2]    │  │                              │ │
│ │   captures (1)            │  └──────────────────────────────┘ │
│ │ ▸ sum                                                        │
│ │ ▸ main                                                       │
│ │ ─ globals ─  later: <uninitialized> [slot 4]                 │
│ └───────────────────────────┘  stdout（若有）                  │
└────────────────────────────────────────────────────────────────┘
```

- **翻页**：前后切换 hits；切换后高亮命中的 `debugger;` span，并清除 heap 选择。
- **帧联动**：点击帧标题高亮该帧 `current_span`。
- **引用联动**：hover `ref #n` 临时高亮节点；click 固定高亮，直到再次点击、选择
  其他引用或切换 hit。通过 Mermaid `class` 指令重新生成 source，不查询 Mermaid
  生成的 DOM id。
- **未收录引用**：若 heap id 不在 `heap.objects`，chip 保留但禁用高亮并显示
  “对象因快照预算未收录”。
- **槽位状态**：未初始化显示 `<uninitialized>`；真实 Monkey Null 显示 `null`。
- **callee/captures/temporaries**：callee 和 captures 各自分组；temporaries 默认
  折叠为“操作数栈临时值 (n)”，只有当前帧可能非空。
- **空态**：成功但 hits 为空时提示“源码里没有执行到 debugger 语句”。
- **截断提示**：显示 `droppedHits`、`omittedObjects`、`omittedEdges`，不静默省略。

Debugger heap graph 沿用 `MAX_GRAPH_NODES = 40`。节点选择优先级为：当前高亮
节点（若已收录）→ visible roots 的 BFS → 其余已收录对象按 id。只画两端都进入
这 40 个节点的边；客户端计算并显示 `droppedGraphNodes`/`droppedGraphEdges`，
同时展示 runtime 的 omitted counts。这是明确标注的 UI 可读性投影，不冒充
完整拓扑。

### 7.3 示例 snippet

```monkey
let makePoint = fn(x, y) {
  let p = [x, y];
  debugger;
  p;
};

let sum = fn(a, b) {
  let total = a + b;
  let point = makePoint(total, b);
  debugger;
  total;
};

sum(1, 2);
```

- Hit 1：`main → sum → makePoint`；`x`、`y` 是内联标量，`p` 的 `ref #n`
  高亮 Array 节点，节点 members 显示 `[3, 2]`。
- Hit 2：`makePoint` 已出栈，`sum.point` 仍引用该存活 Array。相同 id 在对象未
  释放时保持稳定，但 UI 不把 id 相等推广成通用的跨 hit 分配身份。

## 8. 测试策略

| 层           | 必测场景                                                                                                                                                                   |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| lexer/parser | `debugger` token、`debuggerX` identifier、可选分号、span、AST JSON；expression/return 位置拒绝；AST `Display`/`span()`                                                     |
| interpreter  | `1; debugger;`、条件块尾、函数尾、只有 debugger、显式 return 后不可达 debugger；与删掉 debugger 的结果/stdout 一致                                                         |
| compiler     | `OpDebugger` + pc span；尾部 debugger 之后才生成 return/null；local ledger 保留参数、`this`、重名 let；free_names 与 capture index 对齐                                    |
| compiler VM  | `OpDebugger` 无栈效果；上述透明 completion 用例结果不变                                                                                                                    |
| asm          | `StatementCompletion::Transparent`；只发注释；与 interpreter/VM 对已对齐的 baseline 程序做差分                                                                             |
| snapshot     | v2 round-trip；v1 拒绝；strip 移除 metadata；slot 越界/重复/乱序、local count、free count 不匹配及恶意长度拒绝；layout 注解覆盖新段                                        |
| gc VM        | 命中数/顺序/span；main→current 帧序；callee、captures、绝对 temporary slot；参数、重名 binding、分支未初始化与真实 null；`load_bytecode` 清 hits 但保留 REPL globals init  |
| heap 投影    | 数组/字段标量内联、共享引用、Closure→CompiledFunction 过滤、所有 edge 端点存在；100 objects/250 edges、深度/成员/字符截断及 omitted counts；copy-out 不改变后续 GC report  |
| runner/wasm  | ok、parse/compile/runtime/limit envelope；runtime error 与 execution limit 保留此前 hits/stdout；parse/compile error 返回空 hits/stdout；fresh VM 禁用自动 GC              |
| playground   | envelope 不变量校验、翻页清除选择、hover/click 高亮、未收录引用、uninitialized vs null、空态/截断提示、40-node 投影；GC tab Mermaid 视觉、copy、fullscreen 和 theme 无回归 |
| JS 工具链    | Prettier 幂等；minifier 保留且可再 parse，所有 optimization pass 不崩；linter walker/scope/rules 不误报不崩；VS Code grammar 与缩窄 wasm binding 类型可构建                |

## 9. 实施切分

按依赖顺序五个 PR，每步独立可合、CI 全绿：

1. **语法与透明语义**：token、AST、parse/validation、interpreter、`OpDebugger`、
   compiler completion、两个 VM、asm tri-state，以及 Prettier/minifier/linter/
   VS Code 全部 AST 消费者。
2. **调试元数据**：SymbolTable definition ledger、DebugInfo local/free metadata、
   GC frame/global initialized bitset、`.mbc` v2 与 snapshot 安全校验。
3. **运行时与 API**：`gc/debugger.rs`、root-first 有界投影、VM 命中生命周期、
   explicit outcome runner、wasm export；完成 Rust/wasm 测试。
4. **Playground**：重建 `wasm/pkg`，新增 Debugger tab、report parser、共享 Mermaid
   canvas、40-node debugger graph、引用联动与 snippet；完成 vitest。
5. **文档收尾**：实现落地后把本文状态改为“已实施”，更新 README 语言特性与
   examples。`no-debugger`、live session、跨栏实体线和 hit diff 继续留在未来工作。

## 10. 未来工作

- **live 暂停/单步**：wasm-bindgen 持久 VM session，`continue` / `step`。
- **实体跨栏引用线**：若教学验证确有收益，单独设计统一 React/SVG renderer 或
  overlay SVG；必须覆盖 scroll、resize、font load、fullscreen 与坐标更新。
- **linter `no-debugger`** 与 minifier `dropDebugger` 选项，区分教学和生产语义。
- **原始引用模型开关**：展示 Integer 等所有 GcRef，衔接 GC tab 的对象计数。
- **命中点 diff**：先为分配增加 generation/serial，再展示相邻 hit 的新增与消亡。
