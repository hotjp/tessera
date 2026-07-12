# CLAUDE.md — Σ⁴-Engine AI 开发入口

> **文档性质**：AI Agent 唯一入口点（适用于 Claude、Kimi、Cursor Agent、Devin、GitHub Copilot Workspace 等任何 AI 辅助开发场景）
> **阅读协议**：任何实现本系统的 AI 必须从此文件开始阅读，不得跳过
> **版本约束**：本文档与 `SIGMA4_SPEC_v1_1.md` 绑定，不得参考任何历史版本
> **与 AGENT.md 的关系**：内容完全一致，任选其一阅读即可，不要重复阅读

---

## 1. 入口声明（必读）

如果你是 AI 助手（Claude、Kimi、ChatGPT、Cursor Agent、Devin、GitHub Copilot Workspace 等），**这是你的唯一入口文档**。

**不要做的事**：
- ❌ 不要阅读 `ARCHITECTURE.md`、`ARCHITECTURE_v2.md`、`ARCHITECTURE_v3.md`、`ARCHITECTURE_v3_1.md`、`ARCHITECTURE_v4.md` —— 这些是架构演进历史，包含已被否定的设计，阅读会导致错误实现
- ❌ 不要阅读 `SIGMA4_SPEC_v1.md` —— 这是 v1.1 的前序版本，已被修正
- ❌ 不要引入外部依赖（如 gRPC、PostgreSQL、Redis、Kafka、etcd）—— 系统的设计目标就是自包含、零外部依赖
- ❌ 不要询问用户"架构细节"或"业务需求"—— 本文档和 SPEC 已经包含全部所需信息
- ❌ 不要先写网络层再写核心逻辑 —— 必须按本文档第 3 节的顺序执行

**必须做的事**：
- ✅ 先读完本文档全文（约 2 分钟）
- ✅ 再阅读 `SIGMA4_SPEC_v1_1.md` 的指定章节（按第 3 节顺序）
- ✅ 阅读 `DECISIONS.md` —— 它记录了对 SPEC 的覆盖性决策（如 SIMD 改用 nightly `std::simd`、跨平台矩阵）。**与 SPEC 冲突时，以 DECISIONS.md 为准**
- ✅ 按第 3 节的步骤顺序开发，每步完成后再进入下一步
- ✅ 每步完成后执行第 4 节的验收测试，全部通过再进入下一步

---

## 2. 文档导航表

| 文件名 | 状态 | 说明 | AI 行为 |
|--------|------|------|---------|
| `ARCHITECTURE.md` | ❌ 历史文档 | v1.0 架构初稿 | **不读** |
| `ARCHITECTURE_v2.md` | ❌ 历史文档 | v2.0 多维单纯形 | **不读** |
| `ARCHITECTURE_v3.md` | ❌ 历史文档 | v3.0 统一实体论 | **不读** |
| `ARCHITECTURE_v3_1.md` | ❌ 历史文档 | v3.1 时间主轴化 | **不读** |
| `ARCHITECTURE_v4.md` | ❌ 历史文档 | v4.0 内存计算架构 | **不读** |
| `SIGMA4_SPEC_v1.md` | ❌ 已废弃 | v1.0 技术规范（含已知陷阱） | **不读** |
| **`SIGMA4_SPEC_v1_1.md`** | ✅ **唯一现行规范** | v1.1 AI 优化版 | **必须按顺序阅读** |
| **`DECISIONS.md`** | ✅ **覆盖性决策** | 对 SPEC 的正式偏离(ADR-001 nightly std::simd / ADR-002 跨平台矩阵) | **必读，与 SPEC 冲突时以此为准** |
| `AGENT.md` | ✅ **等效入口** | 与 CLAUDE.md 内容一致，任选其一 | **不要重复阅读** |
| **`CLAUDE.md`** | ✅ **本文档** | AI 入口 + 开发顺序 | **当前正在阅读** |
| `global_capital_players_full_index.csv` | ⚠️ 示例数据 | 183 实体示例数据集 | **Step 6 后加载测试** |

---

## 3. 开发顺序（严格遵守）

