# monkey-gc 设计文档

> 本文说明 `monkey-gc` crate 的设计：QuickJS 风格的引用计数 + 三阶段环检测 GC，以及基于同一套 bytecode 的 `GcVM` 运行时。

## 目录

1. [背景](#1-背景)
2. [目标与非目标](#2-目标与非目标)
3. [整体架构](#3-整体架构)
4. [模块设计](#4-模块设计)
5. [GC 算法](#5-gc-算法)
6. [值模型与桥接层](#6-值模型与桥接层)
7. [GcVM 设计](#7-gcvm-设计)
8. [与现有系统的关系](#8-与现有系统的关系)
9. [已知限制](#9-已知限制)
10. [测试策略](#10-测试策略)
11. [后续演进](#11-后续演进)
12. [文件索引](#12-文件索引)

---

## 1. 背景

Monkey 语言在 `monkey-rust` 中已有完整的编译执行管线：

```
lexer → parser → compiler (bytecode) → VM
```

其中 `compiler::VM` 与 `object::Object` 使用 `Rc<Object>` 管理堆对象。引用计数无法回收**纯环状引用**（例如两个闭包互相持有），且 `Rc` 循环会导致内存泄漏。

`monkey-gc` crate 提供一套 **QuickJS 风格的 GC 运行时**，作为现有 bytecode VM 的**并行替代实现**：

| 维度 | `compiler::VM` | `monkey-gc::GcVM` |
|------|----------------|-------------------|
| 堆管理 | `Rc<Object>` | `GcHeap` + `GcRef` |
| 环回收 | 不支持 | 三阶段 cycle collector |
| `object` crate | 直接使用 | 不修改；通过 import/export 桥接 |
| 字节码 | `Bytecode` | 复用同一套 `Bytecode` |

算法参考 QuickJS 源码（`JS_RunGC`、`gc_decref`、`gc_scan`、`gc_free_cycles`）。

---

## 2. 目标与非目标

### 2.1 目标

- 在独立 crate 中实现可环回收的堆，不修改 `object` / `interpreter`。
- 复用 `compiler` 产出的 `Bytecode` 与 opcode 定义。
- 提供与 `compiler::VM` 等价的 Monkey 语义（算术、闭包、builtins 等）。
- 对外暴露 `eval_source` 等便捷 API，返回 `object::Object` 以便与现有工具链对接。
- 保持与 QuickJS GC 算法结构一致，便于对照阅读和后续优化。

### 2.2 非目标

- 当前不替换 `wasm` / `playground` 中的默认 VM。
- 当前不把 `object::Object` 本身改为 GC 管理。
- 当前不实现多线程 / `Send` / `Sync` 保证。
- 当前不实现写屏障；引用维护依赖显式 `dup` / `free`。
- 当前字符串内联在 `ValueCell` 中，未拆到纯 refcount 堆。

---

## 3. 整体架构

```
                    Monkey 源码
                         │
                         ▼
              lexer ──► parser ──► compiler
                         │              │
                         │         Bytecode
                         │         ╱      ╲
                         │        ╱        ╲
                         ▼       ▼          ▼
                   interpreter  compiler::VM  gc::GcVM
                   (AST 求值)   Rc<Object>   GcRef
                         │           │          │
                         └───────────┴──────────┘
                                     │
                              object::Object
```

### 依赖关系

```
monkey-gc
  ├── monkey-compiler  (Bytecode, Opcode, Compiler)
  ├── monkey-object    (Object, BuiltinFunc, BuiltIns)
  ├── monkey-parser    (eval_source 解析)
  └── byteorder        (指令解码)
```

Workspace 成员：根 `Cargo.toml` → `"gc"`。

---

## 4. 模块设计

| 模块 | 文件 | 职责 |
|------|------|------|
| 入口 | `lib.rs` | 导出公共 API；`compile` / `eval` / `eval_source` |
| 堆 API | `heap.rs` | `GcHeap`、`GcRef`；分配 / 释放 / GC 触发 |
| 运行时核心 | `runtime.rs` | `GcRuntime`：refcount、三阶段 GC、对象槽管理 |
| 对象头 | `header.rs` | `GcObjectHeader`、`GcPhase`、`GcObjectType` |
| 侵入式链表 | `list.rs` | `GcList`：`gc_obj` / `tmp` / `zero_ref` 三条链表 |
| 分配统计 | `malloc.rs` | `MallocState`、GC 阈值触发 |
| 值模型 | `value.rs` | `Value`、`ValueCell`、import/export 桥接 |
| 调用帧 | `frame.rs` | `Frame`（闭包 + IP + 栈基址） |
| 虚拟机 | `vm.rs` | `GcVM`：opcode 解释执行 |

### 4.1 核心类型

```rust
// 不透明句柄，本质是堆内索引
pub struct GcRef(pub GcId);  // GcId = usize

// 高层堆 API
pub struct GcHeap { rt: GcRuntime }

// 可参与环检测的 GC 对象
pub trait GcObject: Any {
    fn trace(&self, visit: &mut dyn FnMut(GcId));  // 报告出边
    fn on_free(&mut self, rt: &mut GcRuntime) {}     // 释放回调
}
```

所有 Monkey 运行时值包装在 `ValueCell` 中，实现 `GcObject::trace` 以遍历 `Array` / `Hash` / `Closure` 的子引用。

---

## 5. GC 算法

### 5.1 两类对象

| 类型 | Header | 环检测 | 用途 |
|------|--------|--------|------|
| GC 对象 | `GcObjectHeader` | 是 | `ValueCell`、未来可扩展的 function bytecode 等 |
| 纯 refcount 对象 | `RefCountHeader` | 否 | 字符串等（API 已预留，`add_ref_counted`） |

### 5.2 对象头

```rust
pub struct GcObjectHeader {
    pub ref_count: i32,        // 引用计数
    pub gc_obj_type: GcObjectType,
    pub mark: u8,               // GC 阶段临时标记（非永久 mark bit）
    pub free_mark: bool,        // 僵尸检测（cycle free 期间）
    pub list_prev: Option<GcId>,
    pub list_next: Option<GcId>,
}
```

### 5.3 三条侵入式链表

```
gc_obj_list           — 所有存活 GC 对象（ref_count > 0）
tmp_obj_list          — trial deletion 中 ref_count 归零的候选
gc_zero_ref_count_list — 延迟释放队列（cycle 移除时 ref 被恢复的对象）
```

链表通过 `GcId` 索引 + header 内嵌 `list_prev/next` 实现，等价于 QuickJS `list.h`。

### 5.4 三阶段 Cycle Collection

```
Phase 1: gc_decref     trial deletion，沿 trace 边试探性减引用
        │
        ▼
Phase 2: gc_scan       对仍存活对象恢复被误减的引用
        │
        ▼
Phase 3: gc_free_cycles  释放 tmp 列表中真正不可达的环
```

**Phase 1 — `gc_decref`**

- 遍历 `gc_obj_list` 中每个对象。
- 对每个对象 `mark_children(Decref)`，沿 `trace` 边将子节点 `ref_count -= 1`。
- 若对象自身 `ref_count == 0`，移入 `tmp_obj_list`。

**Phase 2 — `gc_scan`**

- 对 `gc_obj_list` 中仍存活（`ref_count > 0`）的对象：`mark_children(ScanIncref)`，恢复被误减的引用。
- `gc_obj_list` 按链表动态遍历；`ScanIncref` 从 `tmp_obj_list` 迁回来的对象会追加到 `gc_obj_list` 尾部，并在同一轮 scan 中继续扫描。这一点与 QuickJS `list_for_each(el, &rt->gc_obj_list)` 保持一致，确保外部根间接保活的整段环都被恢复。
- 对 `tmp_obj_list` 中的对象：`mark_children(ScanIncref2)`，仅增引用不计入链表迁移。

**Phase 3 — `gc_free_cycles`**

- 设置 `gc_phase = RemoveCycles`。
- 逐个释放 `tmp_obj_list` 中的对象（真正不可达的环）。
- `free_heap_object` 先释放 `trace` 出来的子边，再运行 `on_free`，最后移出 GC 链表。
- 若释放子边和 finalizer 后 `ref_count != 0`，只把物理释放 **延迟** 到 `gc_zero_ref_count_list`；`on_free` 不会在延迟释放阶段重复执行。

### 5.5 即时释放路径

`ref_count` 减到 0 且不在 `RemoveCycles` 阶段时：

```
free_gc → gc_zero_ref_count_list → free_zero_refcount → free_heap_object
```

`free_heap_object` 会 `trace` 子节点并递归 `free_gc`，形成级联释放。对已释放的子节点，`free_gc` 通过 `object_exists` 检查避免沿陈旧边 panic。

### 5.6 GC 触发策略

```rust
pub const DEFAULT_GC_THRESHOLD: usize = 256 * 1024;  // 256 KB
```

- 每次 `GcHeap::alloc` 前调用 `trigger_gc(alloc_size)`。
- 当 `malloc_size + alloc_size > threshold` 时执行 `run_gc()`。
- 触发后阈值上调为 `malloc_size + malloc_size/2`（与 QuickJS 一致）。
- `set_gc_threshold(usize::MAX)` 可禁用自动 GC。

### 5.7 重入保护

```rust
pub enum GcPhase {
    None,
    Decref,        // free_zero_refcount 期间
    RemoveCycles,  // gc_free_cycles 期间
}
```

在 `RemoveCycles` 阶段，`ref_count != 0` 的对象不立即物理释放，而是 defer 到 phase 结束，避免与环拆除逻辑冲突。

---

## 6. 值模型与桥接层

### 6.1 `Value` vs `Object`

`Value` 镜像 `object::Object`，但容器类型的边使用 `GcRef` 而非 `Rc<Object>`：

```rust
pub enum Value {
    Integer(i64),
    Boolean(bool),
    String(String),
    Array(Vec<GcRef>),
    Hash(HashMap<HashKey, GcRef>),
    Null,
    Error(String),
    CompiledFunction(CompiledFunction),
    Closure(GcClosure),       // { func: GcRef, free: Vec<GcRef> }
    Builtin(BuiltinFunc),
}
```

### 6.2 所有权语义

| 操作 | 行为 |
|------|------|
| `alloc_value` | `with_owned_edges` 对所有子 `GcRef` 执行 `dup`，再分配；调用方仍负责释放传入的临时边 |
| `import_object` | 递归 import 子对象，父对象分配完成后释放这些临时子引用，只保留父对象持有的边 |
| VM `push` | 对已有引用执行 `dup` 后入栈；`push_raw` 表示转移一份已拥有的引用 |
| VM `pop` | 将栈槽持有的引用转移给调用方，并把槽位重置为 `null` 引用 |
| VM 覆盖栈槽/全局变量 | 先 `free` 旧值，再写入新 `GcRef` |
| VM 构造数组/哈希/闭包 | 用栈上的临时引用分配父对象，随后清空对应栈区间，避免临时引用泄漏 |
| 运算消费操作数 | `free(left)` + `free(right)` |

### 6.3 import / export 桥接

```
object::Object (Rc)  ──import_object──►  GcRef (Value)
GcRef (Value)          ──export_object──►  object::Object (Rc)
```

- **import**：递归深拷贝到 GC 堆，建立 `GcRef` 边。
- **export**：递归导出为 `Rc<Object>`，供 builtins 和外部 API 使用。
- **builtins**：`call_builtin` 先 export 参数 → 调用 `BuiltinFunc` → import 结果。

`Object::Function`（解释器 AST 函数）**不可导入**，会 panic。

---

## 7. GcVM 设计

### 7.1 结构

```rust
pub struct GcVM {
    heap: GcHeap,
    constants: Vec<GcRef>,      // 由 bytecode.constants import 而来
    stack: Vec<GcRef>,          // 容量 2048
    sp: usize,
    globals: Vec<GcRef>,         // 容量 65536
    frames: Vec<Frame>,         // 最多 1024 帧
    frame_index: usize,
    null: GcRef,                 // 共享 null 单例
    last_popped: GcRef,          // 最近一次 OpPop 的结果
}
```

与 `compiler::VM` 布局一致，仅将 `Rc<Object>` 替换为 `GcRef`。

### 7.2 支持的 Opcode

完整覆盖当前 Monkey bytecode 指令集：

| 类别 | Opcodes |
|------|---------|
| 常量/字面量 | `OpConst`, `OpTrue`, `OpFalse`, `OpNull` |
| 算术 | `OpAdd`, `OpSub`, `OpMul`, `OpDiv`, `OpMinus` |
| 比较/逻辑 | `OpEqual`, `OpNotEqual`, `OpGreaterThan`, `OpBang` |
| 控制流 | `OpJump`, `OpJumpNotTruthy` |
| 栈 | `OpPop` |
| 全局/局部 | `OpGetGlobal`, `OpSetGlobal`, `OpGetLocal`, `OpSetLocal` |
| 复合类型 | `OpArray`, `OpHash`, `OpIndex` |
| 函数 | `OpCall`, `OpReturn`, `OpReturnValue` |
| 闭包 | `OpClosure`, `OpGetFree`, `OpCurrentClosure` |
| 内置 | `OpGetBuiltin` |

### 7.3 借用检查策略

Rust 借用规则要求 VM 在「读堆」与「写堆」之间拆分步骤：

- `callee_kind()` — 先 clone `GcClosure` / 读取 builtin，再 mutate。
- `alloc_and_push` / `dup_and_push` / `push_raw` — 分离分配与栈写入。
- 运算函数 — 先 `get_value` 读操作数，再 `alloc_and_push` 写结果，最后 `free` 操作数。

### 7.4 公共 API

```rust
// 便捷入口
pub fn eval_source(source: &str) -> Result<Object, String>;
pub fn eval(program: &Node) -> Result<Object, String>;
pub fn compile(program: &Node) -> Result<Bytecode, String>;

// 手动控制
let bytecode = Compiler::new().compile(&program)?;
let mut vm = GcVM::new(bytecode);
vm.run();
let result = vm.export_last_result();  // Option<Object>
```

---

## 8. 与现有系统的关系

```
┌─────────────────────────────────────────────────────┐
│                   monkey-rust workspace              │
├──────────────┬──────────────┬───────────────────────┤
│ interpreter  │ compiler     │ gc (本 crate)          │
│ AST 求值     │ 编译 + Rc VM │ 编译 + GC VM           │
│ Rc<Object>   │ Rc<Object>   │ GcRef                  │
├──────────────┴──────────────┴───────────────────────┤
│ object (共享类型定义，未修改)                          │
│ parser / lexer (共享前端)                             │
│ wasm / playground (当前仍用 compiler::VM)             │
└─────────────────────────────────────────────────────┘
```

---

## 9. 已知限制

| 限制 | 说明 |
|------|------|
| QuickJS 对象模型未完整移植 | 当前只实现 Monkey `ValueCell` 所需的 `MonkeyObject`/`FunctionBytecode` 风格路径，没有 shape、realm、var ref、async function 等完整对象系统 |
| finalizer 能力较小 | `on_free` 对应 QuickJS finalizer；调用前 trace 边已经释放，但 Rust 对象字段不会像 QuickJS C 结构那样逐字段置空，因此 `on_free` 不应再次释放 trace 边 |
| 陈旧边防护 | `free_gc` 对已释放子节点做 `object_exists` 检查，避免沿已拆除的环边 panic |
| 字符串未拆堆 | `Value::String` 内联在 `ValueCell` 中，未使用 `RefCountHeader` 路径 |
| `GcObjectType` 预留 | `FunctionBytecode`, `Shape`, `VarRef`, `AsyncFunction`, `MonkeyContext` 等 tag 已定义但未使用 |
| 无写屏障 | 与 QuickJS 一致，依赖显式 `dup`/`free` 维护 refcount |
| 单线程 | 无 `Send`/`Sync` 保证，设计为单线程 VM |

---

## 10. 测试策略

当前 **42 个测试**，分三层：

| 层 | 文件 | 覆盖 |
|----|------|------|
| GC 算法 | `gc_test.rs` (17) | refcount、dup、2/3/4 节点环、自环、外部根间接保活整个环、无环图、on_free 顺序、GC 阈值、mark 函数、幂等性 |
| 值桥接 | `value_test.rs` (9) | import/export 往返、HashKey、子 refcount、import 临时引用释放、Value 层环回收 |
| 端到端 VM | `vm_test.rs` (16) | 算术、布尔、条件、let、字符串、数组、哈希、索引、函数、闭包、builtins、调用后临时引用清理 |

运行：

```bash
cargo test -p monkey-gc
```

---

## 11. 后续演进

1. **wasm 集成** — 在 wasm crate 增加 `GcVM` 导出，供 playground 切换。
2. **字符串拆堆** — 大字符串走 `RefCountHeader`，减 `ValueCell` 体积。
3. **递归 / 复杂环场景** — 继续补充真实程序级回归，验证闭包与容器组合后的生产行为。
4. **GC 统计 API** — 暴露 `gc_object_count`、`malloc_state` 供 playground 调试面板。
5. **性能对比** — 与 `Rc` VM 建立 benchmark（分配速率、GC 暂停）。

---

## 12. 文件索引

```
gc/
├── lib.rs          # 入口 + eval API
├── heap.rs         # GcHeap / GcRef
├── runtime.rs      # GcRuntime + GC 三阶段
├── header.rs       # 对象头 + 类型 tag
├── list.rs         # 侵入式链表
├── malloc.rs       # 分配统计 + 阈值
├── value.rs        # Value + import/export
├── frame.rs        # 调用帧
├── vm.rs           # GcVM
├── gc_test.rs      # GC 单元测试
├── value_test.rs   # 值层测试
└── vm_test.rs      # VM 集成测试
```
