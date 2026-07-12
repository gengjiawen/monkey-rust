# GC Playground Trial Deletion 教学增强方案

> 状态：Implemented
>
> 范围：产品、交互、telemetry 与测试方案；实现已落地于 `gc/` telemetry、`wasm` envelope 与 Playground walkthrough UI。

## 1. 决策摘要

GC Playground 不应只给 `Edges visited` 和 `Candidates` 增加两个孤立的详情列表。最终方案应把现有的 **Scan object decisions** 升级为贯穿三阶段的 **Object decision walkthrough**，让用户能沿同一批对象理解：

1. Trial deletion 减掉了哪些堆内引用；
2. 每个对象为什么成为 Candidate 或 Trial survivor；
3. Candidate 为什么可能在 Scan 中恢复；
4. 哪些对象最终被 Free cycles 释放。

核心教学公式固定为：

```text
GC 前引用计数 − 堆内入边数量 = Trial reference count
```

判断规则固定为：

```text
Trial RC > 0  → 仍有直接的非堆引用，成为 Scan 的可达性起点
Trial RC = 0  → Candidate，等待 Scan 判定
```

页面保留当前三阶段统计卡片，在其下增加两块互相配合的内容：

- **Object decision walkthrough**：以对象为主线展示 RC 计算、阶段判定和最终结果；
- **Visited heap edges**：以边为主线展示 Trial deletion 实际减掉的堆内引用。

默认仍保持报告简洁；对象公式、边明细和恢复路径是可展开的教学信息。

## 2. 当前实现与可复用数据

当前报告已经具备以下能力：

- `GcRuntime::run_gc_with_stats()` 在 `gc_decref` 后保存 `candidate_ids`；
- `gc_scan` 后将这些 ID 分成 `restored_ids` 和 `garbage_candidate_ids`；
- Scan 报告已经为两组对象生成带类型和 ID 的合成 label；
- Playground 已经展示 Restored objects 和 Garbage candidates。

因此当前数据满足以下不变量：

```text
Trial candidates = Restored objects ∪ Garbage candidate objects
```

Candidate 对象列表可以直接由现有两组 Scan 对象合并得到。真正缺失的是：

- Trial deletion 前后的引用计数；
- 每个对象被减掉的堆内入边数；
- 边的 source、target 和结构关系；
- Restored 对象为何可从 Trial survivor 到达的证明路径。

当前 `gc_edge_count()` 只在 `trace` 回调中执行计数，目标 ID 和边关系都会被丢弃。当前通用 `GcObject::trace` 也只提供目标 ID，不提供数组下标、字段名、method 名等结构信息。

## 3. 教学目标

用户完成一次 GC 后，应能从报告独立回答四个问题：

1. **减了什么？**
   Trial deletion 遍历了哪些 heap-to-heap references。
2. **为什么入选？**
   某对象为何在减掉所有堆内入边后变成 RC 0。
3. **为什么恢复？**
   某 Candidate 是否能从 Trial RC 大于 0 的对象到达。
4. **为什么释放？**
   某 Candidate 是否在 Scan 后仍不可达并留在临时候选列表。

报告应明确区分以下概念：

| 概念                | 准确定义                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| Visited edge        | Trial deletion 遍历的一次 heap-to-heap 引用；同一 source/target 出现在不同 slot 时按多条边计算 |
| Heap incoming edges | 指向单个对象、在 Trial deletion 中从其 RC 减掉的堆内引用次数                                   |
| Trial survivor      | Trial deletion 后 RC 大于 0 的对象；它仍有直接的非堆引用                                       |
| Candidate           | Trial deletion 后 RC 为 0 的临时嫌疑对象                                                       |
| Restored            | 可从某个 Trial survivor 沿堆边到达的 Candidate                                                 |
| Garbage candidate   | 无法从任何 Trial survivor 到达、Scan 后仍留在临时候选列表的对象                                |
| Freed               | 在 Free cycles 阶段实际释放的对象                                                              |

必须避免把 Candidate 直接称为 garbage 或 cycle object。一个 Candidate 可能仍可达；一个最终被释放的对象也可能只是由不可达环保活的后代，本身并不位于环中。

## 4. 页面信息架构

保留现有顶部结构：

- Program result；
- Before / After heap snapshot；
- Collector phases；
- Collected by value kind；
- tracked bytes 说明。

Value kind 不应把所有非容器值笼统归为 `Other`。Integer、Boolean、String、
Null、Error、Compiled function 与 Builtin 分开统计；`Other` 只表示无法映射为
Monkey `Value` 的 runtime object。Before / After 只列出任一 snapshot 中实际出现的
类别，并明确 heap snapshot 还包含常量、compiled functions 和 VM bookkeeping values。

将当前独立的 **Scan object decisions** 替换为统一的阶段决策区：

