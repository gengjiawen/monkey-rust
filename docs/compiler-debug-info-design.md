# Monkey Compiler Debug Info 设计文档

> 本文说明 Monkey compiler 中源码位置到字节码 PC 的映射设计，并对比 QuickJS 的 `OP_source_loc` 临时 opcode 方案。

## 目录

1. [背景](#1-背景)
2. [目标与非目标](#2-目标与非目标)
3. [当前实现](#3-当前实现)
4. [数据结构](#4-数据结构)
5. [编译流程](#5-编译流程)
6. [与 QuickJS 的差异](#6-与-quickjs-的差异)
7. [测试策略](#7-测试策略)
8. [后续演进](#8-后续演进)
9. [WASM 与 Playground 集成](#9-wasm-与-playground-集成)

---

## 1. 背景

Monkey 的 lexer/parser 已经为 token 和 AST 节点保留了源码 `Span`：

```rust
pub struct Span {
    pub start: usize,
    pub end: usize,
}
```

这些 span 是源码 byte offset，而不是 line/column。compiler 原先只产出 `Instructions` 和 `constants`，没有保存字节码 PC 与源码位置的关系，因此后续要做 VM 错误定位、调用栈、编辑器诊断或断点时缺少基础数据。

QuickJS 的做法是：编译早期插入临时 `OP_source_loc` opcode，后续优化/resolve 阶段移除该 opcode，并生成独立压缩的 `pc2line` 表。最终 VM 执行的字节码不包含 `OP_source_loc`。

Monkey 当前 compiler 还没有类似 QuickJS 的多阶段 label resolve / bytecode finalize pass，因此第一版采用更直接的旁路表设计。

---

## 2. 目标与非目标

### 2.1 目标

- 为主程序字节码保存 `pc -> Span` 映射。
- 为函数常量内部字节码保存独立的 `pc -> Span` 映射。
- 不改变最终 VM 执行的 opcode 流。
- 不影响现有 compiler/vm 测试中对 `Instructions` 的断言。
- 为未来 line/column、stack trace、debugger、WASM API 暴露位置数据留接口。

### 2.2 非目标

- 当前不引入 `OpSourceLoc` 临时 opcode。
- 当前不实现压缩编码。
- 当前不把 byte offset 转换为 line/column。
- 当前不修改 VM 运行时错误模型。
- 当前不修改 `object::CompiledFunction` 结构，避免大量测试和对象层变更。
- 当前不实现源码 span 到 bytecode PC 的反向查询（Playground 仅支持 bytecode 行 → 源码高亮）。

---

## 3. 当前实现

当前实现位于 `compiler/compiler.rs`。

核心思路：

1. 编译每条有源码归属的指令时调用 `emit_with_span(op, operands, span)`。
2. `emit_with_span` 先正常 emit 原始 opcode，再把返回的 PC 和 AST span 写入当前 scope 的 `DebugInfo`。
3. `Bytecode` 除了 `instructions` 和 `constants` 外，额外携带主程序 `debug_info`。
4. 函数字面量进入独立 compiler scope，离开 scope 时同时返回函数内部 `Instructions` 和 `DebugInfo`。
5. 函数内部 debug info 不写入 `CompiledFunction`，而是保存在 `Bytecode.function_debug_info` 中，key 为函数常量在 constant pool 中的 index。

最终 VM 仍然只执行 `Instructions`，debug info 是旁路元数据。

---

## 4. 数据结构

### 4.1 `PcSpan`

```rust
pub struct PcSpan {
    pub pc: usize,
    pub span: Span,
}
```

表示从某个 bytecode PC 开始，对应源码中的某个 AST span。

### 4.2 `DebugInfo`

```rust
pub struct DebugInfo {
    pub pc_spans: Vec<PcSpan>,
}
```

提供两个关键操作：

- `add_pc_span(pc, span)`：追加一条 PC 到 span 的映射。如果连续记录的 span 相同，会跳过重复记录。
- `span_for_pc(pc)`：从后向前查找最后一个 `pc_span.pc <= pc` 的 span。

这种结构语义接近 `pc2line`：它记录的是位置变化点，而不是每个 byte 都记录一条位置。

### 4.3 `Bytecode`

```rust
pub struct Bytecode {
    pub instructions: Instructions,
    pub constants: Vec<Rc<Object>>,
    pub debug_info: DebugInfo,
    pub function_debug_info: HashMap<usize, DebugInfo>,
}
```

- `debug_info`：主程序指令的 PC 到源码 span 映射。
- `function_debug_info`：函数常量的 PC 到源码 span 映射。key 是 constant pool index。

### 4.4 `CompilationScope`

每个 compiler scope 维护自己的 `DebugInfo`：

```rust
struct CompilationScope {
    instructions: Instructions,
    last_instruction: EmittedInstruction,
    previous_instruction: EmittedInstruction,
    debug_info: DebugInfo,
}
```

函数编译时会 `enter_scope()` 创建新 scope，函数完成后 `leave_scope()` 返回该 scope 的指令和 debug info。

---

## 5. 编译流程

### 5.1 普通 opcode emit

原有 `emit` 保持不变：

```rust
pub fn emit(&mut self, op: Opcode, operands: &Vec<usize>) -> usize {
    let ins = make_instructions(op, operands);
    let pos = self.add_instructions(&ins);
    self.set_last_instruction(op, pos);
    pos
}
```

新增 `emit_with_span`：

```rust
pub fn emit_with_span(&mut self, op: Opcode, operands: &Vec<usize>, span: &Span) -> usize {
    let pos = self.emit(op, operands);
    self.add_pc_span(pos, span);
    pos
}
```

编译表达式和语句时，优先使用 AST 节点自己的 span：

- integer/string/boolean literal 使用 literal span。
- identifier load 使用 identifier span。
- infix/prefix 使用整个表达式 span。
- let/return 最终 store/return 指令使用 statement span。
- function call 使用 call expression span。

### 5.2 删除或替换指令时维护 debug info

compiler 中已有 peephole 行为，例如：

- `remove_last_pop()`
- `replace_last_pop_with_return()`
- `change_operand()`

当前实现中，`remove_last_pop()` 会同步删除被移除 PC 之后的 debug info：

```rust
self.scopes[self.scope_index]
    .debug_info
    .truncate_from_pc(last.position);
```

`replace_last_pop_with_return()` 不改变 PC，因此保留原 span。语义上这表示被替换后的 `OpReturnValue` 仍归属于原表达式 span。

`change_operand()` 只 patch operand，不改变 PC，因此无需调整 debug info。

### 5.3 函数常量

函数编译流程：

1. `enter_scope()` 创建函数内部 scope。
2. 编译函数 body。
3. 必要时把尾部 `OpPop` 替换为 `OpReturnValue`。
4. `leave_scope()` 返回 `ScopedInstructions { instructions, debug_info }`。
5. `instructions.data` 写入 `CompiledFunction`。
6. `debug_info` 写入 `Bytecode.function_debug_info[constant_index]`。
7. 外层 emit `OpClosure`，并把它映射到函数字面量 span。

这样可以避免修改 `object::CompiledFunction`，同时仍能找到函数内部 PC 对应的源码位置。

---

## 6. 与 QuickJS 的差异

### 6.1 QuickJS 方案

QuickJS 的流程可以概括为：

```text
parse/compile
  -> emit normal opcodes + OP_source_loc
  -> optimize / resolve labels
  -> remove OP_source_loc
  -> generate compressed pc2line table
  -> final bytecode + debug table
```

特点：

- `OP_source_loc` 是临时 opcode。
- debug 信息最初内嵌在 bytecode 流中。
- 后续 pass 会删除临时 opcode。
- 删除临时 opcode 后需要重新计算或修正 PC、jump offset、label 地址。
- 最终 VM 不执行 `OP_source_loc`。

### 6.2 当前 Monkey 方案

当前 Monkey 流程：

```text
parse/compile
  -> emit normal opcodes
  -> record pc -> Span side table
  -> final bytecode + debug table
```

特点：

- 没有临时 opcode。
- debug 信息从一开始就是旁路表。
- 不需要删除 debug opcode。
- 不需要额外 patch jump 地址。
- 实现更小，适合当前单 pass compiler。

### 6.3 取舍

当前方案的优点：

- 改动小，不影响 `Opcode` enum 和 `Instructions` 编码。
- 不影响 VM 指令读取逻辑。
- 不破坏现有 bytecode snapshot/断言。
- 不需要引入 finalize pass。

当前方案的缺点：

- 不像 QuickJS 那样把源码位置作为中间 bytecode 的一部分，后续如果实现复杂优化 pass，需要记得同步更新 debug side table。
- 还没有压缩编码。
- 还没有 line/column，只能返回 byte span。

---

## 7. 测试策略

当前新增测试位于 `compiler/compiler_test.rs`：

### 7.1 主程序 PC 到 Span

输入：

```monkey
1;
22
```

断言：

- 第一条表达式的 PC 映射到 `Span { start: 0, end: 1 }`。
- 第二条表达式的 PC 映射到 `Span { start: 3, end: 5 }`。
- `span_for_pc` 可以在指令内部 PC 上查到最近位置。

### 7.2 函数常量 PC 到 Span

输入：

```monkey
let add = fn(a, b) { a + b; };
```

断言：

- constant pool index `0` 的函数 debug info 存在。
- 函数内部 `OpAdd` 附近的 PC 能映射到 `a + b` 的 span。

### 7.3 回归验证

需要持续保证：

```bash
cargo test -p monkey-compiler
cargo test
```

---

## 8. 后续演进

### 8.1 转换为 line/column

当前 `Span` 是 byte offset。可以新增一个 source map helper：

```rust
pub struct SourceMap {
    line_starts: Vec<usize>,
}
```

通过 `line_starts` 把 `Span.start` 转换成 line/column，用于错误提示和编辑器 range。

### 8.2 压缩 debug info

当前 `DebugInfo` 直接保存 `Vec<PcSpan>`。后续可以改为 delta 编码：

```text
pc_delta, start_delta, end_delta
```

或者只保存 `pc_delta, start_delta, len`。这会更接近 QuickJS 的压缩表。

### 8.3 引入 QuickJS 风格临时 opcode

如果未来 compiler 引入多阶段优化/label resolve，可以考虑改为：

1. 在 `Opcode` 中新增 `OpSourceLoc`。
2. 编译 AST 时 emit `OpSourceLoc(start, end)`。
3. 新增 `finalize_bytecode()` pass：
   - 扫描 instructions。
   - 遇到 `OpSourceLoc` 时更新当前 source span。
   - 遇到普通 opcode 时写入新 instructions，并记录旧 PC 到新 PC 的映射。
   - 删除所有 `OpSourceLoc`。
   - 根据旧 PC 到新 PC 映射 patch jump operand。
   - 生成 `DebugInfo`。
4. VM 只接收 finalize 后的 instructions。

这个方案更接近 QuickJS，但需要先把当前 compiler 的直接 jump patch 逻辑演进成 label 或 relocation 模型，否则删除临时 opcode 后 patch jump 会比较脆弱。

### 8.4 写入 `CompiledFunction`

当前函数 debug info 存在 `Bytecode.function_debug_info` 中。后续如果 VM runtime 需要在调用栈里直接访问函数 debug info，可以考虑把 debug info 放入 `object::CompiledFunction`：

```rust
pub struct CompiledFunction {
    pub instructions: Vec<u8>,
    pub num_locals: usize,
    pub num_parameters: usize,
    pub debug_info: DebugInfo,
}
```

这样 VM frame 可以直接通过当前 closure 找到 debug info。代价是 `object` crate 会依赖 compiler debug 类型，或者需要把 debug 类型下沉到共享 crate。

更保守的做法是新建共享 debug 类型模块，避免 `object` 反向依赖 `compiler`。

---

## 9. WASM 与 Playground 集成

### 9.1 WASM API

`wasm/src/lib.rs` 新增 `compile_with_debug(input: &str) -> String`，返回 JSON 序列化的 `BytecodeDebugView`：

```json
{
  "detail": "Instructions:\n0000 OpConst 0\n...",
  "mainDebugInfo": {
    "pcSpans": [{ "pc": 0, "span": { "start": 0, "end": 1 } }]
  },
  "functionDebugInfo": {
    "0": { "pcSpans": [{ "pc": 4, "span": { "start": 21, "end": 26 } }] }
  },
  "instructionLines": [
    { "line": 1, "pc": 0, "scope": { "type": "main" } },
    { "line": 8, "pc": 4, "scope": { "type": "function", "constantIndex": 0 } }
  ]
}
```

字段说明：

- `detail`：与 `compile_detail` 相同的可读 bytecode 文本，供 Playground 展示。
- `mainDebugInfo` / `functionDebugInfo`：主程序与函数常量的 PC → span 表。
- `instructionLines`：`detail` 文本中每条指令行对应的 0-based 行号、PC 和 scope。Playground 用它在用户点击 bytecode 行时定位 PC，避免解析缩进文本。

原有 `compile` / `compile_detail` 保持不变，不携带 debug info。

### 9.2 `BytecodeDebugView`

`compiler/compiler.rs` 提供 `Bytecode::debug_view()`，统一生成 `detail` 文本和 `instructionLines`。`Bytecode::string()` 委托给 `debug_view().detail`，保证展示文本与 line map 一致。

### 9.3 Playground 联动

Playground（`packages/playground/src/App.tsx`）在 bytecode 面板使用 `compile_with_debug`：

1. 解析 JSON 得到 `BytecodeDebugView`。
2. 用 `detail` 填充 bytecode 只读编辑器。
3. 监听 bytecode 编辑器选区变化，根据光标 offset 计算行号。
4. 在 `instructionLines` 中查找对应行，取得 PC 和 scope。
5. 从 `mainDebugInfo` 或 `functionDebugInfo` 调用 `spanForPc`，得到源码 byte span。
6. 在左侧源码编辑器高亮对应范围（复用 AST 联动的 `highlightRange`）。

辅助函数位于 `packages/playground/src/bytecodeDebug.ts`。

交互方向：**bytecode 行 → 源码高亮**。源码 → bytecode 反向高亮尚未实现。

### 9.4 本地开发

修改 compiler 或 wasm 后需要重新构建 wasm 包，Playground 才能加载新 API：

```bash
cd wasm && wasm-pack build --release --scope=gengjiawen
pnpm -C packages/playground dev
```

### 9.5 测试

compiler 侧新增 `bytecode_debug_view_*` 测试，验证：

- `debug_view().detail` 与 `string()` 输出一致。
- 主程序与函数 instruction line 的 line / pc / scope 映射正确。

Playground 联动目前依赖手动验证；后续可考虑 Playwright 或 wasm 层 JSON 快照测试。

