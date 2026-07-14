# Monkey Bytecode Snapshot 设计文档

> 本文说明 Monkey 编译产物(`Bytecode`)的二进制序列化格式与读写实现,严格对齐 QuickJS 的
> `JS_WriteObject` / `JS_ReadObject` + `qjsc` 路线:把编译结果写成 `.mbc` 文件,启动时直接加载执行,
> 跳过 lexer/parser/compiler。

## 目录

1. [背景](#1-背景)
2. [目标与非目标](#2-目标与非目标)
3. [与 QuickJS 的对应关系](#3-与-quickjs-的对应关系)
4. [文件格式](#4-文件格式)
5. [模块布局与 API](#5-模块布局与-api)
6. [安全模型:三层防线](#6-安全模型三层防线)
7. [CLI 集成](#7-cli-集成)
8. [测试策略](#8-测试策略)
9. [实施切分](#9-实施切分)
10. [后续演进](#10-后续演进)

---

## 1. 背景

QuickJS 的 "snapshot" 不是 V8/XS 那种堆内存镜像,而是**编译产物序列化**:

- `JS_WriteObject(ctx, &size, obj, JS_WRITE_OBJ_BYTECODE)` 把 `JS_TAG_FUNCTION_BYTECODE` /
  `JS_TAG_MODULE` 写成 tag 流(quickjs.c 中 `BC_TAG_*` / `BC_VERSION` 一节);
- `qjsc` 用它把 JS 编译成字节码嵌入 C 数组,运行期 `JS_ReadObject` + `JS_EvalFunction` 直接执行;
- 活的运行时状态(闭包环境、全局绑定、JSContext)明确不在序列化范围内:
  `JS_WriteObjectRec` 对活函数对象直接抛 "unsupported object class"。

Monkey 这边,`compiler::compiler::Bytecode` 就是与 `JSFunctionBytecode` 对应的编译产物:

```rust
pub struct Bytecode {
    pub instructions: Instructions,                      // 主程序字节码
    pub constants: Vec<Rc<Object>>,                      // 常量池
    pub debug_info: DebugInfo,                           // 主程序 pc -> Span
    pub function_debug_info: HashMap<usize, DebugInfo>,  // 函数常量 pc -> Span
}
```

关键事实:编译器只会向常量池写入三种对象(见 `compiler.rs` 中全部 `add_constant` 调用点)——
`Object::Integer`、`Object::String`、`Object::CompiledFunction`。常量池是一棵无共享、无环的树
(函数常量通过 `OpConst`/`OpClosure` 的**索引操作数**引用,不持有指针),因此不需要 QuickJS 的
`object_list` / `BC_TAG_OBJECT_REFERENCE` 机制,序列化是纯树形编码。

但注意:`Bytecode.constants` 与 `Compiler::add_constant` 都是公开 API,类型系统并不阻止调用方
放入其他 `Object` 变体,因此写入端 API 必须可失败(见 §5)。

## 2. 目标与非目标

### 2.1 目标

- 定义 `.mbc` 二进制格式,完整往返 `Bytecode` 的四个字段。
- `monkey-gc compile foo.monkey -o foo.mbc` / `monkey-gc run foo.mbc`,运行结果与直接跑源码一致。
- debug info 可选写入,提供 `--strip`(对应 `JS_WRITE_OBJ_STRIP_DEBUG`)。
- **内存安全承诺**:加载并执行任意恶意 `.mbc` 不产生 panic/UB/异常内存增长,最坏结果是
  `SnapshotError` 或 `GcRuntimeError`。该承诺由三层防线共同兑现(§6),
  **不由 reader 单独兑现**——reader 只做静态可判定的部分。
- 格式带版本与字节码 ABI 指纹,opcode 表或 builtin 表变化后旧 `.mbc` 被明确拒绝而不是错乱执行。

### 2.2 非目标

- 不做堆快照 / VM 运行时状态快照(globals、GC 堆、调用栈)。那是 V8/XS 的能力,QuickJS 没有;
  如未来要做,另立文档,与本格式分层。
- 不序列化 `SymbolTable`(REPL 会话续命不在本期;`.mbc` 里全局变量已解析为索引,程序执行不需要符号)。
- **不做 JVM 式字节码 verifier**(CFG 构建、抽象解释栈深、类型流分析)。语义层安全
  (栈下溢、运行期类型错误、free/local 索引越界)由 VM 动态检查兜底并返回 `GcRuntimeError`,
  不做静态证明——完整 verifier 等于用第二种方式复述 VM 语义,维护成本与出错面都不划算。
- **不承诺停机**:合法编码的死循环(如 `OpJump 0`)与源码 `while(true)` 等价,运行你自己选择执行的
  程序时,终止性不属于安全边界(node/qjs 同此立场)。CLI 提供 `--max-instructions` 供沙箱场景使用(§7)。
- 不做压缩、不做字符串去重段(与 debug-info 文档同样的取舍,先保证格式可读可教学)。
- 不承诺跨版本兼容:版本或指纹不匹配一律拒绝,不做迁移。
- 不嵌入 C/Rust 数组产物(qjsc 的 `-c` 模式);只做文件形态。

## 3. 与 QuickJS 的对应关系

| QuickJS | Monkey | 说明 |
| --- | --- | --- |
| `BC_VERSION`(人肉 bump) | `FORMAT_VERSION` + 字节码 ABI 指纹 | 教学仓库指令集/builtin 表常改,指纹自动失效旧文件,防止忘 bump |
| `BC_TAG_INT32` / `BC_TAG_STRING` / `BC_TAG_FUNCTION_BYTECODE` | `TAG_INTEGER` / `TAG_STRING` / `TAG_FUNCTION` | 常量池仅三种,tag 从 1 起,0 非法(同 QuickJS) |
| `bc_put_leb128` / `bc_put_sleb128` | 同,但带长度与溢出规则(§4.1) | QuickJS reader 曾因缺校验出过 CVE |
| atom 表(`bc_atom_to_idx` / `idx_to_atom`) | 无 | atom 表是为 `JSAtom` 索引重映射;Monkey 字符串无 interning,直接内联 |
| `object_list` + `BC_TAG_OBJECT_REFERENCE` | 无 | 常量池是树,无共享/环(见 §1) |
| `JS_WRITE_OBJ_STRIP_DEBUG` / `STRIP_SOURCE` | header flags bit0 + `--strip` | |
| 字节码语义可信性交给 `JS_READ_OBJ_BYTECODE` 门控、嵌入方负责 | 静态检查(reader)+ 动态检查(VM)分层兜底 | `.mbc` 是随手分发的文件,没有"可信嵌入方"可依赖 |
| pc2line 压缩表 | `DebugInfo.pc_spans` 原样编码 | 不压缩,见 compiler-debug-info-design.md |

## 4. 文件格式

### 4.1 基本编码

- 无符号整数(长度、计数、索引、pc、span offset):ULEB128,承载 `u64`。
- `i64` 常量:SLEB128。
- **varint 硬规则**:编码最长 10 字节;第 10 字节仍带 continuation 位、或移位溢出 64 位,
  均报 `InvalidLeb128`。解出的 `u64` 转 `usize` 必须 checked(wasm32 目标上 `usize` 是 32 位,
  这不是理论问题),溢出报 `IntegerOverflow`。reader 不要求 canonical 编码,writer 恒输出 canonical。
- **资源上界规则**:一切声明尺寸不得超过输入剩余字节数——字符串/字节串长度 ≤ 剩余输入,
  条目计数(常量数、pc_span 数等,每条至少占 1 字节)≤ 剩余输入;预分配容量以该上界封顶,
  违反报 `LimitExceeded`。该规则使任意 `.mbc` 的内存开销 O(文件大小),无需任何魔法常数。
- 字符串:ULEB128 字节长度 + UTF-8 字节(读取时校验 UTF-8)。
- 字节串(指令流):ULEB128 长度 + 原始字节。指令内部操作数保持 `op_code.rs` 既有的
  BigEndian 编码原样拷贝——那是指令集自身的编码,与本封装格式正交。
- header 中的固定宽度整数为小端(LE)。

### 4.2 布局

```text
header:
  magic              4 bytes   b"MBC\0"
  format_version     u8        FORMAT_VERSION,当前 1
  abi_fingerprint    u32 LE    见 §4.3
  flags              u8        bit0 = HAS_DEBUG_INFO,其余位必须为 0

section: program
  main_instructions  bytes     Bytecode.instructions.data
  constant_count     uleb
  constants[n]:
    tag              u8
    TAG_INTEGER = 1: value     sleb(i64)
    TAG_STRING  = 2: string
    TAG_FUNCTION= 3: name      string
                     num_locals     uleb
                     num_parameters uleb
                     instructions   bytes

section: debug            (仅当 flags.HAS_DEBUG_INFO)
  main_debug:        pc_span_count uleb, 每项 { pc uleb, start uleb, end uleb }
  fn_debug_count     uleb
  fn_debug[m]:       constant_index uleb + 同 main_debug 结构
```

尾部不允许有多余字节:读完后 `buf` 必须恰好耗尽,否则报 `TrailingBytes`。

写入端必须确定性:`function_debug_info` 是 `HashMap<usize, DebugInfo>`,写出前按
`constant_index` 升序排序。同一 `Bytecode` 两次序列化必须逐字节相同(§8 的 golden
快照测试依赖这一点)。

### 4.3 版本与字节码 ABI 指纹

两道闸门,各管一层:

- `FORMAT_VERSION: u8`:**封装格式**(header/段布局/tag 语义/varint 规则)变化时人工 bump。
- `abi_fingerprint: u32`:覆盖**字节码 ABI**——即"同一段 `instructions.data` 字节在两个版本的
  VM 上是否意义相同"。指纹变化只应导致旧文件被拒绝,与 `FORMAT_VERSION` 无关。

指纹算法:FNV-1a 32 位,seed 取标准 offset basis `0x811c9dc5`;每个字段先吸收其 ULEB 长度
再吸收内容,消除拼接歧义。按序吸收:

1. 所有 opcode,按 `Opcode` 枚举序(strum `EnumIter`):判别值、`DEFINITIONS` 中的名字、
   各 operand 宽度。新增、重排、改宽度都会改变指纹。
2. 所有 builtin,按 `BuiltIns` 表序:数组下标、名字。`OpGetBuiltin` 的操作数是该数组的
   **下标**(`Compiler::new` 用 `enumerate` 注册符号),表的增删重排会让旧文件静默调用
   另一个 builtin,必须纳入指纹。

运行期惰性计算一次即可,不要求编译期常量。注意:指纹只是**兼容性哨兵**,不是完整性校验,
更不是防篡改——攻击者可以伪造匹配的 header,安全性完全依赖 §6 的三层防线。

## 5. 模块布局与 API

按仓库惯例(crate 根平铺文件),放在 compiler crate:

```text
compiler/snapshot.rs        # 编解码实现:BcWriter / BcReader(命名对齐 BCWriterState/BCReaderState)
compiler/snapshot_test.rs   # 测试,lib.rs 中注册两个 mod
```

放 compiler 而不是 gc 的原因:`Bytecode` 定义在 compiler,且序列化发生在 VM 之前,
对 Rc VM(`compiler/vm.rs`)与 GC VM(`gc/vm.rs`)同样可用。gc crate 已依赖 compiler,
CLI 侧直接调用。注意与 `gc::report::HeapSnapshot`(教学用 JSON 报告)无关,不要混用命名。
第 6 节的防线 L2/L3 改动落在 `compiler/op_code.rs` 与 `gc/vm.rs`,不在本模块。

公开 API 保持最小:

```rust
pub const FORMAT_VERSION: u8 = 1;

pub fn bytecode_abi_fingerprint() -> u32;

/// strip_debug = true 时不写 debug 段(flags.HAS_DEBUG_INFO = 0)。
/// 常量池含 Integer/String/CompiledFunction 之外的变体时失败:
/// `Bytecode.constants` 是公开字段,写入端不得假设内容合法。
pub fn write_bytecode(bytecode: &Bytecode, strip_debug: bool)
    -> Result<Vec<u8>, SnapshotWriteError>;

/// 输入按不可信数据处理,所有畸形输入返回 Err。
pub fn read_bytecode(buf: &[u8]) -> Result<Bytecode, SnapshotError>;

#[derive(Debug, PartialEq)]
pub enum SnapshotWriteError {
    UnsupportedConstant { index: usize, kind: String },
}

#[derive(Debug, PartialEq)]
pub enum SnapshotError {
    BadMagic,
    UnsupportedVersion { found: u8, expected: u8 },
    AbiFingerprintMismatch { found: u32, expected: u32 },
    UnexpectedEof,
    InvalidLeb128,
    IntegerOverflow,
    LimitExceeded,                // 声明尺寸超过剩余输入(§4.1 资源上界规则)
    BadTag(u8),
    BadUtf8,
    BadFlags(u8),
    TrailingBytes,
    InvalidInstruction(String),   // §6 L1 指令流静态校验失败,含定位信息
    DuplicateDebugEntry(usize),
    DebugIndexNotFunction(usize), // fn_debug 的 constant_index 未指向 TAG_FUNCTION
    DebugPcOutOfRange { pc: usize, len: usize },
}
```

`read_bytecode` 读回的常量重新包 `Rc::new`;`Bytecode` 各字段已具备 `PartialEq`
能力(`Instructions`/`Object`/`DebugInfo`),往返断言按字段比较即可。

## 6. 安全模型:三层防线

"恶意 `.mbc` 不 panic/UB" 是**系统性质**,由三层共同保证,每层只负责自己静态/动态可判定的部分:

### L1 reader 静态校验(compiler/snapshot.rs)

结构层(header、varint 规则、资源上界、tag、UTF-8、EOF、尾部余字节)之外,对**每段指令流**
(主程序 + 每个 `TAG_FUNCTION` 的函数体)做一次线性扫描。扫描顺带收集**指令边界集合**
(每条指令的起始 offset),然后检查:

| 检查 | 目的 |
| --- | --- |
| opcode 字节必须是已定义 opcode | 未知字节不进 VM |
| 操作数按 `DEFINITIONS` 宽度表完整,不越过流尾 | VM 读操作数不越界 |
| `OpJump` / `OpJumpNotTruthy` 目标 ∈ 指令边界集合 ∪ {流长度} | **只查 ≤ len 不够**:跳进操作数字节等于从流中解出另一套指令,是 L2 修复前的 UB 入口,修复后也会产生无意义执行 |
| `OpConst` / `OpClosure` 常量索引 < 常量数 | 常量池不越界 |
| `OpClosure` 引用的常量必须是 `TAG_FUNCTION` | VM 不对非函数建闭包 |
| `OpClass` / `OpMethod` / `OpGetProperty` / `OpSetProperty` 引用的常量必须是 `TAG_STRING` | 这些路径在 VM 中按名字解释常量 |
| `OpGetBuiltin` 索引 < `BuiltIns.len()` | builtin 表不越界 |
| `OpHash` 元素计数必须为偶数 | VM 按键值对弹栈 |
| debug 段:pc 严格递增且 ≤ 对应指令流长度;`constant_index` 指向 `TAG_FUNCTION` 且不重复 | 拒绝无意义的调试数据 |

L1 **不做**栈深、操作数运行期类型、free/local 索引与闭包实参关系等语义校验——
单条 `OpPop`、`OpGetFree 7`、对整数 `OpCall` 都能通过 L1,这是有意的分层(交给 L3)。

### L2 opcode 安全解码(compiler/op_code.rs)

`cast_u8_to_opcode` 目前是 `unsafe transmute`,越界字节是 UB 而非 panic。
**移除它是本计划的先决改动**:换成 checked 转换(strum `FromRepr` 或显式 match),
VM 取指处对 `None` 返回 `GcRuntimeError`,`Instructions::string` 同步改造。
该改动独立成 PR,对 REPL/playground 同样有价值——它消灭的是整个仓库唯一的 UB 入口,
而不只是 snapshot 的。

### L3 VM 动态防御(gc/vm.rs)

VM 指令实现已经统一返回 `Result<_, GcRuntimeError>`,缺的是把隐式 panic 点接入该通道:

- `pop_owned` / `pop_discard`:`sp == 0` 时返回 runtime error(现状是 `self.sp -= 1`
  直接下溢,gc/vm.rs:564);
- 栈上溢已有 `push_raw` 检查,帧栈上溢已有 `push_frame` 检查,维持;
- `OpGetLocal` / `OpSetLocal` / `OpGetFree`:索引相对 `base_pointer` / `cl.free.len()`
  做边界检查;
- 运行期类型错误(对非可调用对象 `OpCall`、非法操作数类型)沿既有错误路径返回。

这些检查是 O(1) 比较、发生在本就要访问的数据上;与近期"热路径 assert 降级为 debug-only"
的性能取向不冲突——**从字节码可达的检查必须是 release 检查**,debug_assert 只用于
内部不变量。

分层的理由:L1 挡住"解码即崩"与越界索引,L3 挡住"语义即崩",两层都便宜;
中间地带(静态证明栈深与类型)是 verifier 的工作,明确不做(§2.2)。

## 7. CLI 集成

扩展 `gc/main.rs`(binary `monkey-gc`),无参数时保持现有 REPL 行为不变:

```text
monkey-gc                                # REPL(现状)
monkey-gc compile foo.monkey [-o foo.mbc] [--strip]
monkey-gc run foo.mbc|foo.monkey [--max-instructions N]
```

- **按扩展名分派,不做 magic 嗅探**:`.mbc` 走 `read_bytecode`(magic 只在 reader 内部验证,
  损坏时报 `BadMagic`,而不是把它当源码丢给 parser 产生莫名解析错误);其余扩展名走 parse + compile。
- **共用 runner**:两条路径在拿到 `Bytecode` 后汇入同一个函数
  (`Bytecode → GcVM::new → run_with_budget → last_result_string / GcRuntimeError`)。
  不走 `gc::eval_source`——它经 `try_export_last_result` 导出 `Object`,class/instance
  结果会导出失败,且丢运行期错误 `Span`。runner 返回结果字符串与带 span 的结构化错误,
  错误打 stderr、非零退出码。
- **执行预算**:默认不限(与运行 `.monkey` 源码、与 node/qjs 一致,见 §2.2 停机立场);
  `--max-instructions N` 走既有 `run_with_budget` 机制,给沙箱/评测场景用。
  playground 等嵌入场景继续沿用 `DEFAULT_INSTRUCTION_BUDGET` 的既有约定。

## 8. 测试策略

- **字段级往返**:构造覆盖三种常量、嵌套函数、带 debug info 的 `Bytecode`,
  write → read 后逐字段断言相等;`--strip` 版本断言 debug 字段为空。
- **写入端拒绝**:常量池塞入 `Object::Null` / `Object::Function`,断言
  `UnsupportedConstant { index, .. }`。
- **端到端等价**:一组代表性程序(闭包捕获、递归、class/instance、array/hash、
  会触发 runtime error 的程序)分别用共用 runner 直跑与 compile → write → read → 跑,
  比较结果字符串;带 debug 的路径额外比较错误 `Span`。
- **格式守门**:小程序 `.mbc` 的 hexdump 用 `insta` 快照。规则与两道闸门对齐:
  diff **仅落在 header 的 4 个指纹字节** → 只更新 golden;diff 触及其他任何字节 →
  说明封装格式变了,必须 bump `FORMAT_VERSION` 再更新 golden。
- **确定性**:同一 `Bytecode`(含多个函数 debug 条目)序列化两次,字节相等。
- **畸形输入(定向)**:坏 magic、坏版本、坏指纹、未知 tag、越界常量索引、跳转目标落在
  操作数中间、奇数 `OpHash`、超长 varint、声明尺寸超剩余输入、尾部余字节、debug pc 乱序——
  逐条断言对应的 `SnapshotError` 变体(用例即 §6 L1 表的行)。
- **畸形输入(随机)**:对合法 blob 全位置逐字节截断 + 抽样单字节翻转,断言**不 panic**——
  `Ok` 与 `Err` 都可接受(翻转整数 payload 会得到另一个合法文件,断言 `Err` 是错的);
  `Ok` 的结果再喂给 runner 执行,同样断言不 panic(这同时测到 L3)。
- **敌意但结构合法的字节码(L3 专项)**:绕过 compiler 手工构造——单条 `OpPop`、
  `OpGetFree` 越界、对整数 `OpCall`、深递归打满帧栈——经 write/read 后执行,
  断言 `GcRuntimeError` 而非 panic。
- **指纹敏感性**:不可直接测"改 opcode/builtin 表"(需要改源码),以文档 + golden 快照兜底,
  不为此造测试脚手架。

## 9. 实施切分

按可独立合并的 PR 切,安全前置:

1. `docs(snapshot)`:本文档。
2. `refactor(compiler,gc)`:L2 + L3——`op_code.rs` 移除 `unsafe transmute` 换 checked 解码;
   `gc/vm.rs` 补齐 pop 下溢、local/free 索引边界检查。独立于 snapshot 就有价值,
   且让第 3 步的"执行不 panic"承诺可测。
3. `feat(compiler)`:`snapshot.rs` 编解码 + varint/资源上界 + L1 校验 + §8 全部 codec 测试。
4. `feat(gc)`:共用 runner + `monkey-gc compile/run` 子命令 + 端到端等价与 L3 专项测试。

## 10. 后续演进

- **堆快照(明确超出 QuickJS 范畴)**:序列化 GC 堆可达图 + globals + `SymbolTable`,
  对标 V8 startup snapshot / XS。Monkey 的 `GcClosure { func: GcRef, free: Vec<GcRef> }`
  在创建时即拷贝自由变量、无开放 upvalue,`Value::Builtin(BuiltinId)` 已是可序列化 ID,
  条件比 QuickJS 好;需要引入 QuickJS `object_list` 式的对象引用表处理共享与环。另立文档。
- **REPL 会话续命**:序列化 `SymbolTable` + 常量池,让 `compile` 产物可作为下一次编译的基座。
- **体积优化**:字符串去重段、pc_spans 差分编码。
- **wasm/playground**:`write_bytecode` 产物以 `Uint8Array` 暴露,playground 提供
  "导出/导入编译产物" 演示;预编译示例加速加载。
- **cargo-fuzz**:对 `read_bytecode` 及 "read + 有预算执行" 建 fuzz target
  (对齐 QuickJS 仓库的 fuzz.c 文化);§8 的随机翻转测试是它的 fuzz-lite 前身。