```text
Collector phases
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ 1 Trial deletion │ │ 2 Scan           │ │ 3 Free cycles    │
│ Edges visited 14 │ │ Restored 3       │ │ Objects freed 2  │
│ Candidates 5     │ │ Garbage 2        │ │                  │
└──────────────────┘ └──────────────────┘ └──────────────────┘

Object decision walkthrough
  [Candidates 5] [Trial survivors 8] [All graph objects 13]

  Object       RC before   Heap in-edges   Trial RC   Scan       Final
  Node#12           1            1             0      Garbage    Freed
  Array#8           1            1             0      Restored   Retained

Visited heap edges
  Showing 7 candidate-related edges of 14 visited
  Node#12 -- fields["next"] --> Node#13
  Node#13 -- fields["next"] --> Node#12
```

对象表回答“为什么做出这个判定”，边列表回答“具体减掉了什么”。两者共享相同对象 label 和 report-scoped ID。

在 Collector phases 与 walkthrough 之间另有一张 **Heap topology** 卡片：用 mermaid 把 `visitedEdges` 画成 flowchart，实线是阶段 1 实际减掉的堆内边，单个 "External refs" 伪节点以虚线按 trial RC 指向每个幸存者，代表它剩下的全部非堆引用（constants / globals / stack 不逐条区分）。被 `globalRoots`（§9.5）命名的对象直接在节点 label 里标注变量名。只有出现在至少一条 visited edge 里的对象和全部 candidate 才成为节点；孤立 survivor（多为 VM bookkeeping 值）不绘制、只计数。报告截断了边或 decision 明细、无边可画、或节点数超过上限时，卡片降级为一句文字说明——宁可不画，不画一张缺边的图。

## 5. Collector phase cards

三张统计卡继续只承担摘要职责，不在卡片内部堆叠长列表。

### 5.1 Edges visited

卡片辅助文案：

```text
Heap-to-heap references temporarily subtracted
```

tooltip 或 accessible description：

> Trial deletion 遍历的堆内引用次数。不同字段或数组位置指向同一对象时分别计数；constants 表、globals、VM stack/frames 等非堆根引用不包含在这里。

常量表值得点名：它是“看似垃圾却幸存”最常见的原因——默认示例里字符串 `"a"`、`"b"` 正是靠常量表存活的。

`Edges visited` 表示边的出现次数，不是唯一 source/target pair 数量。例如 `[value, value]` 包含两条边。

### 5.2 Candidates

卡片辅助文案：

```text
Trial reference count reached zero
```

tooltip 或 accessible description：

> 减掉所有堆内入边后引用计数变成 0 的对象。它们只是临时 Candidate，Scan 仍可能恢复其中一部分。

### 5.3 Scan 与 Free cycles

现有数字保留，但文案应使用一致术语：

- `Restored`：Candidate 可从 Trial survivor 到达；
- `Garbage candidates`：Scan 后仍不可达；
- `Objects freed`：Free cycles 实际释放的对象。

## 6. Object decision walkthrough

### 6.1 默认筛选

决策区提供三个互斥筛选：

1. **Candidates**：默认项，展示 Trial RC 为 0 的对象；
2. **Trial survivors**：展示 Trial RC 大于 0 的对象；
3. **All graph objects**：展示所有被详细记录的图对象。

默认只展示 Candidates，因为它们最能解释 Trial deletion 与 Scan 的关系。大量与图无关的孤立 scalar、constant 和 runtime bookkeeping 对象不应抢占默认视图。

**Trial survivors 视图必须预期 VM bookkeeping 噪音。** `GcVM` 用 `dup(null)` 预填全部 stack 槽（2048 个）和 globals 槽（65536 个），因此 null 对象的 RC before 与 Trial RC 都约为 67,000。这个数字是诚实的——它如实统计了非堆引用——但对初学者非常突兀，不加解释会显得像 bug。要求：

- walkthrough 各视图的展示排序为：参与已报告边或 witness 的对象在前，孤立对象（没有任何已报告出入边）在后，同组内按 ID 升序。null 这类无堆内出入边的 bookkeeping 对象自然沉底；
- 此类对象的行展开必须解释数值来源（见 §6.3），不能让一个约 67,000 的数字无解释地出现；
- §12.1 的 GcId 排序只约束 JSON 序列化顺序，不约束 UI 展示顺序，两者不冲突。

### 6.2 表格字段

| 字段           | 含义                                               |
| -------------- | -------------------------------------------------- |
| Object         | 当前已有的合成对象 label，例如 `Instance(Node)#12` |
| RC before      | Trial deletion 开始前的引用计数                    |
| Heap in-edges  | 本阶段从该对象 RC 中减掉的堆内入边次数             |
| Trial RC       | `gc_decref` 完成后的临时引用计数                   |
| Trial decision | `Candidate` 或 `Survivor`                          |
| Scan result    | `Restored`、`Garbage` 或 `Scan root`               |
| Final          | `Retained` 或 `Freed`                              |

