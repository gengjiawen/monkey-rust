# Monkey ARM64 汇编后端设计提案（AOT）

> 状态：已实施（`asm/` crate，见 [asm/README.md](../asm/README.md)）。
> 已吸收设计评审：修正调用栈布局、拆分普通调用与构造、
> 冻结 checked `i64` 语义、闭合 native/模拟器两套运行时适配、修正帧寻址与 span 传播、
> 并把差分测试改为独立通道上的规范化结构化协议。
> 尚未实施的部分：wasm 模拟器与 playground 集成（§12）、GC 接入（§14）。
>
> 核心结论：仿照《Compiling to Assembly from Scratch》（Keleshev）的路线，从 AST 单遍 lower 并延迟拼装
> **arm64（AArch64）汇编文本**，交给交叉工具链汇编链接成可执行文件——AOT，不做 JIT。
> 新增**单一 workspace 成员 `asm/`**，同一 crate 构建两次：host 构建产出编译器 CLI，
> aarch64 交叉构建产出运行时静态库，`.s` 文件是两者之间的接口。
>
> 关联设计：[bytecode-snapshot-design.md](./bytecode-snapshot-design.md)（compile/run 的 CLI 形态先例）、
> [js-style-class-design.md](./js-style-class-design.md)（对象模型与调用语义）、
> [compiler-debug-info-design.md](./compiler-debug-info-design.md)（span 映射，playground 联动复用同一思路）。

## 目录

