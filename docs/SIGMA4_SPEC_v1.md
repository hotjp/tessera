# Σ⁴-Engine 技术开发规范 v1.0

> **文档性质**：自包含技术规范  
> **目标读者**：任何具备系统编程与数学基础的工程团队  
> **约束**：本文档不依赖任何外部上下文；不引用特定领域数据集  

---

## 目录

1. [系统概述](#1-系统概述)
2. [核心数学模型](#2-核心数学模型)
3. [数据实体规范](#3-数据实体规范)
4. [系统架构](#4-系统架构)
5. [二进制通信协议](#5-二进制通信协议)
6. [算法规范](#6-算法规范)
7. [存储层规范](#7-存储层规范)
8. [切面扩展协议](#8-切面扩展协议)
9. [测试与验证](#9-测试与验证)
10. [部署与运维](#10-部署与运维)

---

## 1. 系统概述

### 1.1 定位

Σ⁴-Engine 是一个**确定性离散事件级联推理引擎**。核心能力：

- 将任意类型的可命名对象（组织、事件、人物）建模为**多切面单纯形坐标**
- 通过**稀疏事件驱动**更新状态，支持微秒级时间精度
- 在**固定内存预算**内执行确定性级联传播（目标延迟 < 100μs）
- 通过**可逆压缩编码**实现低带宽传输

### 1.2 设计约束

| 约束项 | 数值 | 说明 |
|--------|------|------|
| 实体容量 | ≤ 65,536 | 预分配槽位，u16 索引 |
| 切面数 | ≤ 16 | 预分配，运行时热加载 |
| 端点数/切面 | ≤ 8 | 预分配，支持切面差异化 |
| 状态更新延迟 | < 1μs | 无锁 CAS 写入环形缓冲区 |
| 级联推理延迟 | < 100μs | 5 跳，AVX-512 SIMD |
| 传输帧大小 | < 30 bytes/更新 | 自定义二进制，无字段标签 |
| 快照恢复 | < 500ms | 最新快照 + 增量回放 |

### 1.3 技术栈

- **热路径**：Rust（内存安全、零成本抽象、SIMD 原生支持）
- **管理面**：Python（配置管理、可视化、切面注册）
- **存储**：内存常驻 + SSD 顺序日志
- **传输**：HTTP/2 自定义二进制帧

---

## 2. 核心数学模型

### 2.1 标准单纯形

**定义 2.1**：k-端点标准单纯形 Δᵏ⁻¹ ⊂ ℝᵏ 定义为：

> **Δᵏ⁻¹ = {x ∈ ℝᵏ : Σᵢ₌₁ᵏ xᵢ = 1, xᵢ ≥ 0}**

**引理 2.1**（Duchi et al. 2008）：向量 v ∈ ℝᵏ 到 Δᵏ⁻¹ 的欧几里得投影可在 O(k) 时间内计算。

```rust
/// Duchi 单纯形投影算法
/// 输入: v ∈ ℝᵏ（任意实向量）
/// 输出: x ∈ Δᵏ⁻¹（最近单纯形点）
pub fn project_onto_simplex(v: &[f32], k: usize) -> Vec<f32> {
    let mut u = v[..k].to_vec();
    u.sort_by(|a, b| b.partial_cmp(a).unwrap());  // 降序
    
    let mut cssv = 0.0;
    let mut rho = 0usize;
    
    for (i, &ui) in u.iter().enumerate() {
        cssv += ui;
        if ui * (i + 1) as f32 > (cssv - 1.0) {
            rho = i;
        }
    }
    
    let theta = (u[..=rho].iter().sum::<f32>() - 1.0) / (rho + 1) as f32;
    
    v[..k].iter()
        .map(|&vi| (vi - theta).max(0.0))
        .collect()
}
```

### 2.2 实体状态张量

**定义 2.2**：设系统有 S 个切面，第 s 个切面有 Kₛ 个端点，K_max = maxₛ Kₛ。实体 ξ 的状态张量为：

> **M_ξ ∈ ℝ^{S×K_max}**
>
> 其中第 s 行前 Kₛ 个元素满足 Σᵢ₌₁^{Kₛ} M_ξ[s,i] = 1，M_ξ[s,i] ≥ 0

**定义 2.3**（Frobenius 距离）：两实体 ξ₁, ξ₂ 的距离为：

> **d(ξ₁, ξ₂) = ||M_ξ₁ − M_ξ₂||_F = √[Σₛ₌₁^S Σᵢ₌₁^{K_max} (M_ξ₁[s,i] − M_ξ₂[s,i])²]**

**定理 2.1**（唯一性）：在固定切面配置下，若两个实体的观测数据不同，则 M_ξ₁ ≠ M_ξ₂。

*证明概要*：每个切面的坐标计算是观测数据的确定性函数；Duchi 投影在非退化情况下是单射；拼接保持单射性。

### 2.3 帕累托约束投影

**定义 2.4**：约束集 C 是若干线性不等式的交集：

> **C = {x ∈ ℝ^{S×K_max} : A·vec(x) ≤ b}**

当 C ∩ (Δᵏ¹⁻¹ × ... × Δᵏˢ⁻¹) ≠ ∅ 时，求解标准凸二次规划：

> **min ||x − x_raw||², s.t. x ∈ C**

当约束冲突（可行域为空）时，求解多目标帕累托前沿：

> **min (||x − x_raw||, maxᵢ violationᵢ(x))**

返回帕累托前沿解集，由策略函数选择最终解。

```rust
/// 帕累托投影（简化版：假设无冲突，标准QP）
pub fn pareto_project(
    raw: &[[f32; K_MAX]; MAX_SLICES],
    constraints: &[Constraint],
    slice_dims: &[u8],
) -> [[f32; K_MAX]; MAX_SLICES] {
    let mut result = *raw;
    
    for (s, &k_s) in slice_dims.iter().enumerate() {
        let k = k_s as usize;
        let mut row = [0.0f32; K_MAX];
        row[..k].copy_from_slice(&raw[s][..k]);
        
        // 应用约束（如有）
        for c in constraints.iter().filter(|c| c.slice == s) {
            apply_bound(&mut row, c);
        }
        
        // 投影回单纯形
        let projected = project_onto_simplex(&row, k);
        result[s][..k].copy_from_slice(&projected);
    }
    
    result
}
```

---

## 3. 数据实体规范

### 3.1 统一实体模型

系统中所有可命名对象共享同一框架，通过 `type` 字段区分语义：

```rust
#[repr(u8)]
pub enum EntityType {
    Organization = 0,   // 持存型实体（机构、组织）
    Event = 1,          // 瞬时型实体（事件、冲击）
    Person = 2,         // 代理型实体（个人、家族）
}

#[repr(C, align(64))]
pub struct Entity {
    // === 身份 ===
    pub id: u32,                    // 内部索引（非 UUID，紧凑）
    pub entity_type: u8,
    pub flags: u8,                  // bit 0: is_boundary_breaking
                                    // bit 1: is_constrained
                                    // bits 2-7: reserved
    pub num_slices: u8,
    pub _pad0: u8,
    
    // === 公共时间轴（秒级 Unix 时间戳）===
    pub valid_from: i64,            // 实体生效起始
    pub valid_until: i64,           // i64::MAX = 当前有效
    
    // === 切面坐标 ===
    pub coordinates: [[f32; K_MAX]; MAX_SLICES],
    pub slice_dims: [u8; MAX_SLICES],
    
    // === 增量环（固定大小环形缓冲区）===
    pub delta_ring: [DeltaEvent; DELTA_RING_CAPACITY],
    pub ring_head: u32,
    pub ring_tail: u32,
    
    // === 稳态属性（查询过滤用，不参级联计算）===
    pub steady_state: SteadyState,
    
    // === 元数据（非热路径，可选加载）===
    pub name_ptr: *const u8,        // 指向外部字符串池
    pub name_len: u16,
    pub _pad1: [u8; 6],
}

#[repr(C, packed)]
pub struct DeltaEvent {
    pub timestamp_us: u64,          // 微秒级 Unix 时间戳
    pub slice_mask: u16,            // 哪些切面被修改（位掩码）
    pub endpoint_idx: u8,           // 端点索引
    pub delta_value: f32,           // 变更量（相对值）
    pub _pad: u8,
}

#[repr(C)]
pub struct SteadyState {
    pub geography: u16,             // 国家/地区编码
    pub industry: u16,              // 行业分类
    pub ownership_type: u8,         // 所有制类型
    pub _pad: [u8; 5],
}
```

**常量定义**：

```rust
pub const MAX_SLICES: usize = 16;        // 最大切面数
pub const K_MAX: usize = 8;              // 最大端点数/切面
pub const DELTA_RING_CAPACITY: usize = 1024;  // 增量环容量
pub const MAX_ENTITIES: usize = 65536;   // 最大实体数
```

**内存占用**：单个实体 ≈ 16 KB（含 1024 × 16B 增量环）× 65,536 ≈ 1 GB 常驻内存。

### 3.2 关系模型

```rust
#[repr(C)]
pub struct Relation {
    pub from_id: u32,               // 源实体索引
    pub to_id: u32,                 // 目标实体索引
    pub relation_type: u8,          // 0: ORGANIZATIONAL
                                    // 1: CAUSAL_MARKET
                                    // 2: CAUSAL_FINANCIAL
                                    // 3: NARRATIVE
                                    // 4: PERSONNEL
    pub weight: f32,                // 耦合权重 [0, 1]
    pub time_lag_us: u32,           // 传导时滞（微秒）
    pub valid_from: i64,            // 关系生效时间
    pub valid_until: i64,           // 关系失效时间
}
```

### 3.3 事件模型

Event 是特殊的 Entity，允许坐标**越界**（不投影回单纯形）：

```rust
pub struct EventPayload {
    pub event_type: u8,             // 0: GEOPOLITICAL
                                    // 1: MONETARY
                                    // 2: COMMODITY
                                    // 3: CORPORATE
    pub impact_vector: [[f32; K_MAX]; MAX_SLICES],  // 非归一化冲击
    pub is_boundary_breaking: bool, // true: 不投影回单纯形
}
```

---

## 4. 系统架构

### 4.1 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Data Ingestion                                │
│  传感器 | 行情网关 | NLP 管道 | 心跳探针 → 二进制帧 (18-30 bytes)    │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Ingestion Gateway (Rust)                        │
│  - 帧解码 → 校验 → 时序排序 → 无锁写入 Delta Ring                    │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼ 内存直写
┌─────────────────────────────────────────────────────────────────────┐
│                      Core Engine (Rust, pinned)                      │
│                                                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │ Entity Pool     │  │ Slice Registry  │  │ Cascade Matrix  │     │
│  │ (N × 16KB)      │  │ (YAML 热加载)    │  │ (CSR 稀疏)       │     │
│  │ 常驻内存         │  │ 切面定义 + 端点  │  │ 预计算 + SIMD    │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Compute Kernel (AVX-512 / SVE)                                 │  │
│  │ - Sparse Matrix-Vector (SpMV)                                  │  │
│  │ - Simplex Projection (SIMD 并行)                               │  │
│  │ - Pareto Solve (约束检查 + 快速路径)                            │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Snapshot Logger (后台线程)                                      │  │
│  │ - 每 100ms 批量刷盘                                             │  │
│  │ - Delta-of-Delta 编码                                           │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼ HTTP/2 流
┌─────────────────────────────────────────────────────────────────────┐
│                      Query API (hyper / Rust)                        │
│  /state       → 当前状态 (~48 bytes)                                 │
│  /cascade     → 级联结果 (~800 bytes)                                │
│  /snapshot    → 全量快照 (~N × 48 bytes)                             │
│  /stream      → WebSocket 实时 delta 流                              │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 组件交互

| 组件 | 职责 | 延迟要求 |
|------|------|----------|
| Ingestion Gateway | 协议解码、校验、无锁写入 | < 1μs/帧 |
| Entity Pool | 内存常驻实体数组，索引直读 | ~50ns（memcpy） |
| Slice Registry | 切面定义热加载，坐标重算 | 后台异步 |
| Cascade Matrix | CSR 稀疏矩阵，预计算权重 | N/A（离线） |
| Compute Kernel | SIMD 级联推理、单纯形投影 | < 100μs |
| Snapshot Logger | 增量编码、顺序写 SSD | < 10ms/批次 |

---

## 5. 二进制通信协议

### 5.1 通用帧头

所有通信帧共享 8 字节头部：

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         Magic (0xCAFE)        |  Version  |  Frame Type   |RSV|
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Payload Length                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| 字段 | 大小 | 说明 |
|------|------|------|
| Magic | 2 bytes | 0xCAFE（大端） |
| Version | 1 byte | 协议版本，当前 0x01 |
| Frame Type | 1 byte | 见下表 |
| RSV | 1 byte | 保留 |
| Payload Length | 3 bytes | 大端无符号整数，最大 16MB |

### 5.2 帧类型定义

```rust
#[repr(u8)]
pub enum FrameType {
    StateUpdate     = 0x01,  // 实体状态增量更新
    StateQuery      = 0x02,  // 查询实体状态
    StateResponse   = 0x03,  // 状态查询响应
    CascadeRun      = 0x04,  // 执行级联推理
    CascadeResult   = 0x05,  // 级联结果
    SnapshotReq     = 0x06,  // 请求快照
    SnapshotResp    = 0x07,  // 快照响应
    Heartbeat       = 0xFF,  // 心跳
}
```

### 5.3 STATE_UPDATE 帧体

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Entity ID                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Timestamp (μs)                           |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Slice Mask  | Endpoint  |          Delta Value                 |
|   (u16)     |  (u8)     |            (f32)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**总大小**：8 bytes（帧头）+ 17 bytes（体）= **25 bytes**

### 5.4 CASCADE_RUN 帧体

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Event Entity ID                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Start Time (μs)                            |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Max Hops  |    Theta (f32)    |      Num Targets (u16)       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    Target Entity IDs (4 bytes each)             |
|                              ...                                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 5.5 CASCADE_RESULT 帧体

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Num Affected (u16)  | Max Hop (u8) |      Total Lag (μs)      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
|              (Entity ID, Confidence) pairs × N                 |
|              4 bytes + 4 bytes each                           |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

---

## 6. 算法规范

### 6.1 状态查询（含增量回放）

```rust
/// 查询实体在时刻 T 的完整状态
/// 复杂度: O(1) 基态拷贝 + O(R) delta 回放, R = 环中有效 delta 数
pub fn query_state(entity: &Entity, query_time_us: u64) -> EntitySnapshot {
    let mut coords = entity.coordinates;
    
    // 回放 delta ring 中时间戳 ≤ query_time_us 的所有事件
    let tail = entity.ring_tail as usize;
    let head = entity.ring_head as usize;
    
    for i in tail..head {
        let delta = &entity.delta_ring[i % DELTA_RING_CAPACITY];
        if delta.timestamp_us > query_time_us {
            break;  // delta ring 按时间排序
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
    
    EntitySnapshot {
        id: entity.id,
        coordinates: coords,
        slice_dims: entity.slice_dims,
        num_slices: entity.num_slices,
    }
}
```

### 6.2 稀疏矩阵-向量乘法（SIMD）

```rust
use std::simd::*;

/// CSR 格式稀疏矩阵 × 向量
/// 输入: x (N), 矩阵 A (CSR)
/// 输出: y = A · x
/// 复杂度: O(nnz), nnz = 非零元数量
pub fn spmv_csr(
    x: &[f32],
    row_ptr: &[u32],
    col_idx: &[u32],
    values: &[f32],
    y: &mut [f32],
) {
    let n = row_ptr.len() - 1;
    
    for i in 0..n {
        let start = row_ptr[i] as usize;
        let end = row_ptr[i + 1] as usize;
        
        // SIMD 累加（AVX-512: 16 × f32）
        let mut sum_vec = f32x16::splat(0.0);
        let mut j = start;
        
        while j + 16 <= end {
            //  gather: 从 x[col_idx] 加载 16 个值
            let cols = &col_idx[j..j + 16];
            let vals = f32x16::from_slice(&values[j..j + 16]);
            
            let x_gathered = f32x16::from_array([
                x[cols[0] as usize], x[cols[1] as usize], /* ... */ x[cols[15] as usize]
            ]);
            
            sum_vec += vals * x_gathered;
            j += 16;
        }
        
        let mut sum = sum_vec.reduce_add();
        
        // 标量收尾
        for k in j..end {
            sum += values[k] * x[col_idx[k] as usize];
        }
        
        y[i] = sum;
    }
}
```

### 6.3 级联推理算法

```rust
/// 级联推理核心
/// 输入: 初始冲击向量, 最大跳数, 置信度阈值
/// 输出: 受影响实体列表 (ID, 置信度, 累积时滞)
pub fn cascade(
    initial_vector: &[f32],      // N 维冲击向量
    matrix: &CascadeMatrix,      // CSR 稀疏矩阵
    decay_params: &[f32],        // 每实体衰减系数
    max_hops: u8,
    theta: f32,
) -> Vec<CascadeResult> {
    let n = matrix.n as usize;
    let mut current = initial_vector.to_vec();
    let mut next = vec![0.0f32; n];
    let mut results = Vec::new();
    
    // 记录每实体的跳数和最小时滞
    let mut hop_count = vec![u8::MAX; n];
    let mut min_lag = vec![u32::MAX; n];
    
    for hop in 0..max_hops {
        spmv_csr(&current, &matrix.row_ptr, &matrix.col_idx, &matrix.weights, &mut next);
        
        for i in 0..n {
            next[i] *= decay_params[i];
            
            if next[i] >= theta {
                if hop_count[i] == u8::MAX {
                    hop_count[i] = hop;
                    min_lag[i] = matrix.time_lag_us[i];
                }
            } else {
                next[i] = 0.0;  // 剪枝
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

**原理**：每切面坐标满足 Σαᵢ = 1，只存储前 K_s−1 个值。

```rust
pub struct SimplexCodec;

impl SimplexCodec {
    /// 编码: S × K_max 矩阵 → 紧凑字节流
    /// 压缩率: (K_s−1)/K_s，K_s=4 时节省 25%
    pub fn encode(M: &[[f32; K_MAX]; MAX_SLICES], slice_dims: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        for (s, &k_s) in slice_dims.iter().enumerate() {
            let k = k_s as usize;
            assert!((M[s][..k].iter().sum::<f32>() - 1.0).abs() < 1e-5);
            for i in 0..k - 1 {
                buf.extend_from_slice(&M[s][i].to_le_bytes());
            }
        }
        buf
    }
    
    /// 解码: 字节流 → 完整矩阵（100% 可逆）
    pub fn decode(data: &[u8], slice_dims: &[u8], K_max: usize) -> [[f32; K_MAX]; MAX_SLICES] {
        let mut M = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut offset = 0;
        
        for (s, &k_s) in slice_dims.iter().enumerate() {
            let k = k_s as usize;
            let bytes_to_read = (k - 1) * 4;
            
            for i in 0..k - 1 {
                M[s][i] = f32::from_le_bytes([
                    data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
                ]);
                offset += 4;
            }
            
            // 恢复最后一个元素（单纯形约束）
            M[s][k - 1] = 1.0 - M[s][..k - 1].iter().sum::<f32>();
        }
        
        M
    }
}
```

### 7.2 快照日志格式

```
文件头 (32 bytes):
  Magic:        [u8; 4]   = "SS4\0"
  Version:      u32       = 1
  NumEntities:  u32
  NumSlices:    u32
  SliceDims:    [u8; 16]
  Reserved:     [u8; 4]

实体记录 (变长):
  Entity ID:    u32
  Type:         u8
  Flags:        u8
  NumSlices:    u8
  ValidFrom:    i64
  ValidUntil:   i64
  EncodedMatrix: Vec<u8>  (SimplexCodec 编码)
  NameLen:      u16
  Name:         [u8; N]   (UTF-8)
  Padding:      [u8; M]   (8 字节对齐)
```

**写入策略**：只追加（append-only），每 100ms 批量刷盘。

**恢复协议**：
1. 读取最新完整快照
2. 回放快照后的增量日志
3. 重建 Entity Pool 内存状态

---

## 8. 切面扩展协议

### 8.1 运行时热加载

```rust
impl Engine {
    /// 热加载新切面（无需停机）
    pub fn add_slice(&mut self, slice_def: SliceDef) -> Result<(), EngineError> {
        let slice_idx = self.registry.slices.len();
        if slice_idx >= MAX_SLICES {
            return Err(EngineError::SliceLimitExceeded);
        }
        
        self.registry.slices.push(slice_def.clone());
        
        // 并行初始化所有实体的新切面坐标
        self.entity_pool.par_iter_mut().for_each(|entity| {
            entity.slice_dims[slice_idx] = slice_def.num_endpoints;
            entity.num_slices += 1;
            
            // 均匀初始化
            let k = slice_def.num_endpoints as usize;
            let uniform = 1.0 / k as f32;
            for i in 0..k {
                entity.coordinates[slice_idx][i] = uniform;
            }
        });
        
        // 后台异步重建级联矩阵
        self.rebuild_matrix_async();
        
        Ok(())
    }
}
```

### 8.2 切面 YAML 配置模板

```yaml
slice_registry:
  version: "1.0"
  slices:
    - id: "power"
      name: "权力拓扑"
      endpoints:
        - id: "sovereignty"
          metrics:
            - id: "govt_contract_ratio"
              weight: 0.35
              normalize: { method: "sigmoid", params: { scale: 10.0 } }
        - id: "capital"
          metrics:
            - id: "aum_log"
              weight: 0.30
              normalize: { method: "log_sigmoid", params: { center: 2.0 } }
        - id: "production"
          metrics:
            - id: "physical_output"
              weight: 0.35
              normalize: { method: "log_sigmoid", params: { center: 2.0 } }
        - id: "narrative"
          metrics:
            - id: "media_reach"
              weight: 0.25
              normalize: { method: "log_sigmoid", params: { center: 2.0 } }

    - id: "dynamics"
      name: "动态模式"
      endpoints:
        - id: "stable"
        - id: "cyclic"
        - id: "episodic"
        - id: "transformative"

    - id: "epistemic"
      name: "认知可达"
      endpoints:
        - id: "opaque"
        - id: "disclosed"
        - id: "inferred"
        - id: "manipulated"

    - id: "cascade"
      name: "级联响应"
      endpoints:
        - id: "elastic"
        - id: "plastic"
        - id: "brittle"
        - id: "absorptive"
```

---

## 9. 测试与验证

### 9.1 单元测试矩阵

| 测试项 | 输入 | 预期输出 | 验收标准 |
|--------|------|----------|----------|
| 单纯形投影 | v = [2.0, 1.0, -0.5, 0.0] | x ∈ Δ³ | Σxᵢ = 1, xᵢ ≥ 0, ||x−v|| 最小 |
| 编码-解码 | 随机合法矩阵 M | M' = M | 逐元素误差 < 1e-6 |
| 增量回放 | 基态 + 10 个 delta | 状态 = 基态 + Σdelta | 数值一致 |
| CSR SpMV | 稀疏矩阵 A, 向量 x | y = A·x | 与稠密计算逐元素一致 |
| 级联推理 | 星型拓扑，中心冲击 | 叶子节点收到衰减后信号 | 置信度 = 权重 × 衰减^k |
| 帕累托投影 | raw + 上界约束 | 投影后约束满足 | violation = 0 |

### 9.2 性能基准

```rust
#[cfg(test)]
mod benches {
    use test::Bencher;
    
    #[bench]
    fn bench_state_query(b: &mut Bencher) {
        let entity = create_test_entity();
        b.iter(|| query_state(&entity, 1708732800000000));
    }
    // 目标: < 1μs
    
    #[bench]
    fn bench_cascade_100entities(b: &mut Bencher) {
        let (matrix, decay) = create_test_cascade_matrix(100);
        let initial = vec![1.0f32; 100];
        b.iter(|| cascade(&initial, &matrix, &decay, 5, 0.01));
    }
    // 目标: < 100μs
    
    #[bench]
    fn bench_simplex_codec(b: &mut Bencher) {
        let M = create_test_matrix();
        let dims = [4u8; 4];
        b.iter(|| SimplexCodec::encode(&M, &dims));
    }
    // 目标: < 5μs
}
```

---

## 10. 部署与运维

### 10.1 部署拓扑

```
[Load Balancer (L4)]
       │
  ┌────┴────┐
  ▼         ▼
[Node 1]  [Node 2]   Σ⁴ Engine Cluster
[16GB]    [16GB]     每个节点独立内存
[4core]   [4core]    通过 gossip 同步实体元数据
  │         │
  └────┬────┘
       ▼
[SSD RAID 0]          快照日志存储
```

### 10.2 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `SIGMA4_ENTITY_CAPACITY` | 65536 | 最大实体数 |
| `SIGMA4_SLICE_CAPACITY` | 16 | 最大切面数 |
| `SIGMA4_DELTA_RING_SIZE` | 1024 | 增量环容量 |
| `SIGMA4_SNAPSHOT_INTERVAL_MS` | 100 | 快照刷盘间隔 |
| `SIGMA4_LOG_PATH` | "/var/lib/sigma4/log" | 日志目录 |
| `SIGMA4_BIND_ADDR` | "0.0.0.0:8080" | HTTP/2 监听地址 |

### 10.3 监控指标（Prometheus）

```
sigma4_ingestion_latency_us{quantile="0.99"} 0.8
sigma4_cascade_latency_us{quantile="0.99"} 85.0
sigma4_entity_count 183
sigma4_slice_count 4
sigma4_delta_ring_utilization 0.05
sigma4_snapshot_write_latency_ms 3.2
```

---

## 附录 A：最小可运行 Rust 骨架

```rust
// Cargo.toml:
// [dependencies]
// tokio = { version = "1", features = ["full"] }
// hyper = { version = "1", features = ["full"] }
// serde = { version = "1", features = ["derive"] }

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
    println!("Σ⁴-Engine v1.0");
    println!("Entity size: {} bytes", std::mem::size_of::<Entity>());
    println!("Max memory: {} MB", 
        MAX_ENTITIES * std::mem::size_of::<Entity>() / 1024 / 1024
    );
}
```

---

## 附录 B：版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v1.0 | 2024-07-10 | 初始规范 |

---

*本文档为自包含技术规范，不依赖任何外部上下文。所有数学定义、协议格式、算法伪代码和代码骨架均为首次披露。*