示例：

| Object              | RC before | Heap in-edges | Trial RC | Trial     | Scan      | Final    |
| ------------------- | --------: | ------------: | -------: | --------- | --------- | -------- |
| `Instance(Node)#12` |         1 |             1 |        0 | Candidate | Garbage   | Freed    |
| `Instance(Node)#13` |         1 |             1 |        0 | Candidate | Garbage   | Freed    |
| `Class(Node)#7`     |         3 |             2 |        1 | Survivor  | Scan root | Retained |

每一行都必须可以直接读成公式：

```text
Instance(Node)#12: 1 − 1 = 0 → Candidate
Class(Node)#7:      3 − 2 = 1 → Trial survivor
```

### 6.3 行展开内容

Candidate 行可展开：

- 指向该对象的所有已报告入边；
- 该对象发出的所有已报告出边；
- Restored 时的可达性证明；
- Garbage 时的不可达说明；
- 详情是否因为 telemetry limit 被截断。

Trial survivor 行可展开：

- Trial RC，也就是本轮减掉堆内边后剩余的直接非堆引用次数；
- 从它出发恢复了哪些 Candidate；
- 它不需要在 Scan 中被“恢复”，而是 Scan 的起点之一；
- 剩余非堆引用的来源说明：constants 表、globals、VM stack 槽位等。对 null 这类被 VM 预填槽位大量持有的对象，必须明确指出“剩余引用主要来自 VM 预填的 stack/globals 槽位”，这是 §6.1 所述超大数字的出处。

### 6.4 状态措辞

统一使用：

- `Candidate`
- `Trial survivor`
- `Restored`
- `Garbage`
- `Retained`
- `Freed`

不使用：

- “Alive because it is not in a cycle”；
- “Cycle object”；
- “Root variable”；
- “The edge that actually restored this object”。

最后一种措辞依赖实际遍历顺序，而遍历顺序包含链表和 HashMap 实现细节，不适合作为稳定教学概念。

## 7. Visited heap edges

### 7.1 默认行为

边详情默认折叠。展开后默认显示 **Candidate-related**：

- Candidate → Candidate；
- Trial survivor → Candidate；
- Candidate → Trial survivor。

顶部必须同时显示完整总数和当前明细范围：

```text
Showing 7 candidate-related edges of 14 visited
```

提供两个筛选：

- `Candidate-related`
- `All visited edges`

### 7.2 展示格式

```text
Instance(Node)#12
  -- fields["next"] --> Instance(Node)#13

Instance(Node)#12
  -- class --> Class(Node)#7

Array#18
  -- items[0] --> Instance(Node)#12
```

边应按以下教学优先级展示：

1. Candidate → Candidate；
2. Trial survivor → Candidate；
3. Candidate → Trial survivor；
4. 其余边（Survivor → Survivor）。

前三类合计即 `Candidate-related` 筛选的全部内容——一条边只有 source/target 两端，涉及 Candidate 的组合只有这三种。展示优先级与 §13 的截断保留优先级一致，仅差“witness forest 使用的边”一项（它只影响截断保留，不影响展示排序）。

这样默认 class cycle 示例会优先显示真正构成循环的两条 `next` 边，同时不会隐瞒 instance → class 等其他被访问边。

### 7.3 边关系词汇

结构关系采用 typed relation，不在 Rust 端拼接 UI 字符串：

| Value 类型    | relation kind                      | UI 展示                        |
| ------------- | ---------------------------------- | ------------------------------ |
| Array         | `arrayElement` + index             | `items[0]`                     |
| Hash          | `hashValue` + key kind + key label | `values["name"]` / `values[1]` |
| Closure       | `closureFunction`                  | `function`                     |
| Closure       | `closureFree` + index              | `free[0]`                      |
| Class         | `classConstructor`                 | `constructor`                  |
| Class         | `classMethod` + name               | `methods["connect"]`           |
| Instance      | `instanceClass`                    | `class`                        |
| Instance      | `instanceField` + name             | `fields["next"]`               |
| BoundMethod   | `boundMethodReceiver`              | `receiver`                     |
| BoundMethod   | `boundMethodFunction`              | `method`                       |
| 其他 GcObject | `unknown`                          | `unknown`                      |

字段名、数组下标和 method 名是准确的结构 slot，可以展示。报告不尝试恢复变量名：同一对象可能存在多个 alias，不存在唯一“对象变量名”。

Hash relation 必须保留 `integer` / `boolean` / `string` key kind，避免字符串
`"1"` 与整数 `1` 在教学 UI 中显示成同一个 slot。String key 的展示应转义并设置
长度上限；完整 key 不应无限放大 JSON 或 UI。