**⚠️ 这不是建议，是强制顺序。跳过任何一步会导致后续步骤无法验证。**

### Step 1: 内存布局与数据结构（约 1-2 小时）

**目标**：定义 `Entity`、`DeltaEvent`、`Relation` 的 `repr(C)` 布局，验证内存对齐和大小。

**阅读章节**：
- [SIGMA4_SPEC_v1_1.md §3 数据实体规范](SIGMA4_SPEC_v1_1.md#3-数据实体规范)
- [SIGMA4_SPEC_v1_1.md §3.2 并发模型](SIGMA4_SPEC_v1_1.md#32-并发模型极其重要)
- [SIGMA4_SPEC_v1_1.md §3.3 时间戳的双向语义](SIGMA4_SPEC_v1_1.md#33-时间戳的双向语义极其重要)

**关键产出**：
```rust
// 文件: src/entity.rs
// 必须包含:
// - Entity (repr(C, align(64)))
// - DeltaEvent (repr(C, packed))
// - Relation (repr(C))
// - SteadyState (repr(C))
// - 所有常量: MAX_SLICES, K_MAX, MAX_ENTITIES, DELTA_RING_CAPACITY
// - 对齐断言: assert_eq!(align_of::<Entity>(), 64)
// - 大小断言: assert!(size_of::<Entity>() <= 64 * 1024)
```

**禁止**：
- 不要定义 `String` 或 `Vec` 字段在 `Entity` 中（非热路径的 `name_ptr` 除外）
- 不要改变字段顺序（`repr(C)` 依赖声明顺序）
- 不要省略 `_pad` 字段（它们是对齐必需的）

---

### Step 2: 单纯形投影与编解码器（约 2-3 小时）

**目标**：实现 Duchi 投影算法和 SimplexCodec（无损可逆压缩）。

**阅读章节**：
- [SIGMA4_SPEC_v1_1.md §2.1 标准单纯形](SIGMA4_SPEC_v1_1.md#21-标准单纯形)
- [SIGMA4_SPEC_v1_1.md §2.2 退化单纯形的处理](SIGMA4_SPEC_v1_1.md#22-退化单纯形的处理)
- [SIGMA4_SPEC_v1_1.md §7.1 单纯形约束压缩](SIGMA4_SPEC_v1_1.md#71-单纯形约束压缩无损可逆)

**关键产出**：
```rust
// 文件: src/simplex.rs
// 必须包含:
// - pub fn project_onto_simplex(v: &[f32], k: usize) -> Vec<f32>
// - pub struct SimplexCodec;
// -   impl SimplexCodec { encode(), decode() }
// - 单元测试: 投影后行和 = 1, 所有元素 ≥ 0
// - 单元测试: 编解码往返，误差 < 1e-6
// - 单元测试: 退化维度（含 0 权重）编码正确
```

**禁止**：
- 不要假设输入已归一化（Duchi 的输入可以是任意实数）
- 不要移除权重为 0 的维度（保持 K_max 维，仅置 0）
- 不要忽略 padding 列的显式置 0（影响 Frobenius 距离）

---

### Step 3: 增量环与状态查询（约 2-3 小时）

**目标**：实现 DeltaEvent 的环形缓冲区写入和 `query_state` 查询（含增量回放 + 单纯形投影）。

**阅读章节**：
- [SIGMA4_SPEC_v1_1.md §3.2 并发模型](SIGMA4_SPEC_v1_1.md#32-并发模型极其重要)
- [SIGMA4_SPEC_v1_1.md §6.1 状态查询](SIGMA4_SPEC_v1_1.md#61-状态查询含增量回放)

**关键产出**：
```rust
// 文件: src/entity.rs (在 Entity impl 中)
// 必须包含:
// - pub fn apply_delta(&mut self, delta: DeltaEvent)  // 单线程版本
// - pub fn query_state(&self, query_time_us: u64) -> EntitySnapshot
// - 溢出检测: ring_head - ring_tail >= 1024 时 panic
// - 单元测试: 基态查询（无 delta）< 500ns
// - 单元测试: 100 delta 回放查询 < 2μs
// - 单元测试: 环溢出时正确 panic
```

**禁止**：
- 不要先实现多线程版本（默认单线程 SPSC，需要时再用 crossbeam-channel）
- 不要在查询函数内分配内存（`EntitySnapshot` 必须是栈分配或预分配）
- 不要忘记回放后投影到单纯形

---

### Step 4: 稀疏矩阵与 SpMV（约 3-4 小时）

**目标**：构建 CSR 格式的 CascadeMatrix，实现 SIMD 加速的稀疏矩阵-向量乘法（含硬件检测和标量 fallback）。

**阅读章节**：
- [SIGMA4_SPEC_v1_1.md §6.2 CSR SpMV（SIMD）](SIGMA4_SPEC_v1_1.md#62-csr-spmvsimd)

**关键产出**：
```rust
// 文件: src/matrix.rs
// 必须包含:
// - pub struct CascadeMatrix { n, row_ptr, col_idx, values, time_lag_us }
// - pub fn spmv_csr(x: &[f32], matrix: &CascadeMatrix, y: &mut [f32])
// - 运行时 CPU 检测: is_x86_feature_detected!("avx512f")
// - AVX-512 实现（unsafe）
// - AVX2 实现（unsafe）
// - 标量 fallback（safe）
// - 单元测试: 与稠密计算逐元素对比，误差 < 1e-5
```

**禁止**：
- 不要直接写 `std::simd`（不稳定，编译器支持不一致）
- 不要假设目标硬件支持 AVX-512（必须提供标量 fallback）
- 不要在热路径中使用 `gather` 的模拟实现（性能会暴跌 10 倍）

---

### Step 5: 级联推理引擎（约 3-4 小时）

**目标**：实现完整的级联推理，含脆性阈值特殊处理、衰减、置信度剪枝。

**阅读章节**：
- [SIGMA4_SPEC_v1_1.md §6.3 级联推理（含脆性阈值）](SIGMA4_SPEC_v1_1.md#63-级联推理含脆性阈值)
- [SIGMA4_SPEC_v1_1.md §2.5 帕累托约束投影](SIGMA4_SPEC_v1_1.md#25-帕累托约束投影)

**关键产出**：
```rust
// 文件: src/cascade.rs
// 必须包含:
// - pub struct EntityStateView { coordinates, brittle_threshold, decay_coefficient, time_lag_us }
// - pub fn cascade(initial, matrix, entity_states, max_hops, theta) -> Vec<CascadeResult>
// - 脆性实体特殊处理: brittle > 0.5 时，未突破阈值快速衰减，突破后不衰减
// - 单元测试: 星型拓扑，中心冲击，叶子收到正确衰减信号
// - 单元测试: 脆性实体阈值突破时置信度 = 1.0
// - 性能基准: 100 实体，5 跳，< 100μs（AVX-512）
```

**禁止**：
- 不要对所有实体统一使用指数衰减（脆性实体有特殊规则）
- 不要忽略 `time_lag_us` 的累积（级联结果必须包含 lag）
- 不要先写这一步再补前面（依赖 Step 2-4 的全部产出）

---

### Step 6: 网络层与协议（约 2-3 小时）

**目标**：实现 TCP + TLS 1.3 + 自定义二进制帧协议的服务端。

**阅读章节**：
- [SIGMA4_SPEC_v1_1.md §4.2 传输层协议](SIGMA4_SPEC_v1_1.md#42-传输层协议极其重要)
- [SIGMA4_SPEC_v1_1.md §5 二进制通信协议](SIGMA4_SPEC_v1_1.md#5-二进制通信协议)

**关键产出**：
```rust
// 文件: src/server.rs
// 必须包含:
// - Length-Prefix Framing: [4 bytes length] + [payload]
// - TLS 1.3 配置，ALPN = "sigma4"
// - Frame::decode / Frame::encode
// - 处理: StateUpdate, StateQuery, CascadeRun, SnapshotReq, Heartbeat
// - 最大帧大小限制: assert!(len <= 65536)
// - 单元测试: 帧编解码往返
// - 单元测试: 超大帧拒绝
```

**禁止**：
- 不要走 HTTP/2 标准帧（中间件会拒绝自定义类型）
- 不要走 gRPC（字段标签开销太大）
- 不要走 WebSocket（需要 HTTP 升级握手，增加延迟）
- 不要先写这一步（必须在前 5 步全部完成后）

---

## 4. 验收测试清单（每步必须执行）

### Step 1 验收
```bash
cargo test entity_layout
# 必须全部通过:
# - Entity 对齐 = 64
# - Entity 大小 ≤ 65536
# - DeltaEvent 大小 = 16
# - Relation 大小计算正确
```

### Step 2 验收
```bash
cargo test simplex
# 必须全部通过:
# - 投影后行和 = 1（容差 1e-5）
# - 投影后所有元素 ≥ 0
# - 编解码往返误差 < 1e-6
# - 退化维度（含 0）编码正确
# - Frobenius 距离忽略 padding（不同 padding 值距离 = 0）
```

### Step 3 验收
```bash
cargo test state_query
# 必须全部通过:
# - 无 delta 查询 < 500ns（用 criterion 或自定义计时）
# - 100 delta 回放 < 2μs
# - 环溢出时 panic
# - 查询时间早于所有 delta 时返回基态
# - 查询时间晚于所有 delta 时返回基态 + 全部 delta
```

### Step 4 验收
```bash
cargo test spmv
# 必须全部通过:
# - 与稠密计算逐元素对比，误差 < 1e-5
# - 标量 fallback 和 SIMD 结果一致
# - 空行（无非零元）结果为 0
# - 单元素行结果正确
```

### Step 5 验收
```bash
cargo test cascade
# 必须全部通过:
# - 星型拓扑叶子收到正确衰减信号
# - 脆性实体阈值突破时置信度 = 1.0
# - 脆性实体未突破时快速衰减
# - 性能: 100 实体 5 跳 < 100μs（release 模式）
```

### Step 6 验收
```bash
cargo test protocol
# 必须全部通过:
# - 帧编解码往返
# - 超大帧拒绝（> 65536）
# - TLS 握手成功（ALPN = "sigma4"）
# - StateUpdate 端到端（客户端写入 → 服务端解析 → 应用 delta）
# - CascadeRun 端到端（客户端请求 → 服务端执行 → 返回结果）
```

---

## 5. 常见错误速查（AI 自检）

如果你在实现中遇到以下情况，说明你可能读错了文档或跳过了步骤：

| 症状 | 原因 | 修复 |
|------|------|------|
| 编译时 Entity 大小 > 65536 | 字段顺序不对或缺少 padding | 回读 §3.1，严格复制布局 |
| 单纯形投影后行和 ≠ 1 | 未处理 NaN 或浮点误差 | 回读 §2.1，检查输入不含 NaN |
| 编解码后 padding 列非 0 | decode() 未显式置 0 | 回读 §7.1，添加 `for i in k..K_max { M[s][i] = 0.0 }` |
| 多线程下 delta 环数据损坏 | 违反了 SPSC 假设 | 回读 §3.2，改为单线程或 MPSC 队列 |
| 级联结果与手工计算不符 | 脆性阈值未处理 | 回读 §6.3，添加 brittle 分支 |
| HTTP 中间件拒绝连接 | 走了 HTTP/2 标准帧 | 回读 §4.2，改为 TCP + TLS + 自定义协议 |
| 状态查询延迟 > 10μs | 函数内部分配了内存 | 回读 §6.1，确保 `coords` 是栈拷贝 |

---

## 6. 版本锁定

本文档锁定以下文件版本：
- `AGENT.md` / `CLAUDE.md` — v1.1（当前）
- `SIGMA4_SPEC_v1_1.md` — v1.1（2024-07-10）

**如果未来出现 `SIGMA4_SPEC_v1_2.md` 或更高版本，本文档会自动失效。新版本发布时，应检查本段是否已更新。**

---

*本文档为自包含 AI 开发指南。不引用任何外部上下文。所有开发决策已在本文档和 `SIGMA4_SPEC_v1_1.md` 中明确声明，AI 不应向用户询问技术细节。*