1. [背景与路线选择](#1-背景与路线选择)
2. [目标与非目标](#2-目标与非目标)
3. [与原书的对应关系](#3-与原书的对应关系)
4. [总体架构：单 crate 构建两次](#4-总体架构单-crate-构建两次)
5. [值表示：两阶段](#5-值表示两阶段)
6. [代码生成约定](#6-代码生成约定)
7. [函数与闭包的调用约定](#7-函数与闭包的调用约定)
8. [运行时 ABI](#8-运行时-abi)
9. [平台与工具链](#9-平台与工具链)
10. [测试策略与观测协议](#10-测试策略与观测协议)
11. [分章实施路线](#11-分章实施路线)
12. [Playground 演示](#12-playground-演示)
13. [已知前置问题](#13-已知前置问题)
14. [后续演进](#14-后续演进)

---

## 1. 背景与路线选择

仓库现有三条执行路径：`interpreter`（树遍历）、`compiler`+`vm`（字节码栈机）、
`gc`（带循环回收的字节码 VM），外加 `.mbc` 快照的 AOT-to-bytecode 工作流。
本提案补上第四条：AOT-to-native，目标只有 arm64。

候选路线对比：

| 路线                             | 新代码量级 | 主要成本                                             | 结论                                                          |
| -------------------------------- | ---------- | ---------------------------------------------------- | ------------------------------------------------------------- |
| 进程内 JIT（手写 encoder）       | ~3–4k 行   | arm64 指令编码、mmap/W^X、刷 icache、macOS `MAP_JIT` | 教学价值高但机制成本重，留作远期                              |
| JIT via `dynasm-rs` / Cranelift  | ~1.5–3k 行 | 依赖重，Cranelift 基本学不到 arm64 本身              | 不采用                                                        |
| **AOT 输出 `.s` 文本（本提案）** | ~2k 行     | 运行时 ABI 层                                        | **采用**：encoder/W^X/icache 全部消失，emitter 就是字符串拼接 |

这正是《Compiling to Assembly from Scratch》的做法（该书目标是 32 位 ARM，本文全部换算到 AArch64）。
它的三条灵魂原则全部保留：

- **单遍 lower**：每个 AST 节点只访问一次，无 IR、无优化；函数体先写独立 buffer，完成后补帧并拼装；
- **零寄存器分配**：表达式结果一律放 `x0`（累加器），临时值压机器栈；
- **运行时当 libc 用**：语言里复杂的部分（字符串/数组/哈希/闭包/class）全部下沉为
  `extern "C"` 运行时函数，汇编只负责搬运 tagged value 和跳转。

**从 AST 直出而不是从字节码翻译**：两者工作量接近，选 AST 直出是为了教学映射直观
（"一个 `if` 变成 `cmp`+`b.eq`"）。关键前提是作用域分析可以复用——
`compiler/symbol_table.rs` 不依赖字节码，Global/Local/Free/Builtin/Function 五种 scope、
slot 索引、跨层自由变量捕获链（`resolve` 中的逐层 `define_free`）都能被新后端原样使用
（具名函数的自引用需要显式步骤，见 §7 与 §13）。

## 2. 目标与非目标

### 2.1 目标

- `monkey-asm build foo.monkey -o foo`：生成 `foo.s`，调用交叉工具链汇编链接出
  aarch64-linux 可执行文件；`monkey-asm run` 在非 arm64 机器上透过 `qemu-aarch64` 执行。
- 语言特性与里程碑 0 冻结的语义矩阵对齐：整数（完整 `i64` **值域**，经 SMI + boxed integer
  双表示；算术采用 checked-fatal 规则）、
  布尔/字符串/数组/哈希/null、闭包、递归、class/`new`/`this`/属性读写、全部 builtin
  （六个 id、七个名字，`print` 是 `puts` 的别名，见 `object/builtins.rs`）。
- 与 interpreter / 普通 VM / gc VM 的**差分测试**；现有引擎先经语义一致性前置 PR 对齐，观测协议见 §10。
- `.s` 携带 span 信息，playground 做 godbolt 式"源码 ↔ 汇编"联动视图（§12）。

### 2.2 非目标

- 不做 JIT，不在进程内生成机器码。
- 不做寄存器分配与任何优化（"已知顶层函数直接 `bl`"之类留到 §14）。
- 只支持 Linux GNU as 方言；macOS（`_` 前缀、`@PAGE/@PAGEOFF`）留作后续小开关。
- 不支持 x86_64 目标。
- v1 内存策略为分配后不回收（对齐原书 malloc-从不-free）；对接 `gc` crate 是可选的最后一章。
- 参数声明上限：**普通函数 7 个，方法与 constructor 6 个**（`x0` 传闭包、`x1` 起传参，
  方法的隐式 `this` 占用 `x1`，与现有编译器 `num_parameters = params + 1` 的口径一致，
  见 `compiler.rs::compile_method`）。声明超限在编译期报错；调用点的紧凑 `argv` 可更长，以支持
  variadic `puts`，但任何 `CallDispatch::Invoke` 都不得超过 `x1..x7` 的七个实际参数。
- 语言错误都是**终止型运行时错误**：运行时记录稳定的错误类别、stderr 打印人类可读消息后
  `exit(1)`；invalid builtin 不再作为可绑定的 `Object::Error` 值继续执行。实现内部可以使用
  `Result`，但不得让 Rust panic 穿过 FFI 边界。
- CLI 定位为**仓库内开发工具**：`build` 缺静态库时会递归调用 cargo，依赖仓库源码在场；
  独立分发（把 runtime 静态库随 CLI 打包）不在 v1 范围。

## 3. 与原书的对应关系

| 原书                                      | 本提案                                            | 说明                                      |
| ----------------------------------------- | ------------------------------------------------- | ----------------------------------------- |
| TypeScript 实现，每个 AST 类一个 `emit()` | Rust `lower.rs` 对 `parser::ast` 做 match 遍历    | 结构同构                                  |
| 目标 ARMv7（32 位），GNU 工具链 + QEMU    | AArch64，`aarch64-linux-gnu-gcc` + `qemu-aarch64` | 附录同款流程                              |
| `push {r0, ip}` 凑 8 字节对齐             | `str x0, [sp, #-16]!`（一值一槽）                 | arm64 硬性要求 sp 16 字节对齐，反而更无脑 |
| 环境 = 名字 → fp 偏移的 Map               | 复用 `SymbolTable` 的 slot index → fp 负偏移      | 自引用见 §7                               |
| Part I：整数子集到函数递归                | 第 1–3 章（裸 i64，跑通 fib）                     | 一一对应                                  |
| Dynamic Typing 章（低位 tag）             | 第 4 章 tagged value                              | 编码见 §5                                 |
| Arrays and Heap 章（调 malloc）           | 第 5 章（调 `rt_*`，堆在运行时 crate 里）         | 运行时更厚，因为 Monkey 天生动态          |
| 无闭包、无 class                          | 第 6、7 章                                        | 超出原书的部分，遵守 §10 的冻结语义矩阵   |
| GC 章                                     | 第 8 章（可选，对接 `gc` crate）                  |                                           |

## 4. 总体架构：单 crate 构建两次

只新增一个 workspace 成员，沿用仓库扁平文件惯例（无 `src/`）：

```
asm/
  Cargo.toml        # [lib] crate-type = ["rlib", "staticlib"] + [[bin]] monkey-asm
  lib.rs
  emitter.rs        # main/函数/数据分区 buffer、label 计数器、span 栈、帧/立即数助手
  lower.rs          # AST 单遍 lower + 延迟拼装，复用 compiler::symbol_table::SymbolTable
  runtime_core.rs   # 与存储/执行方式无关的语义函数和 CallDispatch（§8、§12）
  runtime_backend.rs # ValueStore trait、PointerStore/HandleStore 与 opaque CodeHandle
  runtime.rs        # native 调用适配 + #[no_mangle] extern "C" FFI 壳：
                    #   #[cfg(not(target_family = "wasm"))]
  main.rs           # CLI：build / run 子命令
  *_test.rs         # 惯例同其他 crate，insta 快照放 snapshots/
```

同一 crate 两种用法：

```bash
# 1) host 构建 → 编译器 CLI
cargo build -p monkey-asm

# 2) aarch64 交叉构建 → 运行时静态库
#    std 的 String/Vec/HashMap 直接可用，也可依赖 object crate 复用 VM 逻辑
cargo build -p monkey-asm --lib --target aarch64-unknown-linux-gnu --release
#    → target/aarch64-unknown-linux-gnu/release/libmonkey_asm.a
```

`build` 子命令串起链路：生成 `out.s` → 确认 `libmonkey_asm.a` 存在（缺失则在仓库内递归调用
cargo 构建，见 §2.2 的 CLI 定位）→

```bash
aarch64-linux-gnu-gcc out.s libmonkey_asm.a -o prog -lpthread -ldl -lm -lrt -lutil
qemu-aarch64 -L /usr/aarch64-linux-gnu ./prog
```

上面列的是 linux-gnu 基线；CLI 构建 runtime 时必须读取 rustc 为该 target 报出的
`native-static-libs` 并据此组装最终链接参数，不能假定不同 Rust/toolchain 版本的依赖集合恒定。

第 1–3 章（裸 i64 阶段）不需要静态库，`aarch64-linux-gnu-gcc out.s -o prog` 即可
（x86_64 主机上普通 `cc` 无法汇编 arm64 文本，任何阶段都必须走交叉工具链）——
里程碑 0 的手写 ABI probe 会单独链接 `libmonkey_asm.a`，但 lower 生成的程序到第 4 章才首次依赖它。

`wasm` crate 只依赖本 crate 的 rlib 纯函数部分（`emitter`/`lower`，以及 §12 V2 需要的
`runtime_core`/`HandleStore`）；它只提供模拟内存、指令执行与调用控制流适配。
`runtime.rs` 的 native 指针解码、函数指针调用和 FFI 符号经 `cfg` 门控，不进入 wasm 产物。

## 5. 值表示：两阶段

### 5.1 阶段一（第 1–3 章）：裸 i64

寄存器里就是机器整数，布尔用 0/1。没有堆、没有运行时库。打印整数经 libc `printf`：
`.rodata` 放 `"%ld\n"`，`x0` 传格式串、`x1` 传值——**仅限 Linux**（AAPCS64 在 Linux 上
变参走寄存器；Darwin 变参走栈，这是 §9 只支持 Linux 的原因之一）。阶段二起 `printf`
全面废除；第 6 章开放 builtin 后，Monkey 的 `puts`/`print` 经 `rt_call` 进入 runtime 的非变参输出 sink。

目的与原书 Part I 相同：用最短路径打通"AST → `.s` → 交叉汇编 → qemu 正确输出"的全链条。
第 1–3 章是会被后续章节替换的教学子集，只使用不触发 `i64` 溢出的语料，不参加完整语义差分；
checked 算术、tagged value 与规范化错误从第 4 章起同时启用。这样早期里程碑不暗含一套与最终版
不同且未定义的溢出语义。

### 5.2 阶段二（第 4 章起）：64 位 tagged value

| 类别                 | 编码                                                                           | 判别                       |
| -------------------- | ------------------------------------------------------------------------------ | -------------------------- |
| SMI（小整数，63 位） | `value << 1`，bit0 = 0                                                         | `tbz x, #0`                |
| 堆引用               | native 为 `ptr \| 0b001`（对象 8 字节对齐，还原为 `v - 1`）；simulator 见 §8.1 | bit0 = 1 且低 3 位 = `001` |
| builtin              | `(BuiltinId << 3) \| 0b101`                                                    | 低 3 位 = `101`            |
| `false`              | 常量 `0b0011`（3）                                                             | 整值比较                   |
| `true`               | 常量 `0b0111`（7）                                                             | 整值比较                   |
| `null`               | 常量 `0b1011`（11）                                                            | 整值比较                   |

- **整数覆盖完整 `i64` 值域，但不是 bigint**：SMI 只覆盖 `[-2^62, 2^62-1]`；该范围外但仍在
  `i64` 内的字面量，以及 SMI 运算后仍落在 `i64` 内的结果，由运行时装箱为堆对象
  **boxed integer**（`rt_box_int`）。所有算术/比较函数都接受 SMI 与 boxed 的任意组合，返回时
  能收缩回 SMI 则收缩。这对齐 AST 的 `Integer.raw: i64`，但不声称能表示超出 `i64` 的结果。
- **算术规则冻结为 checked-fatal**：`+`/`-`/`*`/一元 `-` 分别使用 `checked_add`、
  `checked_sub`、`checked_mul`、`checked_neg`；`/` 先区分除零，再使用 `checked_div`。
  有效除法按 Rust `i64`/AArch64 `sdiv` 一致的规则向零截断。
  `i64::MAX + 1`、`-i64::MIN`、`i64::MIN / -1` 都产生 `IntegerOverflow`，除零产生
  `DivisionByZero`，二者最终走 `rt_fatal`。不得依赖 Rust debug/release 的不同溢出行为。
- 单例（true/false/null）判别一律用**整值比较**，不只看低 3 位（3 与 11 的低 3 位相同）。
- builtin 是立即数编码，不占堆；名字解析发生在编译期（`SymbolScope::Builtin` 的 index → id），
  `print`/`puts` 两个名字共享 `BuiltinId::Puts`。
- 字符串/数组/哈希/闭包/class/实例/bound method/boxed integer 都是堆对象，
  布局对汇编**完全不透明**（自由变量也经 `rt_get_free` 访问，见 §7）；
  这让运行时可以直接用 Rust 的 `String`/`Vec`/`HashMap` 实现。
- 整数加法内联 SMI fast path（`orr` 检查两 bit0 后 `adds`；AArch64 的 `V` 标志这里只表示
  **SMI 编码范围**溢出，溢出或非 SMI 都转 `bl rt_add`，由后者做 raw `i64` checked 运算并决定
  装箱或 fatal）。SMI 相加免 untag：`(a<<1)+(b<<1) = (a+b)<<1`。其余运算 v1 直接调运行时。
- 全局值是 `.bss` 中单一连续数组 `g_globals`，符号只用 `Symbol.index` 寻址，源码名字仅出现在注释；
  因此同名重新绑定不会生成重复 label。`main` 开头调 `rt_globals_init` 把整个区间写成 `null`
  并注册给运行时，避免"bss 零值 = SMI 0"的语义混淆。

## 6. 代码生成约定

累加器 + 机器栈，核心模式如下（`;` 后为说明，实际产物用 `//` 注释携带源码片段）：

```asm
// 二元运算：emit 左 → 压栈 → emit 右 → 弹到 x1 → 运算
str  x0, [sp, #-16]!        ; push 左操作数（临时值 16 字节一槽，保持 sp 对齐）
...                         ; 右操作数求值 → x0
ldr  x1, [sp], #16          ; pop：x1 = 左，x0 = 右
sub  x0, x1, x0             ; sub/sdiv 靠操作数顺序保证语义（阶段二为 rt_sub 慢路径包裹）

// 比较（阶段一）：cset 比 AArch32 的 moveq/movne 对干净
cmp  x1, x0
cset x0, gt                 ; 阶段二在此之上 tag 成 true/false，或走 rt_gt 慢路径

// 整数字面量：load_imm64(x0, 0x0123456789abcdef) 的确定展开
movz x0, #0xcdef
movk x0, #0x89ab, lsl #16
movk x0, #0x4567, lsl #32
movk x0, #0x0123, lsl #48

// if/else：label 计数器 .L0/.L1…
// truthiness 语义（对齐两个 VM 的 is_truthy）：false 与 null 为假，其余一切为真（包括 0）。
// v1 统一调运行时判定，内联双比较留作优化（§14）：
bl   rt_truthy
cbz  x0, .L0

// 全局变量：单一 g_globals 数组，示例为 Symbol.index == 3
adrp x8, g_globals
add  x8, x8, :lo12:g_globals
ldr  x0, [x8, #24]           ; 大 slot 偏移改走统一地址物化助手

// 字符串字面量：.rodata 存字节，使用点包装成堆对象
adrp x0, .Lstr0
add  x0, x0, :lo12:.Lstr0
mov  x1, #5                 ; 字节长度
bl   rt_string_from_bytes
```

任意来自源码或 AST 计数的整数都必须经过 emitter 的 `load_imm64(reg, value)`，不能假定
`mov reg, #imm` 可编码。该助手固定展开为一条 `movz` 加零到三条 `movk`（可省略为零的半字），
不依赖有距离上限的 literal pool；字符串长度、全局/局部偏移、参数个数、捕获数都复用它。
上例中的 `mov #5` 和 `ldr #24` 只是已证明可编码时的短形式。

函数帧（AAPCS64）：

```asm
stp  x29, x30, [sp, #-16]!  ; prologue
mov  x29, sp
sub  sp, sp, #FRAME
...
mov  sp, x29                ; epilogue
ldp  x29, x30, [sp], #16
ret
```

生成的 C 入口 `main` 使用同样的保存/恢复序列；无论最后一个 Monkey 值是什么，正常路径在
epilogue 前都显式 `mov w0, #0`。最终值已在此之前通过 §10 的观测通道提交，绝不能把 tagged
value 或某个 `rt_*` 的残留返回值当作进程退出码。
`--observe` 构建的初始化顺序固定为 `rt_observer_init(3)`、`rt_globals_init(...)`、用户代码，
确保初始化以后发生的任何 fatal 都能写 error record；普通构建跳过第一步。

帧布局与寻址规则：

- 槽数 = `1（闭包槽）+ SymbolTable::num_definitions`（后者已含参数、方法的隐式 `this` 与
  全部 `let` 局部）；`FRAME = 16 × 槽数`，天然 16 对齐。
- 闭包槽在 `[x29, #-16]`；符号 `i` 的槽在 `[x29, #-16*(i+2)]`。
- prologue 把 `x1..x{num_parameters}` spill 到符号槽 `0..num_parameters-1`
  （方法的 `this` 就是符号 0，对齐 `compile_method` 先 `define("this")` 的顺序），
  `x0` spill 到闭包槽；其余局部槽写入 `null` 常量初始化。
- **立即数上限是硬约束**：负偏移 load/store（`ldur`/`stur` 形式）只覆盖 `[-256, 255]`，
  即最多 16 个槽直接寻址；`sub sp` 的立即数上限 4095。emitter 的帧寻址助手必须处理：
  偏移超限时先用 `load_imm64(x8, off)` 物化任意 64 位偏移，再
  `sub x8, x29, x8` 后经 `x8` 访问；不得发出并不总能编码的 `mov x8, #off`，
  大帧拆多条 `sub` 或用 `LSL #12` 形式。这是 emitter 单元测试的重点用例。

### 6.1 单遍 lower 与延迟拼装

“单遍”只表示 AST 节点不被重复遍历，不表示汇编文本必须流式写入一个字符串。`Emitter` 至少维护
`main_text`、`function_texts`、`rodata`、`bss` 四类 buffer。lower 一个函数或方法时：

1. 进入新的 `SymbolTable`；具名函数先定义 function name，再按顺序定义参数。
2. 把函数体 lower 到独立 `FunctionBuffer`，期间可以继续产生嵌套函数 buffer。
3. body 完成后读取最终 `num_definitions` 与 `free_symbols`，计算 `FRAME`，在 body 前拼入
   prologue/spill/null 初始化，在所有返回路径补统一 epilogue。
4. 把成品函数追加到 `function_texts`；回到父 buffer 后才按 `free_symbols` 正序装载捕获值并调用
   `rt_closure`。最终 `.text` 顺序为 `main`（以 `ret` 结束）后接全部函数，执行流不会落入函数体。

这同时解决 prologue 早于帧大小、closure 构造早于捕获列表以及函数定义落入 `main` 控制流的问题，
而不引入 IR 或第二次 AST 遍历。

### 6.2 span 传播

`emitter.rs` 维护 **span 栈**，而不是不可恢复的单游标。`lower.rs` 通过
`with_span(node.span(), |emitter| ...)` push 当前 span，并在闭包返回时无条件 pop；遍历完子节点后，
父节点 span 因此自动恢复。没有源码归属的合成 prologue/epilogue/label 使用 `span: None`。
每条真正对应运算的指令必须取该运算节点的 span；相关单元测试至少覆盖嵌套 infix、if、call 和
函数字面量，确保父运算不会误用最后一个子节点的 span。该映射供 snapshot 与 playground 使用（§12）。

## 7. 函数与闭包的调用约定

与原书最大的分歧点：原书函数是二等公民、按名字 `bl`；Monkey 函数是一等值且有闭包。约定：

- **`x0` = tagged closure Value（隐藏参数），用户可见参数 `x1`–`x7`**。
  普通函数最多 7 个参数；方法/constructor 的 `this` 占 `x1`，用户参数 `x2`–`x7`，最多 6 个（§2.2）。
- 自由变量：v1 一律经 `rt_get_free(closure, index)` 读取（index 来自 `free_symbols`），
  闭包堆布局对汇编不透明，与 §5.2 一致；按固定偏移直接 load 是 §14 的优化项。
- **具名函数的自引用**：lower 进入函数作用域后，若 `FunctionDeclaration.name` 非空
  （parser 在 `let f = fn(...)` 时回填该字段，见 `parser/lib.rs` 的 let 解析），
  必须显式调用 `SymbolTable::define_function_name`；此后该名字解析为
  `SymbolScope::Function`，codegen 读闭包槽（即 spill 后的 `x0`）。
  不做这一步，名字会落到外层 global/local/free——语义与性能都不对。
  现有字节码编译器恰好缺这一步，见 §13。

**第 3 章的过渡路径**：该章尚无 tagged value 和 runtime，只接受“已知顶层函数标识符”形式的调用；
callee 与递归目标直接 `bl .LfnN`，`x0` 传无语义的 0 占位，参数仍放 `x1..x7`，从一开始就沿用
最终帧布局。IIFE、函数值、builtin 动态分发到第 6 章才开放。第 6 章会删除这条临时 lower 路径，
所有普通调用改走 `rt_call`；§14 所说的直接 `bl` 优化是此后基于静态证明重新引入，二者不是同一实现。

### 7.1 调用点栈布局

第 6 章起，调用序列**预分配一块 8 字节紧凑的参数区**（总大小 16 对齐），正序填充；
求值各表达式期间的临时压栈发生在参数区之下，每次求值结束 sp 回到区基址再 `str`：

```asm
// f(a, b)：区大小 = align16(8 × (1 + argc)) = align16(24) = 32
sub  sp, sp, #32
...求值 callee → x0
str  x0, [sp]               ; 槽 0：callee
...求值 a → x0
str  x0, [sp, #8]           ; 槽 1：arg0
...求值 b → x0
str  x0, [sp, #16]          ; 槽 2：arg1
ldr  x0, [sp]               ; x0 = callee
mov  x1, #2                 ; x1 = argc
add  x2, sp, #8             ; x2 = argv（正序、连续、8 字节步长）
bl   rt_call
add  sp, sp, #32            ; 释放参数区
```

栈图（低地址在下）：

```
sp+0   callee     ← ldr x0, [sp]
sp+8   arg0 (a)   ← argv 基址
sp+16  arg1 (b)
sp+24  padding（16 对齐）
```

`argv` 因此是一个普通的正序连续 `*const u64` 数组，运行时可直接按切片读取；
指针只在本次调用内有效，被调方不得保留（§8 约束）。`Expression::FunctionCall` 使用上面的
`rt_call`；`Expression::New` 使用完全相同的参数区，但最后一条改为 `bl rt_construct`。
调用种类由 AST 节点决定，绝不能靠 callee 的运行时类型把二者合并。
示例中的 `#32`、slot offset 和 `#2` 都是可编码短形式；大 variadic builtin 调用必须复用 §6 的
栈调整、地址物化与 `load_imm64` 助手，不能把调用区大小或 argc 默认为 12/16 位立即数。

### 7.2 普通调用与构造分发

普通调用的 callee 在编译期不知道是 closure/builtin/bound method，统一走
`rt_call(callee, argc, argv)`（对应 `gc/vm.rs` 的 `callee_kind` 分发）：

- **closure**：校验 `argc == num_parameters`，按 argc match 出对应元数的函数指针调用
  （`f(closure)`、`f(closure, a1)`……），实参装入 `x1..`；
- **builtin**（低 3 位 `101`）：按 id 直接调 `runtime_core` 内的实现；
- **bound method**：受体注入为第一实参（`x1`），用户实参后移，等效 argc+1 调用其 closure；
- **class**：产生 `NotCallable`（`class C {}; C()` 必须报“必须使用 new”），不得隐式构造；
- 其他值：产生 `NotCallable`。

`new C(...)` 独立 lower 为 `rt_construct(callee, argc, argv)`：callee 必须是 class，否则产生
`NotConstructable`；成功时分配实例，有 constructor 则以实例为 `this` 调用，最终一律返回实例
（对齐 `compile_method` 对 constructor 强制 `OpGetLocal 0; OpReturnValue` 的语义）。无 constructor
时只允许零个参数。里程碑 0 的手写 ABI 用例必须同时覆盖 `C()` 失败、`new C()` 成功和
`let f = fn() {}; new f()` 失败，防止两条入口再次合并（`new` 的现有语法只接受标识符 callee）。

"callee 是已知顶层函数时直连 `bl`"是 §14 的优化项，不进首版。

## 8. 运行时 ABI

`Value = u64`（tagged，见 §5.2）。所有函数 `extern "C"`；**任何错误不得 panic 越过 FFI 边界**，
统一走带稳定 `RuntimeErrorKind` 的 `rt_fatal`；所有 `argv`/指针参数仅在调用期间有效，运行时不得保留。
所有 `(ptr, count/len)` 对在长度非零时必须非 null、满足元素对齐且覆盖完整区间；长度为零时允许 null，
FFI 壳直接使用空 slice，绝不能对 null 调 `slice::from_raw_parts`。
语言语义以 §10.1 的冻结矩阵为准，不再用“照抄某个现有 VM 分支”代替规范。

| 函数                                | 签名                                                                                 | 语义要点                                                      |
| ----------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------- |
| `rt_globals_init`                   | `(base: *mut Value, count: u64)`                                                     | 全区写 `null` 并注册（供未来 GC 扫 root）                     |
| `rt_string_from_bytes`              | `(ptr: *const u8, len: u64) -> Value`                                                | UTF-8 字节 → 堆 String                                        |
| `rt_box_int`                        | `(raw: i64) -> Value`                                                                | 超 SMI 范围整数装箱                                           |
| `rt_array`                          | `(argv: *const Value, len: u64) -> Value`                                            | 元素正序                                                      |
| `rt_hash`                           | `(argv: *const Value, pairs: u64) -> Value`                                          | `k0,v0,k1,v1…`；键不可哈希 → fatal                            |
| `rt_closure`                        | `(code: *const u8, num_parameters: u64, free: *const Value, num_free: u64) -> Value` | code 为函数 label 地址                                        |
| `rt_get_free`                       | `(closure: Value, index: u64) -> Value`                                              | v1 自由变量唯一读取路径                                       |
| `rt_class`                          | `(name: *const u8, len: u64) -> Value`                                               | 空类骨架                                                      |
| `rt_class_add_method`               | `(class: Value, name: *const u8, len: u64, method: Value, is_ctor: u64)`             | 逐个安装方法/constructor                                      |
| `rt_get_property`                   | `(obj: Value, name: *const u8, len: u64) -> Value`                                   | 字段优先，其次装配 bound method；缺失 → fatal（对齐 VM 报错） |
| `rt_set_property`                   | `(obj: Value, name: *const u8, len: u64, v: Value)`                                  | 仅实例可写                                                    |
| `rt_index`                          | `(obj: Value, idx: Value) -> Value`                                                  | 数组越界/哈希缺键 → `null`（对齐 VM）                         |
| `rt_add` `rt_sub` `rt_mul` `rt_div` | `(l: Value, r: Value) -> Value`                                                      | SMI/boxed 任意组合；checked `i64`；`rt_add` 兼字符串拼接      |
| `rt_eq` `rt_neq` `rt_gt`            | `(l: Value, r: Value) -> Value`                                                      | eq/neq 按 §10.1 相等矩阵；gt 只接受整数；返回 true/false 常量 |
| `rt_minus` `rt_bang`                | `(v: Value) -> Value`                                                                | `checked_neg`；`bang(v) = !truthy(v)`                         |
| `rt_truthy`                         | `(v: Value) -> u64`                                                                  | 0/1；falsy = `false` 与 `null`，其余为真                      |
| `rt_call`                           | `(callee: Value, argc: u64, argv: *const Value) -> Value`                            | 只做普通调用；class → `NotCallable`                           |
| `rt_construct`                      | `(callee: Value, argc: u64, argv: *const Value) -> Value`                            | 只做 `new`；非 class → `NotConstructable`                     |
| `rt_observer_init`                  | `(fd: u64)`                                                                          | 仅 `--observe` 产物在 main 开头调用，注册结构化记录通道       |
| `rt_observe_result`                 | `(v: Value)`                                                                         | 向 observer 写一个 framed `ok` 记录（§10.2）                  |
| `rt_fatal`                          | `(kind: u64, msg: *const u8, len: u64) -> !`                                         | 写可选 observer error 记录、stderr 消息、`exit(1)`            |

builtin（`len`/`first`/`last`/`rest`/`push`/`puts`，及别名 `print`）不单列 FFI 符号，
统一经 `rt_call` 按 id 分发到 `runtime_core` 内的实现；`puts`/`print` 的结果是 `null`。arity/type
错误是终止型 `ArityError`/`TypeError`，不产生可继续参与运算的 Error value；现有引擎的对应迁移是
里程碑 0 前置工作（§10、§13）。

`RuntimeErrorKind` 是 `.s` 与静态库之间的冻结 ABI，首版编号如下；测试比较 `kind`，不比较可能改进措辞的
人类消息：

| 编号 | 名称               | 用途                                                    |
| ---- | ------------------ | ------------------------------------------------------- |
| 0    | `InternalError`    | ABI 破坏、非法 tag 等实现错误；不得出现在正常差分语料中 |
| 1    | `TypeError`        | 运算、属性、索引或 builtin 的值类型错误                 |
| 2    | `ArityError`       | 函数、方法、constructor 或 builtin 参数个数错误         |
| 3    | `NotCallable`      | 普通调用的 callee 不可调用；class 也属于此类            |
| 4    | `NotConstructable` | `new` 的 callee 不是 class                              |
| 5    | `MissingProperty`  | 实例字段和方法均不存在                                  |
| 6    | `InvalidHashKey`   | hash key 不是 integer/boolean/string                    |
| 7    | `DivisionByZero`   | 整数除零                                                |
| 8    | `IntegerOverflow`  | checked `i64` 运算越界，包括 `MIN / -1`                 |
| 9    | `ResourceLimit`    | 参数、栈、分配或观测记录超过实现上限                    |

### 8.1 可复用 core 与两种 backend

仅把 `extern "C"` 壳移出 `runtime_core` 不足以让 native 与模拟器共用逻辑。core 不负责退出进程，
而是把所有失败返回为稳定类别：

```rust
struct RuntimeFailure {
    kind: RuntimeErrorKind,
    message: String,
}
type RuntimeResult<T> = Result<T, RuntimeFailure>;
```

core 中的运算写成 `fn op(store: &mut impl ValueStore, ...) -> RuntimeResult<T>`；`puts`/`print` 额外接收
`OutputSink`，native 使用 stdout sink，模拟器与测试使用字节 buffer。native FFI 壳把 `Err` 交给
`rt_fatal`；模拟器编码同一个 error observer record 后停止，不会调用进程级 `rt_fatal`。

存储 backend 的约束如下：

- native `PointerStore` 用显式 `#[repr(align(8))]` 的 cell 包装 `UnsafeCell<HeapObject>`，把
  `Box<HeapCell>` 地址编码成低位 `001`，
  `Box::leak` 保持地址稳定；所有可变访问只经 store API，避免从多个裸 `&mut` 造成别名 UB；
- wasm 模拟器的 `HandleStore` 使用 `((arena_index as u64) << 3) | 0b001`（同样是低位 `001`），
  只由 store 解码，绝不把 wasm host 指针冒充被模拟的 AArch64 地址；
- closure 中保存 `CodeHandle = u64`，native 解释为函数地址，模拟器解释为 label/指令索引。

`runtime_core::dispatch_call`/`dispatch_construct` 不直接调用代码指针，而返回
`RuntimeResult<CallDispatch>`：

```rust
enum CallDispatch {
    Return(Value),
    Invoke {
        code: CodeHandle,
        closure: Value,
        args: Vec<Value>,
        return_policy: ReturnPolicy, // Direct 或 ConstructorInstance(Value)
    },
}
```

native `runtime.rs` 对 `Invoke` 按 0–7 元数选择 `extern "C" fn` 签名并调用；模拟器则压入 continuation、
把参数写入模拟寄存器并跳到 `code` 对应的 PC，函数返回后再应用 `return_policy`。builtin 直接得到
`Return`。因此 PointerStore/HandleStore、真实函数指针/模拟 PC 都有明确适配层，`rt_call` 和
`rt_construct` 的语义核心仍只实现一次。

- 内存策略：v1 `Box::leak`，永不回收（原书同款）；第 8 章可选换成 `gc` crate + 精确栈扫描。
- 已知坑：**阶段二起不得从汇编调 `printf`**（变参 ABI 平台差异，Darwin 走栈）；
  阶段一的 `printf` 用法是 Linux-only 的临时通道（§5.1），阶段二删除。

## 9. 平台与工具链

- 汇编方言只取 **Linux GNU as**：无符号前缀、`:lo12:` 重定位、`.bss/.rodata` section 名。
- x86_64 开发机全链路可跑：`aarch64-linux-gnu-gcc` 交叉汇编链接 + `qemu-aarch64` 用户态执行
  （原书 QEMU 附录的同款流程）。任何阶段都不能用 host 原生 `cc` 汇编 arm64 文本。
- CI 两条腿：x86_64 job 跑单元/`.s` 快照，并安装交叉工具链与 QEMU 跑 ABI、端到端差分；
  arm64 runner job 原生重跑同一端到端集合。具体 hosted/self-hosted runner label 留在 workflow 配置，
  不写入设计 ABI。
- macOS 本地运行：后续在 emitter 加一个方言开关（`_main` 前缀、`@PAGE/@PAGEOFF`、
  section 名映射），不在首版范围。

## 10. 测试策略与观测协议

### 10.1 语言语义冻结矩阵

现有 interpreter、普通 VM 与 gc VM 不是天然一致的规范：例如 null 的 `!`、aggregate 相等、
builtin error 和 HashMap Display 都存在差异。里程碑 0 必须先合入一个语义一致性 PR，三引擎与新后端
共同遵守下面这张矩阵；在它完成前不得宣称完整差分通过，也不得用排除用例掩盖差异。

| 主题               | 冻结语义                                                                                               |
| ------------------ | ------------------------------------------------------------------------------------------------------ |
| truthiness / `!`   | 只有 `false`、`null` 为假；`!v` 严格等于 `!truthy(v)`，因此 `!null == true`、`!0 == false`             |
| 整数               | 值域为 `i64`；SMI/boxed 只是表示差异；所有算术按 §5.2 checked-fatal，除零单独分类                      |
| scalar 相等        | integer 按 raw 值（SMI 与 boxed 可相等），boolean/string/null 按值，builtin 按 id                      |
| aggregate 相等     | array 逐元素、hash 按键值集合递归比较，与插入/迭代顺序无关                                             |
| identity 相等      | closure、class、instance、bound method 按同一次执行中的对象身份；不同类型 `== false`、`!= true`        |
| 大小比较           | `>`/`<` 只接受 integer；其他组合为 `TypeError`                                                         |
| 索引               | 数组越界和 hash 缺键返回 `null`；错误容器/索引类型为 `TypeError`；非法 hash key 为 `InvalidHashKey`    |
| 调用/构造          | `C()` 为 `NotCallable`，`new C()` 才构造；`new` 非 class 为 `NotConstructable`；constructor 总返回实例 |
| builtin 与其他错误 | arity/type 等错误立即终止，不存在可赋值后继续执行的 Error value；类别使用 §8 的稳定枚举                |

identity 对象的地址只在单次引擎执行内部有意义，跨引擎测试身份时必须让 Monkey 程序先执行 `==`，
再比较所得 boolean，不能比较 native 指针或 GC id。

语义一致性前置 PR 至少包含：

1. interpreter、普通 VM、gc VM 全部改用 checked 整数运算和同一 `RuntimeErrorKind` 映射；
2. 普通 VM/gc VM 的 bang 改为 truthiness 的逻辑反值；
3. 三引擎实现上表的 aggregate/identity 相等规则；
4. builtin 返回的 `Object::Error`/`Value::Error` 在调用边界立即转成终止型错误；
5. 修复 §13 的 `define_function_name` 路径；
6. 抽出共享的规范化 language display 与 observer value encoder。

### 10.2 独立、带帧的观测协议

stdout 是 Monkey 程序所有 `puts`/`print` 副作用的原始字节流，绝不再混入 `=> ` 标记或最终值。
差分构建使用 `--observe`：harness 在启动程序前把一条专用 pipe 安装为 **fd 3**；生成的 `main`
开头调用 `rt_observer_init(3)`，正常结束前调用一次 `rt_observe_result`。普通 `build/run` 不初始化
observer，也不会访问 fd 3。

observer 恰好写一条记录，wire format 为 `u64` big-endian payload 长度 + UTF-8 JSON payload。
JSON 字符串按标准转义，所以换行、`=> ` 和任意用户输出都不会破坏 framing：

```json
{"status":"ok","value":{"type":"integer","value":"9223372036854775807"}}
{"status":"error","kind":"IntegerOverflow"}
```

正常记录由 `rt_observe_result` 写出并以退出码 0 结束；`rt_fatal` 在 observer 已初始化时先写 error
记录，再把详细消息写 stderr 并 `exit(1)`。协议只比较稳定的 `kind`，stderr 仅保留作诊断，不做逐字
差分；未写记录、多写记录、长度不符、signal 退出或“error 记录 + 退出码 0”都算 harness 失败。

`CanonicalValue` 采用以下精确的 tagged JSON 形状，而不是 Display 字符串：

| Monkey 值               | JSON value                                                                              |
| ----------------------- | --------------------------------------------------------------------------------------- |
| integer                 | `{"type":"integer","value":"-42"}`（十进制字符串覆盖完整 i64）                          |
| boolean / string / null | `{"type":"boolean","value":true}` / `{"type":"string","value":"s"}` / `{"type":"null"}` |
| array                   | `{"type":"array","elements":[<CanonicalValue>...]}`                                     |
| hash                    | `{"type":"hash","entries":[{"key":<CanonicalValue>,"value":<CanonicalValue>}...]}`      |
| closure                 | `{"type":"function"}`                                                                   |
| builtin                 | `{"type":"builtin","id":"puts"}`（`print` 也规范化为共享 id `puts`）                    |
| class / instance        | `{"type":"class","name":"C"}` / `{"type":"instance","class":"C"}`                       |
| bound method            | `{"type":"bound_method","class":"C","method":"m"}`                                      |

hash `entries` 按 `(key type rank, canonical key bytes)` 排序：rank 固定为 integer=0、boolean=1、string=2；
对应 bytes 分别为十进制 UTF-8、ASCII `false`/`true`、原始 UTF-8。closure/class/instance/bound method
不编码地址；其身份语义按上一节在程序内部测试。程序以无值语句结尾时结果为 null。

`puts` 使用共享的 **language display**：整数十进制、boolean 小写、null 为 `null`、string 原样、
array 为 `[a, b]`、hash 为 `{k: v}` 且使用上面的稳定键顺序；closure 为 `[function]`、builtin 为
`[builtin function]`、class 为 `[class C]`、instance 为 `[object C]`、bound method 为
`[bound method C.m]`。每个参数的 display 后写一个 `\n`，零参数不写 stdout，返回值始终为 `null`；
因此字符串自身包含的换行只是普通 stdout 字节，harness 绝不按“行”解析。interpreter 与两个 VM 的
测试 runner 注入可捕获的输出 sink，并直接产生同样的 observer record。

差分 harness 先验证 frame 并把 JSON 解析为上述结构，再比较三元组：

```text
(stdout_exact_bytes, observer_record, exit_class)
```

`exit_class` 固定为 `success`、`runtime_error`、`signal(n)` 或 `protocol_failure`；合法引擎结果只能是
前两类，signal、缺失/多余/损坏的 observer record 都直接使测试失败。

### 10.3 测试分层

沿用仓库现有惯例（`*_test.rs` 共置 + `insta` 快照）：

1. **emitter 单元测试**：label 分配、`load_imm64` 全边界、大帧/大 slot、全局重绑定、main 退出码、
   span push/pop 与函数 buffer 拼装。
2. **`.s` 快照测试**：`lower` 是纯函数，`insta` 快照生成的汇编文本；任何架构可跑，
   是 code review 汇编产物变化的主通道。
3. **runtime_core 双 backend 测试**：同一组值/调用语料分别跑 PointerStore 与 HandleStore，比较
   `CanonicalValue`、错误类别和 `CallDispatch`，包括 constructor 的 return policy。
4. **手写 `.s` ABI 冒烟**（里程碑 0 产物，§11）：不经 lower，直接调全部 FFI；至少覆盖
   0/7 参数、栈对齐、SMI↔boxed、三个 i64 溢出边界、`rt_call`/`rt_construct` 区分、observer framing。
5. **端到端差分测试**：语义前置 PR 完成后，现有 interpreter/VM 测试语料逐条编译执行
   （qemu 或真机），按 §10.2 比较；`examples/` 下程序全量运行，不保留局部递归或 Display 排除项。

## 11. 分章实施路线

每章一个能独立验证的里程碑：

| 章        | 内容                                                                                                                                                                                                                                                          | 里程碑                                       | 预估     |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | -------- |
| 0         | **语义与 ABI 冻结**：先合入 §10/§13 的三引擎语义一致性 PR；实现 observer framing、`runtime_core` + `ValueStore`、`PointerStore`/`HandleStore` 双 backend、`rt_call`/`rt_construct` 分流；冻结值编码、帧/调用栈图和全部 FFI，并让 5–10 个手写 `.s` 用例过 qemu | 语义基线与 ABI 的可执行规约成立              | 2 天     |
| 1         | emitter 骨架 + `main` + 整数算术 + printf 打印                                                                                                                                                                                                                | 算术表达式打印结果                           | 半天     |
| 2         | let（全局/局部）+ if/else + 比较                                                                                                                                                                                                                              | 条件逻辑                                     | 半天     |
| 3         | 顶层函数 + 临时 direct `bl` 调用 + 递归（裸 i64）                                                                                                                                                                                                             | **fib**；仅为教学子集，不进入完整差分        | 1 天     |
| 4         | tagged value lowering + boxed integer + 接入已冻结的 runtime + truthiness + checked `i64` 算术                                                                                                                                                                | 完整 `i64` 值域与稳定 overflow/div-zero 错误 | 1–1.5 天 |
| 5         | string/array/hash/index                                                                                                                                                                                                                                       | 数据结构                                     | 1 天     |
| 6         | 删除第 3 章临时路径 + builtins（含 `print` 别名）+ 一等函数 + 闭包 + 普通调用统一走 `rt_call`                                                                                                                                                                 | builtin 动态分发、闭包计数器、局部递归       | 1–1.5 天 |
| 7         | class/`new`/`this`/property + 构造统一走 `rt_construct`                                                                                                                                                                                                       | 全语言特性的四引擎差分套件通过               | 1 天     |
| 8（可选） | 对接 `gc` crate（精确栈扫描 root）替换 leak                                                                                                                                                                                                                   | 真 GC                                        | 另计     |

第 1–3 章即原书 Part I 的完成线；里程碑 0–7 合计约 8–9 个工作日达到四引擎语义与特性对齐，
其中第 1–3 章只验证教学子集，完整差分从第 4 章的最终值表示与错误语义启用后开始累计。

## 12. Playground 演示

复用现有基建：`sourceSpan.ts`、tagged JSON envelope（`status/stage/span`）、
执行预算先例（`PLAYGROUND_GC_INSTRUCTION_BUDGET`）。

### V1：Godbolt 式双栏视图（随第 1 章上线）

- wasm 新导出 `compile_to_arm64(input)`，envelope 返回逐行汇编与 span：

  ```json
  {
    "status": "ok",
    "lines": [
      {
        "text": "  sub x0, x1, x0",
        "kind": "code",
        "span": { "start": 12, "end": 17 }
      },
      { "text": ".L0:", "kind": "label" }
    ]
  }
  ```

- 新增 **ARM64 tab**，与 AST / bytecode / snapshot 视图并列：左源码右汇编，
  双向 hover 联动高亮；同一 span 在 AST 树、字节码、arm64 汇编三视图联动，
  形成"一个前端、两个后端"的教学画面。
- 下载 `.s` 按钮（对齐 `.mbc` download），旁附可复制的交叉汇编链接命令，
  明确浏览器内不真跑 arm64。默认示例 fib。

### V2：单步执行模拟器（第 4 章后）

- 生成的指令子集自定义（约 30 条）、运行时边界只有 §8 那张表，因此在 Rust/wasm 里
  做**文本级** aarch64 子集模拟器可行：解析自己 emit 的 `.s`，维护 x0–x30/sp/pc 与模拟栈，
  `.rodata`/全局区/参数区由独立的模拟内存提供；带指针参数的 ABI adapter 先从模拟内存读取
  byte/value slice，再调用 core，绝不把 wasm host 指针冒充 AArch64 地址。
- 模拟器使用 arena-index **`HandleStore`** 保存堆对象；closure 的 opaque `CodeHandle` 保存函数 label
  对应的模拟 PC。算术、容器、属性、builtin 等不会进入 Monkey 函数体的 `rt_*` 原语经 adapter
  调用 `runtime_core`，得到 value 或稳定错误类别。
- `bl rt_call`/`bl rt_construct` 先调用 `dispatch_call`/`dispatch_construct`：`CallDispatch::Return(v)`
  把 `v` 写入模拟 `x0` 并继续下一条指令；`CallDispatch::Invoke` 则压入含 return PC 与
  `return_policy` 的 continuation，把 closure/实参写入模拟 `x0..x7`，再把 PC 切到 `CodeHandle`。
  被调函数执行 `ret` 时，模拟器应用 `Direct` 或 `ConstructorInstance` policy 后恢复 continuation。
  因而 native 的真实函数指针调用与 wasm 的模拟 PC 切换只共享语义 core，不共享地址表示或控制流适配。
- UI：寄存器面板 + 栈帧可视化 + 单步/断点，pc 高亮当前汇编行并经 span 联动源码行；
  指令预算兜底防死循环。
- 差分按钮：同一程序跑 gc VM 与模拟器，按 §10.2 并排比较原始 stdout、observer record 与
  exit class；结构化差异可直接定位到值或错误类别。

### 明确不做

浏览器内真执行 arm64（unicorn.wasm / 服务端 qemu）：成本高、黑盒、教学增量低于 V2 模拟器。
真机验证由 CLI + arm CI 承担，playground 负责"看懂"。

## 13. 已知前置问题

**现有字节码编译器从未调用 `define_function_name`**（仓库内该方法零调用点），
因此 `SymbolScope::Function` → `OpCurrentClosure` 的 emit 路径（`compiler.rs` 的
`load_symbol`）实为死代码。后果：`let f = fn() { f() }` 出现在**局部作用域**时，
`f` 经外层 Local 被捕获为 Free，而 `OpClosure` 构造时该局部槽尚未写入
（`Statement::Let` 先 define 后编译右值、最后才 `OpSetLocal`）——捕获到未初始化值。
顶层递归不受影响（Global 是调用时经索引晚绑定）。

这不是新后端可以暂时绕开的差异。开始端到端差分前，必须先合入一个覆盖 interpreter、普通 VM、
gc VM 的语义一致性 PR；新后端随后从同一基线实现语义。该 PR 是里程碑 0 的合入门槛，至少包含：

1. 字节码编译器在进入具名函数作用域时调用 `define_function_name`，让普通 VM 与 gc VM 的局部
   具名递归经 `OpCurrentClosure` 读取当前闭包；新后端按 §7 从闭包槽读取同名符号；
2. 三个现有引擎统一为 §5.2 的 checked `i64` 算术以及 `DivisionByZero`/`IntegerOverflow` 分类；
3. `!` 严格实现为 truthiness 的逻辑反值，并统一 §10.1 的 scalar、aggregate 与 identity 相等规则；
4. builtin arity/type error 和其他运行时错误在调用边界转为终止型错误，统一映射到 §8 的
   `RuntimeErrorKind`，不再把 error object 当作普通值继续执行；
5. 抽出三个现有引擎及新 runtime 共用的规范化 language display、`CanonicalValue` encoder 与
   observer framing，并用同一批 conformance tests 固定结果。

以上用例从前置 PR 起就是必测集合，不设局部递归、溢出、bang、aggregate equality、builtin error
或 display 的排除清单。只有该 PR 通过后才启用 §10.3 的端到端四引擎差分。

## 14. 后续演进

- macOS 汇编方言开关（本地 Apple Silicon 直接跑）。
- 已知顶层函数调用点直连 `bl`，跳过 `rt_call` 分发；已知 builtin 可跳过 callee 类型判别，
  但仍复用同一 runtime builtin dispatcher。
- 自由变量按固定偏移直接 load，替换 `rt_get_free`（届时闭包布局成为 codegen↔runtime 的冻结 ABI）。
- truthiness 内联双比较、SMI fast path 扩展到比较与其余算术。
- 对接 `gc` crate：精确栈扫描 root（fp 链 + tagged 槽 + `rt_globals_init` 注册的全局区），
  替换 leak 策略（第 8 章）。
- 远期：自带汇编器把 `.s` 降为机器码，是通往进程内 JIT 的自然台阶；
  以及 bytecode → asm 的模板后端作对照教学（同一后端两种前端输入）。