## 8. Scan reachability witness

### 8.1 教学含义

对于每个 Restored Candidate，报告应提供一条从 Trial survivor 出发的可达性证明：

```text
Array#3
  -- items[0] --> Array#8
  -- items[0] --> Array#11
```

UI 文案：

> Reachability witness：这是一条确定性的可达路径，不代表 collector 的实际事件顺序。

对于 Garbage Candidate：

```text
No path from any trial survivor.
Remained in the temporary candidate list after Scan.
```

### 8.2 计算方式

不应为每个 Candidate 分别执行一次 BFS。建议在捕获的完整堆图上执行一次确定性的 multi-source BFS：

1. 所有 `Trial RC > 0` 的对象作为起点；
2. 起点按 ID 排序；
3. 每个对象的出边按稳定边顺序排序；
4. 为首次到达的对象记录 predecessor edge；
5. Restored Candidate 沿 predecessor chain 回溯到起点；
6. 未被访问的 Candidate 即为 Garbage Candidate。

报告可以保存紧凑的 witness forest，而不是为每个对象重复完整路径：

```text
objectId
rootId
predecessorId
relation
```

前端沿 predecessor 重建完整路径。所有 witness edge 在边详情截断时具有最高保留优先级。

## 9. Telemetry 数据契约

### 9.1 兼容策略

现有字段保持不变：

- `trialDeletion.edgesVisited`
- `trialDeletion.candidates`
- `scan.restored`
- `scan.garbageCandidates`
- `scan.restoredObjects`
- `scan.garbageCandidateObjects`
- `freeCycles.freed`

新字段采用 additive change。旧消费者可以忽略，新 Playground parser 必须严格验证新增字段。

### 9.2 对象目录

报告顶层增加一个 normalized object catalog。所有边、decision 和 witness 通过 ID 引用对象，避免在每条边重复长 label：

```json
{
  "objects": [
    {
      "id": 12,
      "kind": "instance",
      "label": "Instance(Node)#12"
    }
  ]
}
```

对象 ID 仍只在单次同步 report 内有效。catalog 至少包含所有被 edge、decision、witness 或现有 Scan 对象列表引用的对象。

### 9.3 Trial deletion

概念结构：

```json
{
  "trialDeletion": {
    "edgesVisited": 14,
    "candidates": 5,
    "objectDecisions": [
      {
        "objectId": 12,
        "refCountBefore": 1,
        "heapIncomingEdges": 1,
        "trialRefCount": 0,
        "decision": "candidate",
        "final": "freed"
      }
    ],
    "visitedEdges": [
      {
        "fromId": 12,
        "toId": 13,
        "relation": {
          "kind": "instanceField",
          "name": "next"
        }
      }
    ],
    "omittedObjectDecisions": 0,
    "omittedEdgeDetails": 0
  }
}
```

`decision` 只允许：

- `candidate`
- `survivor`

`final` 只允许：

- `retained`
- `freed`

`final` 在 `gc_free_cycles` 完成后回填（§10 步骤 4）：survivor 恒为 `retained`；candidate 依据是否留在 garbage 列表映射为 `freed` 或 `retained`。显式序列化该字段是为了让 §6.2 表格的 Final 列有直接数据来源，UI 不必依赖“candidate 且出现在 `garbageCandidateObjects` 中 ⇒ Freed”的跨列表 join——那种推导依赖 `Objects freed = Garbage candidates` 不变量成立，一旦未来不变量放宽（§14 末段），UI 会静默出错。显式字段同时给 parser 一个可交叉校验的锚点（§15）。

`trialRefCount` 虽可由另外两个字段计算，仍应显式序列化：它是 collector 的实际阶段结果，也便于验证报告不变量。

### 9.4 Scan witness

概念结构：

```json
{
  "scan": {
    "restored": 3,
    "garbageCandidates": 2,
    "restorationWitnesses": [
      {
        "objectId": 11,
        "rootId": 3,
        "predecessorId": 8,
        "relation": {
          "kind": "arrayElement",
          "index": 0
        }
      }
    ],
    "omittedWitnesses": 0
  }
}
```

witness forest 每个 restored 对象一条记录，理论上无上界，因此和 edge details、object decisions 一样受 §13 的上限与省略计数约束，不能成为报告里唯一没有 limit 的明细。

Garbage Candidate 不需要伪造空路径；其不在 witness forest 中，并且已经存在于 `garbageCandidateObjects`。

### 9.5 Global roots

报告顶层增加 `globalRoots`：`collect_garbage` 动手前对 VM 全局表的快照，每个已赋值的全局槽位一条 `name → objectId` 记录。

```json
{
  "globalRoots": [
    { "name": "Node", "objectId": 7 },
    { "name": "makeCycle", "objectId": 10 }
  ]
}
```

