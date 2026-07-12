# Σ⁴-Engine 技术开发规范 v1.1（Agent 优化版）

> **版本**：v1.1  
> **目标读者**：AI Agent（自动代码生成）  
> **设计目标**：消除歧义、显式声明所有隐含假设、标记常见陷阱、提供决策树  
> **约束**：本文档不依赖任何外部上下文；Agent 应能在零先验知识下独立实现  

---

## 目录

1. [Agent 开发指南（必读）](#1-agent-开发指南必读)
2. [核心数学模型](#2-核心数学模型)
3. [数据实体规范](#3-数据实体规范)
4. [系统架构](#4-系统架构)
5. [二进制通信协议](#5-二进制通信协议)
6. [算法规范](#6-算法规范)
7. [存储层规范](#7-存储层规范)
8. [切面扩展协议](#8-切面扩展协议)
9. [测试与验证](#9-测试与验证)
10. [部署与运维](#10-部署与运维)
11. [附录 A：Agent 陷阱清单](#附录-a-agent-陷阱清单)
12. [附录 B：决策树](#附录-b-决策树)
13. [附录 C：最小可运行 Rust 骨架](#附录-c-最小可运行-rust-骨架)

---

## 1. Agent 开发指南（必读）

### 1.1 本文档的阅读协议

**⚠️ Agent 警告**：本文档中所有标记为 `AGENT_TRAP:` 的块，是其他 AI Agent 在实现过程中反复踩坑的位置。你必须在编码前仔细阅读这些块，并在代码中显式处理对应的边界条件。

**⚠️ Agent 警告**：本文档中所有标记为 `AGENT_MUST:` 的指令是强制性的，不得省略、简化或用"行业标准"替代。

### 1.2 系统定位（关键理解）

Σ⁴-Engine 不是：
- ❌ 知识图谱（不存在图遍历语义）
- ❌ 数据库（不存在磁盘查询热路径）
- ❌ 机器学习系统（不存在训练/推理分离，不存在概率模型）
- ❌ 微服务架构（不存在服务间 RPC 调用）

Σ⁴-Engine 是：
- ✅ **内存常驻的确定性离散事件仿真内核**
- ✅ 所有计算在固定预分配内存上完成
- ✅ 所有延迟目标以微秒为单位

### 1.3 开发顺序（不要跳过）

```
Step 1: 数据结构（Entity, DeltaEvent, Relation）← 先做内存布局
Step 2: 单纯形投影 + 编解码器 ← 先做数学核心，再做上层
Step 3: 增量环 + 状态查询 ← 先做查询路径，再做更新路径
Step 4: 稀疏矩阵 + SpMV ← 先做离线矩阵构建，再做在线推理
Step 5: 级联推理 ← 最后做，依赖前面全部
Step 6: 网络层 ← 最后做，因为协议依赖前面全部
```

**AGENT_MUST: 不要先写 HTTP 服务再填业务逻辑。必须先验证 Step 1-3 的内存布局和数学正确性，再叠加网络层。**

---

## 2. 核心数学模型

### 2.1 标准单纯形

**定义 2.1**：k-端点标准单纯形 Δᵏ⁻¹ ⊂ ℝᵏ 定义为：

> **Δᵏ⁻¹ = {x ∈ ℝᵏ : Σᵢ₌₁ᵏ xᵢ = 1, xᵢ ≥ 0}**

**AGENT_MUST: Duchi 投影的输入向量 v 可以是任意实数（含负数、含大于1的数）。不要假设输入已归一化。**

```rust
/// Duchi 单纯形投影算法
/// 输入: v ∈ ℝᵏ（任意实向量，不要求和为1，不要求非负）
/// 输出: x ∈ Δᵏ⁻¹（欧几里得最近点）
/// 复杂度: O(k log k)，由排序主导
/// 
/// AGENT_TRAP: 不要试图优化掉排序。虽然存在 O(k) 的选择算法，
/// 但排序实现更简单、更稳定，且 k ≤ 8 时性能差异可忽略。
pub fn project_onto_simplex(v: &[f32], k: usize) -> Vec<f32> {
    assert!(k > 0, "k must be positive");
    assert!(v.len() >= k, "input vector too short");
    
    // 步骤1: 复制并降序排序
    let mut u = v[..k].to_vec();
    u.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    
    // AGENT_TRAP: NaN 处理。如果输入含 NaN，partial_cmp 返回 None，
    // unwrap 会 panic。生产代码应使用 unwrap_or 或提前过滤 NaN。
    // 本规范要求调用方保证输入不含 NaN。
    
    // 步骤2: 寻找 rho
    let mut cssv = 0.0f32;
    let mut rho = 0usize;
    
    for (i, &ui) in u.iter().enumerate() {
        cssv += ui;
        let threshold = (cssv - 1.0) / (i + 1) as f32;
        if ui > threshold {
            rho = i;
        }
    }
    
    // 步骤3: 计算 theta
    let theta = (u[..=rho].iter().sum::<f32>() - 1.0) / (rho + 1) as f32;
    
    // 步骤4: 投影
    v[..k].iter()
        .map(|&vi| (vi - theta).max(0.0))
        .collect()
}
```

**AGENT_TRAP: 投影后必须验证 Σxᵢ = 1 且 xᵢ ≥ 0。浮点误差可能导致和为 0.9999999 或 1.0000001。在单元测试中验证容差 < 1e-5。**

### 2.2 退化单纯形的处理

**AGENT_TRAP: 当某个端点的权重趋近于 0 时，不要移除该维度。保持维度不变，仅将权重设为 0。否则后续 delta 更新可能导致维度不一致。**

正确行为：
```rust
// 输入: [0.8, 0.2, 0.0, 0.0]
// 输出: [0.8, 0.2, 0.0, 0.0]（保持4维）
// 不要输出: [0.8, 0.2]（错误！降维了）
```

### 2.3 实体状态张量

**定义 2.2**：设系统有 S 个切面，第 s 个切面有 Kₛ 个端点，K_max = maxₛ Kₛ。实体 ξ 的状态张量为：

> **M_ξ ∈ ℝ^{S×K_max}**
>
> 其中第 s 行前 Kₛ 个元素满足 Σᵢ₌₁^{Kₛ} M_ξ[s,i] = 1，M_ξ[s,i] ≥ 0
> 第 s 行第 Kₛ 到 K_max−1 列填充 0

**AGENT_TRAP: padding 列（Kₛ 到 K_max−1）必须显式置 0，不能保留未初始化内存。这影响 Frobenius 距离计算。**

### 2.4 Frobenius 距离

```rust
/// Frobenius 距离（含 padding 处理）
pub fn frobenius_distance(
    a: &[[f32; K_MAX]; MAX_SLICES],
    b: &[[f32; K_MAX]; MAX_SLICES],
    slice_dims: &[u8],
) -> f32 {
    let mut sum = 0.0f32;
    for (s, &k_s) in slice_dims.iter().enumerate() {
        let k = k_s as usize;
        for i in 0..k {
            let diff = a[s][i] - b[s][i];
            sum += diff * diff;
        }
        // AGENT_TRAP: 不要对 padding 列（i ≥ k）求和。
        // 它们都是 0，但显式跳过更清晰。
    }
    sum.sqrt()
}
```

### 2.5 帕累托约束投影

**AGENT_TRAP: 约束冲突时的投影顺序影响最终结果。必须按以下固定顺序执行：**

1. 先应用所有 `lower_bound` 约束（强制下限）
2. 再应用所有 `upper_bound` 约束（强制上限）
3. 最后应用所有 `linear` 约束（线性组合）
4. 约束应用后，对每行重新执行 Duchi 投影

```rust
pub fn pareto_project(
    raw: &[[f32; K_MAX]; MAX_SLICES],
    constraints: &[Constraint],
    slice_dims: &[u8],
) -> [[f32; K_MAX]; MAX_SLICES] {
    let mut result = *raw;
    
    // AGENT_MUST: 约束应用顺序固定：lower → upper → linear
    for c in constraints.iter().filter(|c| c.kind == ConstraintKind::LowerBound) {
        apply_lower_bound(&mut result[c.slice], c.endpoint, c.value);
    }
    
    for c in constraints.iter().filter(|c| c.kind == ConstraintKind::UpperBound) {
        apply_upper_bound(&mut result[c.slice], c.endpoint, c.value);
    }
    
    for c in constraints.iter().filter(|c| c.kind == ConstraintKind::Linear) {
        apply_linear(&mut result[c.slice], &c.coefficients, c.target);
    }
    
    // 约束应用后，行和可能 ≠ 1，必须重新投影
    for s in 0..slice_dims.len() {
        let k = slice_dims[s] as usize;
        let projected = project_onto_simplex(&result[s], k);
        result[s][..k].copy_from_slice(&projected);
    }
    
    result
}
```

**AGENT_TRAP: 约束应用后必须重新投影到单纯形。不要假设约束应用后行和自然为 1。**

---

## 3. 数据实体规范

### 3.1 统一实体模型

```rust
// AGENT_MUST: 所有常量必须在编译期确定，不得运行时计算。
pub const MAX_SLICES: usize = 16;
pub const K_MAX: usize = 8;
pub const MAX_ENTITIES: usize = 65536;
pub const DELTA_RING_CAPACITY: usize = 1024;

// AGENT_MUST: 内存布局必须与硬件缓存行对齐（64 bytes）。
// 这影响 SIMD 加载性能和伪共享。
#[repr(C, align(64))]
pub struct Entity {
    // === 身份 ===
    pub id: u32,
    pub entity_type: u8,
    pub flags: u8,                  // bit 0: is_boundary_breaking
                                    // bit 1: is_constrained
                                    // bits 2-7: reserved
    pub num_slices: u8,
    pub _pad0: u8,
    
    // === 公共时间轴（秒级 Unix 时间戳）===
    // AGENT_TRAP: 时间戳是 i64（有符号），不是 u64。
    // 原因: Unix 时间戳在 2038 年前可能为负（历史时间）。
    pub valid_from: i64,
    pub valid_until: i64,           // i64::MAX = 当前有效
    
    // === 切面坐标 ===
    // AGENT_TRAP: 这是 [行: 切面][列: 端点] 的二维数组。
    // 不是 [端点][切面]。切面对应行，端点对应列。
    pub coordinates: [[f32; K_MAX]; MAX_SLICES],
    pub slice_dims: [u8; MAX_SLICES],
    
    // === 增量环（固定大小环形缓冲区）===
    // AGENT_TRAP: 这是并发热点。下文详细说明并发模型。
    pub delta_ring: [DeltaEvent; DELTA_RING_CAPACITY],
    pub ring_head: u32,
    pub ring_tail: u32,
    
    // === 稳态属性 ===
    pub steady_state: SteadyState,
    
    // === 元数据（非热路径，可选加载）===
    pub name_ptr: *const u8,
    pub name_len: u16,
    pub _pad1: [u8; 6],
}

#[repr(C, packed)]
pub struct DeltaEvent {
    pub timestamp_us: u64,          // 微秒级 Unix 时间戳
    pub slice_mask: u16,            // 哪些切面被修改（位掩码）
    pub endpoint_idx: u8,
    pub delta_value: f32,           // 变更量（相对值，可正可负）
    pub _pad: u8,
}

#[repr(C)]
pub struct SteadyState {
    pub geography: u16,
    pub industry: u16,
    pub ownership_type: u8,
    pub _pad: [u8; 5],
}
```

**AGENT_MUST: `Entity` 总大小必须 ≤ 64KB（L1 缓存容量的一半），以支持缓存常驻。**

### 3.2 并发模型（极其重要）

**AGENT_TRAP: 本文档 v1.0 未明确声明并发模型，导致多个实现出现数据竞争。v1.1 显式声明如下：**

```
并发模型: 单生产者单消费者（SPSC）

生产者: Ingestion Gateway（单线程）
  - 从网络接收帧
  - 解析为 DeltaEvent
  - 写入 Entity.delta_ring[ring_head % 1024]
  - ring_head.fetch_add(1, Ordering::Release)

消费者: Query Engine（单线程，与生产者同线程或不同线程）
  - 读取 Entity.delta_ring[ring_tail % 1024]
  - 若 timestamp_us ≤ query_time: 应用 delta，ring_tail += 1
  - 否则: 停止回放

AGENT_MUST: 如果生产者和消费者在不同线程，必须使用 Release/Acquire 内存序。
AGENT_MUST: 如果是单线程（生产者直接调用查询），无需原子操作，用普通读写即可。
```

**AGENT_TRAP: 不要试图优化为多生产者。当前架构明确为单生产者。如果未来需要多生产者，应改用 MPSC 队列（crossbeam-channel）而非无锁环。**

```rust
// 单线程版本（推荐，最简单）
impl Entity {
    pub fn apply_delta_singlethreaded(&mut self, delta: DeltaEvent) {
        let idx = (self.ring_head as usize) % DELTA_RING_CAPACITY;
        self.delta_ring[idx] = delta;
        self.ring_head += 1;
        
        // AGENT_TRAP: 不要覆盖未消费的 delta。
        // 如果 ring_head - ring_tail >= 1024，说明环已满。
        // 此时必须触发快照刷盘，然后重置 ring_tail = ring_head。
        if self.ring_head - self.ring_tail >= DELTA_RING_CAPACITY as u32 {
            panic!("Delta ring overflow. Entity {} needs snapshot.", self.id);
        }
    }
}

// 多线程版本（仅在确认需要时使用）
use std::sync::atomic::{AtomicU32, Ordering};

pub struct ThreadSafeEntity {
    // ... 同上，但 ring_head 和 ring_tail 改为 AtomicU32
    pub ring_head: AtomicU32,
    pub ring_tail: AtomicU32,
}

impl ThreadSafeEntity {
    pub fn apply_delta(&self, delta: DeltaEvent) {
        let head = self.ring_head.load(Ordering::Relaxed);
        let idx = (head as usize) % DELTA_RING_CAPACITY;
        
        // AGENT_TRAP: 直接写入环槽位。这是安全的，因为每个槽位
        // 只被 head 指针独占（SPSC 保证）。
        unsafe {
            let ring_ptr = self.delta_ring.as_ptr() as *mut DeltaEvent;
            ring_ptr.add(idx).write(delta);
        }
        
        self.ring_head.store(head + 1, Ordering::Release);
    }
}
```

### 3.3 时间戳的双向语义（极其重要）

**AGENT_TRAP: v1.0 未区分 `valid_from` 和 `valid_until` 的兜底方向，导致多个实现出错。**

```rust
pub struct TimestampNormalizer;

impl TimestampNormalizer {
    /// valid_from 的兜底: 缺失字段用最小值
    /// "1969年" → 1969-01-01 00:00:00 UTC（最早可能时间）
    pub fn normalize_from(year: i32, month: Option<u8>, day: Option<u8>) -> i64 {
        let m = month.unwrap_or(1);
        let d = day.unwrap_or(1);
        let dt = Utc.ymd(year, m, d).and_hms(0, 0, 0);
        dt.timestamp()
    }
    
    /// valid_until 的兜底: 缺失字段用最大值
    /// "1969年" → 1969-12-31 23:59:59 UTC（最晚可能时间）
    /// AGENT_TRAP: 这是与 valid_from 的关键区别！
    pub fn normalize_until(year: i32, month: Option<u8>, day: Option<u8>) -> i64 {
        let m = month.unwrap_or(12);
        let d = day.unwrap_or(
            if month.is_none() { 31 } else { days_in_month(year, m) }
        );
        let dt = Utc.ymd(year, m, d).and_hms(23, 59, 59);
        dt.timestamp()
    }
}
```

### 3.4 关系模型

```rust
#[repr(C)]
pub struct Relation {
    pub from_id: u32,
    pub to_id: u32,
    pub relation_type: u8,
    pub weight: f32,
    pub time_lag_us: u32,           // AGENT_TRAP: 微秒级，不是毫秒级
    pub valid_from: i64,
    pub valid_until: i64,
}
```

---

## 4. 系统架构

### 4.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Data Ingestion                                │
│  硬件传感器/行情网关/NLP管道/心跳探针 → 二进制帧                      │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Ingestion Gateway (单线程)                      │
│  AGENT_MUST: 单线程。不要加线程池、不要加 tokio::spawn。             │
│  原因: Delta Ring 是 SPSC 模型，多线程会导致数据竞争。               │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼ 内存直写
┌─────────────────────────────────────────────────────────────────────┐
│                      Core Engine (Rust, pinned)                      │
│                                                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │ Entity Pool     │  │ Slice Registry  │  │ Cascade Matrix  │     │
│  │ (预分配数组)     │  │ (YAML 热加载)    │  │ (CSR 稀疏)       │     │
│  │ [Entity; 65536] │  │ 切面定义 + 端点  │  │ 预计算 + SIMD    │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Compute Kernel                                                 │  │
│  │ - SpMV (稀疏矩阵-向量乘法)                                     │  │
│  │ - Simplex Projection (SIMD 并行)                               │  │
│  │ - Pareto Solve (约束检查)                                      │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Snapshot Logger (独立线程，每 100ms 批量刷盘)                   │  │
│  │ AGENT_MUST: 不要与 Compute Kernel 共享线程。                   │  │
│  │ 原因: 刷盘是 IO 密集型，会阻塞计算。                           │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼ TCP + TLS 1.3
┌─────────────────────────────────────────────────────────────────────┐
│                      Query API                                       │
│  /state       → 当前状态 (~48 bytes)                                 │
│  /cascade     → 级联结果 (~800 bytes)                                │
│  /snapshot    → 全量快照 (~N × 48 bytes)                             │
│  /stream      → WebSocket 实时 delta 流                              │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 传输层协议（极其重要）

**AGENT_TRAP: v1.0 说 "HTTP/2 自定义二进制帧"，这导致多个实现试图在 HTTP/2 帧格式上叠加自定义类型。这是错误的。**

**正确设计：**

```
传输层: TCP + TLS 1.3 + 自定义应用层协议

不要走 HTTP/2 标准帧。
原因: 
1. 自定义帧类型（0x01-0x07）会被标准 HTTP/2 中间件拒绝
2. HTTP/2 的 HPACK 头部压缩对二进制数据是 overhead
3. 我们需要的是原始 TCP 流 + 自己的帧边界协议

帧边界方式: Length-Prefixed Framing
  每个帧: [4 bytes: length] + [length bytes: payload]
  这避免了粘包问题，比 HTTP/2 的 CONTINUATION 更简单

TLS ALPN: 协商 "sigma4"（自定义协议标识符）
  如果客户端不支持，拒绝连接（不要降级到 HTTP/1.1）
```

```rust
// 服务端（tokio + rustls）
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

async fn serve(addr: &str) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(addr).await?;
    let tls_acceptor = load_tls_config()?;  // ALPN = ["sigma4"]
    
    loop {
        let (stream, _) = listener.accept().await?;
        let tls_stream = tls_acceptor.accept(stream).await?;
        
        // AGENT_MUST: 每个连接一个独立任务，但 Entity Pool 是共享的（只读）
        tokio::spawn(handle_connection(tls_stream));
    }
}

async fn handle_connection(mut stream: TlsStream<TcpStream>) {
    let mut buf = [0u8; 65536];
    
    loop {
        // 读取 4 字节长度前缀
        stream.read_exact(&mut buf[..4]).await.unwrap();
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        
        // AGENT_TRAP: 必须限制最大帧大小，防止内存耗尽攻击
        assert!(len <= 65536, "Frame too large: {}", len);
        
        stream.read_exact(&mut buf[..len]).await.unwrap();
        let frame = Frame::decode(&buf[..len]);
        
        let response = process_frame(frame);
        
        // 写回: 长度前缀 + payload
        let resp_len = response.len() as u32;
        stream.write_all(&resp_len.to_be_bytes()).await.unwrap();
        stream.write_all(&response).await.unwrap();
    }
}
```

---

## 5. 二进制通信协议

### 5.1 帧格式（修订版）

```
帧结构:
┌──────────────────────────────────────────────────────────────────┐
│ Length Prefix (4 bytes, big-endian, 不包括自身)                   │
├──────────────────────────────────────────────────────────────────┤
│ Payload                                                          │
│ ┌──────────┬──────────┬───────────────────────────────────────┐ │
│ │ Version  │ FrameType│ Frame Body (变长)                     │ │
│ │  1 byte  │  1 byte  │                                       │ │
│ └──────────┴──────────┴───────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘

AGENT_TRAP: v1.0 的 8 字节头部已废弃。
原因: Length Prefix 已经提供了帧边界，不需要额外的 Magic 和 Version 字段。
Version 和 FrameType 移至 Payload 内部。
```

### 5.2 帧类型定义（不变）

```rust
#[repr(u8)]
pub enum FrameType {
    StateUpdate     = 0x01,
    StateQuery      = 0x02,
    StateResponse   = 0x03,
    CascadeRun      = 0x04,
    CascadeResult   = 0x05,
    SnapshotReq     = 0x06,
    SnapshotResp    = 0x07,
    Heartbeat       = 0xFF,
}
```

### 5.3 STATE_UPDATE 帧体

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Version  | FrameType |                                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Entity ID                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Timestamp (μs)                           |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Slice Mask  | Endpoint  |          Delta Value                 |
|   (u16)     |  (u8)     |            (f32)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

总 Payload: 19 bytes
总帧大小: 4 + 19 = 23 bytes
```

---

## 6. 算法规范

### 6.1 状态查询（含增量回放）

```rust
/// 查询实体在时刻 T 的完整状态
/// AGENT_TRAP: 本函数是热路径，禁止在函数内部分配内存。
pub fn query_state(entity: &Entity, query_time_us: u64) -> EntitySnapshot {
    // AGENT_MUST: 基态必须深拷贝，不能返回引用。
    // 原因: 调用方可能修改返回的坐标，这会污染 Entity Pool。
    let mut coords = entity.coordinates;
    
    // 回放 delta ring
    let tail = entity.ring_tail as usize;
    let head = entity.ring_head as usize;
    
    for i in tail..head {
        let delta = &entity.delta_ring[i % DELTA_RING_CAPACITY];
        if delta.timestamp_us > query_time_us {
            break;
        }
        
        let slice = delta.slice_mask.trailing_zeros() as usize;
        let endpoint = delta.endpoint_idx as usize;
        coords[slice][endpoint] += delta.delta_value;
    }
    
    // 投影回单纯形
    for s in 0..entity.num_slices as usize {
        let k = entity.slice_dims[s] as usize;
        let projected = project_onto_simplex(&coords[s], k);
        coords[s][..k].copy_from_slice(&projected);
    }
    
    EntitySnapshot { coords, slice_dims: entity.slice_dims, num_slices: entity.num_slices }
}
```

### 6.2 CSR SpMV（SIMD）

**AGENT_TRAP: SIMD 实现必须包含硬件检测和 fallback。**

```rust
/// SpMV with runtime feature detection
pub fn spmv_csr(x: &[f32], matrix: &CascadeMatrix, y: &mut [f32]) {
    if is_x86_feature_detected!("avx512f") {
        spmv_csr_avx512(x, matrix, y);
    } else if is_x86_feature_detected!("avx2") {
        spmv_csr_avx2(x, matrix, y);
    } else {
        spmv_csr_scalar(x, matrix, y);  // AGENT_MUST: 必须有标量 fallback
    }
}

#[target_feature(enable = "avx512f")]
unsafe fn spmv_csr_avx512(x: &[f32], matrix: &CascadeMatrix, y: &mut [f32]) {
    use std::arch::x86_64::*;
    
    let n = matrix.n as usize;
    
    for i in 0..n {
        let start = matrix.row_ptr[i] as usize;
        let end = matrix.row_ptr[i + 1] as usize;
        
        let mut sum = _mm512_set1_ps(0.0);
        let mut j = start;
        
        while j + 16 <= end {
            let cols = _mm512_loadu_si512(matrix.col_idx.as_ptr().add(j) as *const _);
            let vals = _mm512_loadu_ps(matrix.values.as_ptr().add(j));
            
            // gather: x[col_idx[j..j+16]]
            let gathered = _mm512_i32gather_ps(cols, x.as_ptr() as *const i8, 4);
            sum = _mm512_fmadd_ps(vals, gathered, sum);
            
            j += 16;
        }
        
        let mut scalar_sum = _mm512_reduce_add_ps(sum);
        
        // 标量收尾
        for k in j..end {
            scalar_sum += matrix.values[k] * x[matrix.col_idx[k] as usize];
        }
        
        y[i] = scalar_sum;
    }
}

/// 标量 fallback（所有平台通用）
fn spmv_csr_scalar(x: &[f32], matrix: &CascadeMatrix, y: &mut [f32]) {
    let n = matrix.n as usize;
    
    for i in 0..n {
        let start = matrix.row_ptr[i] as usize;
        let end = matrix.row_ptr[i + 1] as usize;
        
        let mut sum = 0.0f32;
        for k in start..end {
            sum += matrix.values[k] * x[matrix.col_idx[k] as usize];
        }
        
        y[i] = sum;
    }
}
```

### 6.3 级联推理（含脆性阈值）

**AGENT_TRAP: v1.0 未包含脆性实体的特殊处理逻辑。**

```rust
pub fn cascade(
    initial_vector: &[f32],
    matrix: &CascadeMatrix,
    entity_states: &[EntityStateView],  // 包含 cascade 坐标和 brittle_threshold
    max_hops: u8,
    theta: f32,
) -> Vec<CascadeResult> {
    let n = matrix.n as usize;
    let mut current = initial_vector.to_vec();
    let mut next = vec![0.0f32; n];
    let mut results = Vec::new();
    
    let mut hop_count = vec![u8::MAX; n];
    let mut min_lag = vec![u32::MAX; n];
    
    for hop in 0..max_hops {
        spmv_csr(&current, matrix, &mut next);
        
        for i in 0..n {
            let state = &entity_states[i];
            let brittle_coord = state.coordinates[cascade_slice][brittle_endpoint];
            
            // 脆性实体特殊处理
            let next_conf = if brittle_coord > 0.5 {
                // 脆性实体: 未达阈值时快速衰减，突破后全置信度传导
                let impact = next[i];
                if impact >= state.brittle_threshold {
                    impact  // 阈值突破，不衰减
                } else {
                    impact * 0.5f32.powi(hop as i32)  // 快速衰减
                }
            } else {
                next[i] * state.decay_coefficient
            };
            
            if next_conf >= theta {
                if hop_count[i] == u8::MAX {
                    hop_count[i] = hop;
                    min_lag[i] = state.time_lag_us;
                }
                next[i] = next_conf;
            } else {
                next[i] = 0.0;
            }
        }
        
        std::mem::swap(&mut current, &mut next);
    }
    
    for i in 0..n {
        if current[i] >= theta && hop_count[i] != u8::MAX {
            results.push(CascadeResult {
                entity_id: i as u32,
                confidence: current[i],
                hop: hop_count[i],
                lag_us: min_lag[i],
            });
        }
    }
    
    results
}
```

---

## 7. 存储层规范

### 7.1 单纯形约束压缩（无损可逆）

```rust
pub struct SimplexCodec;

impl SimplexCodec {
    /// 编码: S × K_max 矩阵 → 紧凑字节流
    /// AGENT_TRAP: 输入矩阵每行前 K_s 个元素必须和为 1（容差 1e-5）。
    /// 如果不满足，编码结果是未定义的。
    pub fn encode(M: &[[f32; K_MAX]; MAX_SLICES], slice_dims: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        for (s, &k_s) in slice_dims.iter().enumerate() {
            let k = k_s as usize;
            let row_sum: f32 = M[s][..k].iter().sum();
            assert!((row_sum - 1.0).abs() < 1e-5, 
                "Row {} sum = {}, expected 1.0", s, row_sum);
            
            for i in 0..k - 1 {
                buf.extend_from_slice(&M[s][i].to_le_bytes());
            }
        }
        buf
    }
    
    /// 解码: 字节流 → 完整矩阵（100% 可逆）
    /// AGENT_TRAP: 解码后的 padding 列（i ≥ K_s）必须显式置 0。
    pub fn decode(data: &[u8], slice_dims: &[u8], K_max: usize) 
        -> [[f32; K_MAX]; MAX_SLICES] 
    {
        let mut M = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut offset = 0;
        
        for (s, &k_s) in slice_dims.iter().enumerate() {
            let k = k_s as usize;
            
            for i in 0..k - 1 {
                M[s][i] = f32::from_le_bytes([
                    data[offset], data[offset + 1], 
                    data[offset + 2], data[offset + 3]
                ]);
                offset += 4;
            }
            
            M[s][k - 1] = 1.0 - M[s][..k - 1].iter().sum::<f32>();
            
            // AGENT_MUST: 显式置 0 padding
            for i in k..K_max {
                M[s][i] = 0.0;
            }
        }
        
        M
    }
}
```

---

## 8. 切面扩展协议

### 8.1 运行时热加载

```rust
impl Engine {
    /// 热加载新切面
    /// AGENT_TRAP: 这不是高频操作。调用频率应 < 1次/天。
    /// 如果高频调用，应考虑预分配更多槽位。
    pub fn add_slice(&mut self, slice_def: SliceDef) -> Result<(), EngineError> {
        let slice_idx = self.registry.slices.len();
        if slice_idx >= MAX_SLICES {
            return Err(EngineError::SliceLimitExceeded);
        }
        
        self.registry.slices.push(slice_def.clone());
        
        // AGENT_MUST: 并行初始化所有实体的新切面坐标
        // 使用 Rayon 或 crossbeam 的 scoped threads
        self.entity_pool.par_iter_mut().for_each(|entity| {
            entity.slice_dims[slice_idx] = slice_def.num_endpoints;
            entity.num_slices += 1;
            
            let k = slice_def.num_endpoints as usize;
            let uniform = 1.0 / k as f32;
            for i in 0..k {
                entity.coordinates[slice_idx][i] = uniform;
            }
            
            // AGENT_MUST: 新切面的 padding 列必须置 0
            for i in k..K_MAX {
                entity.coordinates[slice_idx][i] = 0.0;
            }
        });
        
        // 后台异步重建级联矩阵
        self.rebuild_matrix_async();
        
        Ok(())
    }
}
```

---

## 9. 测试与验证

### 9.1 单元测试矩阵

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simplex_projection_basic() {
        let v = vec![2.0, 1.0, -0.5, 0.0];
        let x = project_onto_simplex(&v, 4);
        
        // AGENT_TRAP: 不要只检查和为1。必须验证每个 xᵢ ≥ 0。
        assert!((x.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert!(x.iter().all(|&xi| xi >= -1e-6));
    }
    
    #[test]
    fn test_simplex_codec_roundtrip() {
        let mut M = [[0.0f32; K_MAX]; MAX_SLICES];
        M[0] = [0.1, 0.5, 0.2, 0.2, 0.0, 0.0, 0.0, 0.0];
        M[1] = [0.3, 0.3, 0.3, 0.1, 0.0, 0.0, 0.0, 0.0];
        
        let dims = [4u8, 4u8];
        let encoded = SimplexCodec::encode(&M, &dims);
        let decoded = SimplexCodec::decode(&encoded, &dims, K_MAX);
        
        for s in 0..2 {
            for i in 0..K_MAX {
                assert!((M[s][i] - decoded[s][i]).abs() < 1e-6,
                    "Mismatch at [{}][{}]: {} vs {}", s, i, M[s][i], decoded[s][i]);
            }
        }
    }
    
    #[test]
    fn test_delta_ring_overflow() {
        let mut entity = create_test_entity();
        
        for i in 0..DELTA_RING_CAPACITY + 10 {
            entity.apply_delta(DeltaEvent {
                timestamp_us: i as u64,
                slice_mask: 1,
                endpoint_idx: 0,
                delta_value: 0.01,
                _pad: 0,
            });
        }
        
        // AGENT_TRAP: 溢出时应触发 panic（或快照刷盘），而不是静默覆盖
    }
    
    #[test]
    fn test_frobenius_distance_ignores_padding() {
        let mut a = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut b = [[0.0f32; K_MAX]; MAX_SLICES];
        
        a[0][0] = 0.5; a[0][1] = 0.5;
        b[0][0] = 0.5; b[0][1] = 0.5;
        
        // 故意在 padding 列放不同值
        a[0][7] = 999.0;
        b[0][7] = -999.0;
        
        let dims = [2u8];
        let dist = frobenius_distance(&a, &b, &dims);
        
        assert!(dist < 1e-6, "Padding should not affect distance: {}", dist);
    }
}
```

### 9.2 性能基准

```rust
#[cfg(test)]
mod benches {
    use test::Bencher;
    
    #[bench]
    fn bench_state_query_no_deltas(b: &mut Bencher) {
        let entity = create_test_entity();
        b.iter(|| query_state(&entity, 1708732800000000));
    }
    // 目标: < 500ns（无 delta 时，纯 memcpy）
    
    #[bench]
    fn bench_state_query_100_deltas(b: &mut Bencher) {
        let entity = create_entity_with_deltas(100);
        b.iter(|| query_state(&entity, 1708732800000000));
    }
    // 目标: < 2μs（100 个 delta 回放）
    
    #[bench]
    fn bench_cascade_100entities_5hops(b: &mut Bencher) {
        let (matrix, states) = create_test_cascade(100);
        let initial = vec![1.0f32; 100];
        b.iter(|| cascade(&initial, &matrix, &states, 5, 0.01));
    }
    // 目标: < 100μs（AVX-512）
}
```

---

## 10. 部署与运维

### 10.1 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `SIGMA4_ENTITY_CAPACITY` | 65536 | 最大实体数 |
| `SIGMA4_SLICE_CAPACITY` | 16 | 最大切面数 |
| `SIGMA4_DELTA_RING_SIZE` | 1024 | 增量环容量 |
| `SIGMA4_SNAPSHOT_INTERVAL_MS` | 100 | 快照刷盘间隔 |
| `SIGMA4_LOG_PATH` | "/var/lib/sigma4/log" | 日志目录 |
| `SIGMA4_BIND_ADDR` | "0.0.0.0:8443" | TCP + TLS 监听地址 |
| `SIGMA4_TLS_CERT` | "/etc/sigma4/cert.pem" | TLS 证书路径 |
| `SIGMA4_TLS_KEY` | "/etc/sigma4/key.pem" | TLS 私钥路径 |

### 10.2 监控指标

```
sigma4_ingestion_latency_us{quantile="0.99"} 0.8
sigma4_cascade_latency_us{quantile="0.99"} 85.0
sigma4_state_query_latency_ns{quantile="0.99"} 450
sigma4_entity_count 183
sigma4_slice_count 4
current
sigma4_delta_ring_utilization 0.05
sigma4_snapshot_write_latency_ms 3.2
sigma4_simd_mode{mode="avx512"} 1
sigma4_simd_mode{mode="scalar"} 0
```

---

## 附录 A：Agent 陷阱清单

### 数学层陷阱

| 陷阱 | 后果 | 检测方法 |
|------|------|----------|
| Duchi 输入含 NaN | panic 或错误投影 | `assert!(!v.iter().any(|&x| x.is_nan()))` |
| 投影后未验证 | 行和 ≠ 1，后续计算发散 | 单元测试检查容差 < 1e-5 |
| 退化维度移除 | 后续 delta 越界 panic | 永远保持 K_max 维，仅将权重置 0 |
| Frobenius 包含 padding | 距离计算错误 | 单元测试验证 padding 不影响距离 |

### 并发层陷阱

| 陷阱 | 后果 | 正确做法 |
|------|------|----------|
| 多生产者无锁环 | 数据竞争，静默损坏 | 单生产者（SPSC），或用 MPSC 队列 |
| 使用 Relaxed 内存序跨线程 | 可见性问题，读取 stale 数据 | Producer 用 Release，Consumer 用 Acquire |
| Compute 和 Logger 同线程 | IO 阻塞计算，延迟飙升 | Logger 独立线程，批量刷盘 |

### 网络层陷阱

| 陷阱 | 后果 | 正确做法 |
|------|------|----------|
| 在 HTTP/2 上叠加自定义帧 | 中间件拒绝，连接断开 | TCP + TLS + 自定义应用层 |
| 无最大帧大小限制 | 内存耗尽攻击 | `assert!(len <= 65536)` |
| 未处理 TLS ALPN 失败 | 降级到不安全连接 | 拒绝非 "sigma4" ALPN 的连接 |

### 数值层陷阱

| 陷阱 | 后果 | 正确做法 |
|------|------|----------|
| 脆性实体统一衰减 | LTCM 式崩溃被低估 | brittle > 0.5 时，未突破阈值快速衰减，突破后不衰减 |
| 约束应用后不重新投影 | 行和 ≠ 1，坐标无效 | 约束应用后必须调用 Duchi 投影 |
| valid_from 和 valid_until 同向兜底 | 失效实体提前被排除 | valid_from 最小值兜底，valid_until 最大值兜底 |

---

## 附录 B：决策树

```
开始实现 Σ⁴-Engine
│
├─ 先做内存布局？
│  ├─ YES → 定义 Entity, DeltaEvent, Relation 的 repr(C) 布局
│  └─ NO  → STOP。没有正确内存布局，后续全部错误。
│
├─ 先做网络层？
│  ├─ YES → STOP。网络层依赖前面全部，最后做。
│  └─ NO  → 继续
│
├─ 实现单纯形投影？
│  ├─ 用排序？ → YES。O(k log k)，k ≤ 8，足够快。
│  ├─ 用选择算法？ → NO。更复杂，收益不明显。
│  └─ 用梯度下降？ → STOP。这是凸问题，有解析解。
│
├─ 实现增量环？
│  ├─ 多线程写入？
│  │  ├─ YES → 用 crossbeam-channel MPSC，不要无锁环。
│  │  └─ NO  → 单线程直接写，无需原子操作。
│  └─ 环满时？
│     ├─ 覆盖旧数据？ → NO。数据丢失不可接受。
│     └─ 触发快照？ → YES。刷盘后重置环。
│
├─ 实现 SIMD？
│  ├─ 直接写 AVX-512？ → NO。必须检测 CPU 特性。
│  ├─ 写 std::simd？ → MAYBE。Rust std::simd 尚不稳定。
│  └─ 写 target_feature + 标量 fallback？ → YES。最可靠。
│
├─ 实现传输协议？
│  ├─ 用 gRPC？ → NO。字段标签 overhead 太大。
│  ├─ 用 HTTP/2 自定义帧？ → NO。中间件不兼容。
│  └─ 用 TCP + TLS + Length-Prefix？ → YES。原始、可控、低开销。
│
└─ 实现级联推理？
   ├─ 先写全图遍历？ → NO。用 CSR 稀疏矩阵 × 向量。
   ├─ 忽略脆性阈值？ → NO。必须处理 brittle > 0.5 的特殊衰减。
   └─ 用 SIMD SpMV？ → YES。但必须有标量 fallback。
```

---

## 附录 C：最小可运行 Rust 骨架

```rust
// Cargo.toml:
// [package]
// name = "sigma4-engine"
// version = "0.1.0"
// edition = "2021"
// 
// [dependencies]
// tokio = { version = "1", features = ["full"] }
// rustls = "0.22"
// tokio-rustls = "0.25"
// rayon = "1.8"
// 
// [dev-dependencies]
// criterion = "0.5"

use std::simd::*;

const MAX_SLICES: usize = 16;
const K_MAX: usize = 8;
const MAX_ENTITIES: usize = 65536;
const DELTA_RING_CAPACITY: usize = 1024;

#[repr(C, align(64))]
pub struct Entity {
    pub id: u32,
    pub entity_type: u8,
    pub flags: u8,
    pub num_slices: u8,
    pub _pad0: u8,
    pub valid_from: i64,
    pub valid_until: i64,
    pub coordinates: [[f32; K_MAX]; MAX_SLICES],
    pub slice_dims: [u8; MAX_SLICES],
    pub delta_ring: [DeltaEvent; DELTA_RING_CAPACITY],
    pub ring_head: u32,
    pub ring_tail: u32,
    pub name_ptr: *const u8,
    pub name_len: u16,
    pub _pad1: [u8; 6],
}

#[repr(C, packed)]
pub struct DeltaEvent {
    pub timestamp_us: u64,
    pub slice_mask: u16,
    pub endpoint_idx: u8,
    pub delta_value: f32,
    pub _pad: u8,
}

fn main() {
    println!("Σ⁴-Engine v1.1");
    println!("Entity size: {} bytes", std::mem::size_of::<Entity>());
    println!("Max memory: {} MB", 
        MAX_ENTITIES * std::mem::size_of::<Entity>() / 1024 / 1024
    );
    
    // 验证对齐
    assert_eq!(std::mem::align_of::<Entity>(), 64);
    assert!(std::mem::size_of::<Entity>() <= 64 * 1024);
}
```

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v1.0 | 2024-07-10 | 初始规范 |
| v1.1 | 2024-07-10 | Agent 优化版：显式标记所有陷阱、并发模型、硬件假设、时间戳双向语义、传输层协议、脆性阈值 |

---

*本文档为自包含技术规范，不依赖任何外部上下文。所有数学定义、协议格式、算法伪代码和代码骨架均为首次披露。*