名字来自编译器符号表（`Compiler::global_symbols()`），是被定义的根集合——这不违反“不猜别名”原则（§21）：全局槽位本身就是一条被命名的根引用，报告只陈述符号表里的事实，不猜对象还有哪些局部别名。

不变量：`objectId` 必须能在 catalog 中解析；`name` 不得重复；被命名的对象若有 decision 记录，decision 必为 `survivor`——命名槽位是非堆引用，trial deletion 减不掉它，减得掉就是 collector 的 bug。

## 10. 运行时采集流程

只增强 `run_gc_with_stats()` 的只读 telemetry 路径，普通 `run_gc()` 不生成 label、边详情或 witness，避免为正常 collector 引入不必要开销。

建议流程：

1. **Trial deletion 前**
   - 保存当前 GC object IDs；
   - 生成对象 catalog；
   - 保存每个对象的 `refCountBefore`；
   - 枚举完整语义边；
   - 计算每个对象的 `heapIncomingEdges`；
   - 由完整边数产生 `edgesVisited`。
2. **执行 `gc_decref`**
   - 保存每个对象的 `trialRefCount`；
   - 保存 Candidate IDs 和 Trial survivor IDs；
   - 验证 RC 公式。
3. **执行 `gc_scan`**
   - 保存 Restored IDs 和 Garbage Candidate IDs；
   - 在步骤 1 捕获的不可变图上构造 deterministic witness forest。
4. **执行 `gc_free_cycles`**
   - 保存 Freed 数量；
   - 将对象最终状态映射为 Retained 或 Freed，回填 `objectDecisions[].final`（§9.3）。

整个过程仍然是一次不可中断的原子 collection。Playground 不获得中间阶段的可变 heap，也不暴露单阶段 GC API。

## 11. 语义边枚举的架构约束

不能在报告模块里完全复制一份 `Value::trace` 分支。否则以后增加一种 Value edge 时可能出现：

```text
collector 能遍历该边
telemetry 却没有报告该边
```

原始设想是把边定义集中为一个内部 semantic edge visitor `visit_edges(relation, target)`，让 `Value::trace` 复用它并忽略 `relation`。

**实现决策（有意偏离上面的复用设想）**：最终保留了两份手写枚举，`Value::trace` 与 `Value::visit_edges` 并存：

- `Value::trace` 保持零分配、不排序，服务普通 collection 的热路径；
- `Value::visit_edges` 为满足 §12 的确定性要求，对 hash key 与 method/field 名排序后再回调，允许分配；
- 若让 `trace` 复用 `visit_edges`，每次普通 GC 都要为 telemetry 的排序付出分配开销，得不偿失。

双实现的漂移风险由三道防线兜住：

1. 两个 match 都显式列举全部 `Value` 变体、不写 `_ =>` catch-all——新增变体时两处都编译失败，强迫作者归类它的边；
2. `report_test.rs::visit_edges_targets_match_trace_targets` 对每种带边变体断言两者产出的 target 多重集一致；
3. collection 期间 `debug_assert_eq!(edges_visited, gc_edge_count())` 校验语义捕获的边数与 collector 实际遍历的边数相等。

其余约束照常成立：

- 普通 GC 算法仍只依赖 target，不依赖任何 UI 文案；
- relation 使用轻量 enum/borrowed metadata，普通 collection 不分配 label string。

对于无法提供语义 relation 的通用 `GcObject`：

- 仍通过现有 `trace` 捕获 target；
- relation 回退为 `unknown`；
- 不得因为缺少调试 relation 而漏算或跳过 GC 边。

## 12. 确定性要求

报告必须在相同输入和相同 runtime 状态下保持稳定。

### 12.1 对象排序

按 `GcId` 升序。

### 12.2 边排序

稳定排序键：

```text
fromId
relation kind
relation name/index/key
toId
```

### 12.3 Hash、field 和 method

不得使用 HashMap 当前迭代顺序直接输出；报告层必须按 typed key/name 排序。

Hash key 是 Integer/Boolean/String 混合类型，没有天然全序，排序规则必须显式定义：

1. 先按类型，顺序为 integer < boolean < string（与 `HashKey` enum 声明顺序一致，实现可直接 `#[derive(PartialOrd, Ord)]`）；
2. 同类型内按值：整数按数值，boolean 按 false < true，字符串按字节序。

排序键使用未截断的原始 key；§7.3 的转义与长度上限只作用于展示 label，不作用于排序键，否则超长 key 截断后会破坏顺序稳定性。

### 12.4 Witness

- BFS 起点按 ID 排序；
- adjacency 使用稳定边顺序；
- 选择最短路径；
- 多条最短路径时选排序最前的一条。

报告不展示实际 GC 遍历时间线，因为链表顺序和 HashMap 顺序属于实现细节，不是稳定的教学语义。

## 13. 详情数量限制

聚合指标必须始终完整；详细 telemetry 必须有明确上限，避免大型源码产生不可控 WASM JSON 和 DOM。

初始上限：

- 最多 500 条 edge details；
- 最多 500 条 object decisions；
- 最多 500 条 restoration witnesses。

发生截断时必须返回明确数字：

```text
omittedEdgeDetails
omittedObjectDecisions
omittedWitnesses
```

UI 必须显示：

```text
500 of 1,284 visited edges shown
784 edge details omitted; aggregate count remains exact
```

不能静默截断。

详细记录的选择优先级：

1. witness forest 使用的边；
2. Candidate → Candidate；
3. Trial survivor → Candidate；
4. Candidate → Trial survivor；
5. 其余边。

第 2–4 项与 §7.2 的展示优先级完全一致（Candidate-related 的全部三种组合）；本列表只多出第 1 项，因为 witness 边被截掉会让恢复证明失效，而展示排序不受影响。

Object decision 优先级：

1. 所有可容纳的 Candidates；
2. witness path 上的 Trial survivors；
3. 其他 Trial survivors；
4. 其余图对象。

Restoration witness 按对应 restored 对象的 ID 升序保留，保证截断结果确定。

任何被已报告 edge、decision 或 witness 引用的 ID 都必须存在于 object catalog。若 limit 使这个引用完整性无法满足，应省略整条详细记录，而不是产生 dangling ID。

对 witness 的引用完整性额外加强一级：保留某条 witness 时，其 predecessor chain 上所有对象（含 `rootId`）不仅要存在于 catalog，其 object decision 记录也必须同时保留；无法同时满足时省略整条 witness 并计入 `omittedWitnesses`。这保证 §15 的 witness chain 校验（终点是 survivor、中间节点是 candidate）对每条已报告的 witness 都无条件可执行，不因 decision 截断而变成“无法验证”。

## 14. 报告不变量

Rust 实现和测试应验证：

```text
trialRefCount
  = refCountBefore − heapIncomingEdges
```

```text
Candidate
  ⇔ trialRefCount == 0
```

```text
Candidates
  = Restored + Garbage candidates
```

```text
Restored IDs ∩ Garbage IDs
  = ∅
```

```text
Candidate IDs
  = Restored IDs ∪ Garbage IDs
```

```text
Σ heapIncomingEdges（对全部对象，截断前）
  = edgesVisited
```

最后这条汇总恒等式把新的逐对象计数与现有 `gc_edge_count()` 聚合绑定在一起，是发现语义边枚举与 collector trace 漂移（§11 担心的场景）最直接的检查。它在 Rust 端对截断前的完整数据验证；截断后的 JSON 明细天然不满足求和，parser 不检查这一条。

`final` 字段的一致性：

```text
final == "freed"
  ⇔ decision == "candidate" 且对象位于 Garbage candidate 列表
```

当前 collector 语义还应满足：

```text
Objects freed
  = Garbage candidates
```

详情发生截断时：

```text
Edges visited
  = reported edge details + omitted edge details

Restored
  = reported restoration witnesses + omitted witnesses
```

如果未来某类 runtime object 使 `Objects freed = Garbage candidates` 不再成立，协议必须改为明确描述差异，不能让 UI 靠相等关系猜测——这也是 `final` 需要显式序列化而非由 UI 推导的原因（§9.3）。

不变量的验证方式：单元测试全覆盖，运行时用 `debug_assert!` 兜底。release/WASM 构建中检测到违反时不得 panic——那会以一条难懂的错误打崩 playground；应照常返回报告，让差异由测试与 CI 暴露。

## 15. TypeScript parser 与兼容性

Playground parser 应继续把 WASM JSON 当作不可信输入验证：

- 所有 count 是非负 finite number；
- object ID 是非负 safe integer；
- relation kind 属于已知枚举；
- relation-specific 字段存在且类型正确；
- `fromId`、`toId`、`objectId`、`rootId`、`predecessorId` 都能在 catalog 中解析；
- decision 只允许 `candidate` 或 `survivor`；
- final 只允许 `retained` 或 `freed`，且 `final == "freed"` 当且仅当 decision 为 `candidate` 且对象出现在 garbage candidate 列表中；
- omitted count 与聚合 count 不矛盾；
- witness predecessor chain 无环，且 `rootId` 自身不在 witness forest 中（起点不是被恢复对象）；
- 每条已报告 witness 的链终点 decision 为 `survivor`、链上中间节点 decision 为 `candidate`——§13 保证已报告 witness 引用的 decision 记录不被截断，因此该校验无条件可执行，无需“报告未截断”前提；
- `globalRoots` 的 `objectId` 能在 catalog 中解析、`name` 不重复；被命名对象若有 decision 记录则必为 `survivor`（§9.5）——decision 被截断时该项跳过，不误报。

新增字段采用 additive JSON contract，但 Rust、WASM fixture、TypeScript types、parser tests 和 UI fixture 必须在同一个变更中同步。Playground 消费的是 `wasm/pkg/`，实现完成后必须重新构建 wasm package，不能只依赖 `cargo test`。

## 16. 响应式与无障碍

### 16.1 桌面端

使用语义化 table 展示对象决策；数值列右对齐，状态列使用文字 badge。

### 16.2 移动端

二选一：

- table 保持语义并允许横向滚动；或
- 每个对象转换为分组 decision card。

不得通过隐藏 RC 列来适配窄屏，否则核心教学公式会消失。

### 16.3 无障碍要求

- filter 使用可键盘操作的 radio/tab 语义；
- 展开按钮包含对象 label；
- Candidate、Restored、Garbage 不能只靠颜色区分；
- edge relation 使用文本和箭头，不能只靠线条颜色；
- tooltip 内容同时提供给屏幕阅读器；
- truncated state 使用可见文本和 live-safe status，不依赖 hover。

## 17. 测试矩阵

### 17.1 Rust telemetry

1. **无根两节点环**
   - 两条互相引用边；
   - 两个 Candidate；
   - 两个 Garbage/Freed；
   - RC 公式成立。
2. **自引用对象**
   - 一条 self edge；
   - 一个 Candidate；
   - relation 和计数正确。
3. **有根的环**
   - 至少一个对象 Trial RC 大于 0；
   - 其余对象可先成为 Candidate；
   - 所有 Candidate 经 witness path 恢复；
   - freed 为 0。
4. **有根嵌套数组**
   - 中间和叶子 Trial RC 为 0；
   - deterministic witness 展示链式恢复。
5. **重复数组引用 `[x, x]`**
   - 报告 `items[0]` 和 `items[1]` 两条边；
   - target 的 incoming count 增加 2。
6. **Class / Instance**
   - `class` 和 `fields["next"]` relation 正确。
7. **Class methods**
   - `constructor` 与 `methods["connect"]` relation 正确。
8. **Closure**
   - `function` 与 `free[index]` relation 正确。
9. **Hash**
   - typed key label 正确；
   - 输出顺序稳定。
10. **非 Value GcObject**
    - relation 回退为 `unknown`；
    - aggregate edge count 不丢失。
11. **详情截断**
    - aggregate count 完整；
    - omitted count 正确（含 `omittedWitnesses`）；
    - witness edge 优先保留；
    - 被保留 witness 链上所有对象的 decision 记录同时保留，否则整条 witness 省略；
    - catalog 不产生 dangling ID。
12. **第二次空 GC**
    - 无新 Candidate 时 empty details 合法；
    - 所有聚合指标与明细一致。
13. **VM bookkeeping survivors**
    - null 等被 stack/globals 预填槽位持有的对象作为 Trial survivor 报告；
    - 超大 `refCountBefore` 下 RC 公式与 `Σ heapIncomingEdges = edgesVisited` 恒等式仍成立。

### 17.2 WASM

- 新字段序列化为 camelCase；
- success envelope 包含合法 object catalog、relations 和 decisions；
- error envelope 不受影响；
- truncated report 仍是合法、可解析的 JSON。

### 17.3 TypeScript parser

- 接受完整合法报告；
- 拒绝未知 relation kind；
- 拒绝缺失 relation-specific 字段；
- 拒绝 dangling object ID；
- 拒绝非法 decision；
- 拒绝非法 final，以及 final 与 garbage 列表矛盾的报告；
- 拒绝终点不是 survivor 的 witness chain；
- 拒绝负数、非整数 ID 和非 finite count。

### 17.4 Playground

- 默认 class cycle 显示两条 `fields["next"]` 循环边；
- Candidate 表格展示正确 RC 公式；
- Restored 对象展示 witness path；
- Garbage 对象展示“无 Trial survivor 可达路径”；
- Trial survivors 视图中孤立 bookkeeping 对象（null）排在参与图的对象之后，行展开可见非堆引用来源解释；
- Candidates 为 0 时有明确 empty state；
- edge details 折叠状态和 filter 可操作；
- truncated notice 可见；
- 所有状态都可通过文本读取；
- 移动端仍能看到完整公式。

## 18. 文档与构建同步

实现时同步更新：

- `docs/js-style-class-design.md` 的 collection report contract；
- `docs/gc.md` 的三阶段 telemetry；
- `gc/gc-report.md` 的 Trial deletion 和 Scan 教学段落；
- `gc/README.md`；
- `packages/playground/README.md`；
- WASM JSON 示例；
- Rust、WASM、TypeScript 和 RTL fixtures。

文档必须明确：

- object ID 只在本次 report 内有效；
- `Edges visited` 不含 constants/globals/frame 等非堆根引用；
- witness 是确定性可达证明，不是实际事件时间线；
- synthetic label 不是源码变量名；
- tracked bytes 仍只是 collector accounting proxy。

实现完成后按仓库流程重新生成 `wasm/pkg/`，再执行 Playground build/test，避免 UI 运行旧 bytecode。

## 19. 分阶段交付

### Phase A：Trial decision telemetry

- `refCountBefore`；
- `heapIncomingEdges`；
- `trialRefCount`；
- Candidate / Survivor decision；
- Retained / Freed `final` 状态（freed 集合在本阶段已可得）；
- RC 公式、`Σ heapIncomingEdges = edgesVisited` 和集合不变量测试。

Phase A **不引入语义 edge visitor**：`refCountBefore` 与 `heapIncomingEdges` 用现有泛型 `trace` 的 target 计数即可得到，typed relation 属于 Phase B。这条边界要钉死，避免实现时把 §11 的重构提前拖进来。

Phase A 完成后，即使尚未展示语义边，Object decision walkthrough 也可以解释 Candidate 的数值来源。

### Phase B：Semantic edges 与 Scan witness

- typed edge relations；
- stable edge ordering；
- multi-source BFS witness forest；
- edge/detail limits；
- additive WASM contract 和 parser validation。

### Phase C：Playground 教学 UI

- 统一 Object decision walkthrough；
- Candidate / Survivor / All filters；
- row expansion；
- Visited heap edges；
- tooltip、empty state、truncation notice；
- 响应式和无障碍；
- docs、WASM rebuild 和端到端验证。

### Phase D：Global roots 与 heap topology graph

- 报告顶层 `globalRoots`（§9.5）：symbol table → `GcVM::set_global_names` → `collect_garbage` 快照；
- walkthrough 内的全局名 chips 与 survivor 行说明；
- Heap topology 卡片（§4）：mermaid flowchart、External refs 伪节点、fate 配色与主题跟随；
- 截断 / 空图 / 超节点上限时降级为文字说明，孤立 survivor 只计数不绘制。

这些阶段可以拆成独立 commit 或 PR，但对用户可见的最终功能应在合并时保证 Rust → WASM → TypeScript → UI contract 完整闭环。

## 20. 验收标准

### 教学验收

使用默认 class cycle 示例，用户无需阅读实现代码即可指出：

1. 两个 instance 之间分别存在一条 `next` 边；
2. Trial deletion 从两个 instance 各减掉一条堆内入边；
3. 两者 Trial RC 都变为 0，因此只是先成为 Candidate；
4. Scan 无法从任何 Trial survivor 到达它们；
5. 两者最终留在临时候选列表并被释放。

使用 rooted nested array 示例，用户应能指出：

1. 中间数组成为 Candidate 不代表它是垃圾；
2. 根直接持有的外层数组 Trial RC 大于 0；
3. witness path 证明外层数组可以沿元素边到达中间和叶子；
4. Scan 因此恢复这些 Candidate，Free cycles 不释放它们。

### 技术验收

- 普通 `run_gc()` 行为和开销模型不因教学 telemetry 改变；
- `run_gc_with_stats()` 仍原子完成三阶段；
- aggregate counters 与详细记录满足全部不变量；
- report 输出确定且详情有界；
- 不出现 dangling object ID；
- Rust workspace tests、WASM tests、Playground tests 和 build 全部通过；
- 生成的 wasm package 与 Rust contract 同步。

## 21. 明确不做

本方案不包含：

- 暂停在 Trial deletion 并让用户手动继续 Scan；
- 暴露 `gc_decref` 等单阶段 public API；
- 猜测对象对应的源码变量名或局部别名（`globalRoots` 只陈述符号表已知的全局槽位，见 §9.5，不属于猜测）；
- 在拓扑图中逐条区分 constants / globals / stack 的 root 边（单个 External refs 伪节点汇总全部非堆引用）；
- 把 Candidate 直接称为 cycle 或 garbage；
- 展示不稳定的实际遍历顺序；
- 绘制无界的完整 heap graph（拓扑图只画本次访问过的边，截断或超节点上限时不绘制，见 §4）；
- 将 tracked bytes 解释为浏览器 resident memory；
- 改变 QuickJS-style 三阶段 collector 的判定算法。

## 22. 最终产品原则

这项增强的目标不是让数字“可以点击”，而是让三个阶段形成一条可以验证的因果链：

> 用边解释减了什么，用 RC 公式解释为何入选，用可达路径解释为何恢复，用最终状态解释为何释放。

当报告同时满足这四层信息时，`Edges visited` 和 `Candidates` 才从运行时统计升级为真正有教学价值的 GC 解释器。
