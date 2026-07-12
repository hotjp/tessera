# Σ⁴-System v4.0：微秒级稀疏状态级联引擎

> **版本**：v4.0（架构重写）  
> **核心认知修正**：状态更新是稀疏事件驱动的（非连续采样）；查询是惰性求值的；计算必须在内存中完成；传输必须是二进制的  
> **性能约束**：状态更新 < 1ms；级联推理 < 100μs；HTTP 导出 < 50 bytes/payload；切面动态扩展无停机  

---

## 1. 认知修正：从"数据库架构"到"内存计算架构"

我之前的架构（v3.1）犯了一个根本性错误：**用数据库的思维解决实时计算问题**。

v3.1 假设：
- 实体状态存储在 PostgreSQL / DuckDB 中 → 查询时磁盘 I/O
- 级联推理用 Python 实现 → GC 停顿、解释器开销
- HTTP API 返回 JSON → 文本序列化、字段膨胀
- 时间精度到秒 → 无法支持高频事件

这导致系统目标延迟（"snapshot < 5ms"）与实际需求（"μs 级"）差了 **3 个数量级**。

**v4.0 的重新定位**：

| 维度 | v3.1 假设 | v4.0 修正 |
|------|-----------|-----------|
| 状态更新频率 | 年度/季度刷新 | 毫秒级稀疏事件（门电路开闭、心跳探针、实时交易） |
| 状态存储 | 磁盘数据库 | 内存常驻 + 增量日志持久化 |
| 计算延迟 | Python ms 级 | Rust SIMD μs 级 |
| 传输格式 | HTTP JSON | 自定义二进制帧 + HTTP/2 流 |
| 切面扩展 | 重启加载 YAML | 运行时动态追加张量维度 |
| 查询语义 | `snapshot_at(T)` 磁盘查询 | 内存直读当前状态 + 环形缓冲区回放 |

---

## 2. 核心数据模型：稀疏增量时间序列

### 2.1 状态的三层表示

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: 基态 (Base State)                                  │
│  每个实体的「当前坐标」——常驻内存的 float32 数组               │
│  形状: (S, K_max)，S=切面数，K_max=最大端点数                 │
│  访问: O(1) 内存直读                                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ 稀疏事件触发
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: 增量环 (Delta Ring)                                │
│  固定大小的环形缓冲区，存储最近 N 个状态变更                   │
│  每个元素: (timestamp_us, slice_mask, endpoint_idx, delta)   │
│  追加: O(1) 无分配                                           │
│  回放: O(N) N = 缓冲区深度（通常 < 1000）                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ 定期/触发式
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: 快照日志 (Snapshot Log)                            │
│  增量编码后的持久化日志（磁盘/SSD）                            │
│  格式: 二进制顺序写，只追加不修改                              │
│  恢复: 读取最新快照 + replay 增量                              │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 实体内存布局

```rust
// Rust 伪代码（生产实现语言）
#[repr(C, align(64))]  // 64 字节对齐，适配 SIMD
struct EntityState {
    // 标识
    id: u32,              // 4 bytes，实体 ID（内部索引，非 UUID）
    entity_type: u8,      // 1 byte: 0=ORG, 1=EVENT, 2=PERSON
    flags: u8,            // 1 byte: is_chinese, is_boundary_breaking, etc.
    _pad: [u8; 2],        // 2 bytes padding
    
    // 时间轴（公共主轴）
    valid_from_us: u64,   // 8 bytes，微秒级 Unix 时间戳
    valid_until_us: u64,  // 8 bytes，u64::MAX = 当前有效
    
    // 基态（当前坐标）
    // 形状: (MAX_SLICES, MAX_ENDPOINTS)
    // 当前: MAX_SLICES = 16（预留扩展），MAX_ENDPOINTS = 8
    // 实际使用: slice_dims[0..S]，每行前 K_s 个元素有效
    coordinates: [[f32; 8]; 16],  // 16 × 8 × 4 = 512 bytes
    slice_dims: [u8; 16],         // 每个切面的实际端点数
    num_slices: u8,               // 当前激活的切面数
    _pad2: [u8; 7],
    
    // 增量环（固定大小环形缓冲区）
    // 容量: 1024 个 delta 事件
    delta_ring: [DeltaEvent; 1024],
    ring_head: u32,       // 写入位置
    ring_tail: u32,       // 最旧有效位置
    
    // 元数据指针（非热路径）
    display_name_ptr: *const u8,
    display_name_len: u16,
}

#[repr(C, packed)]
struct DeltaEvent {
    timestamp_us: u64,    // 8 bytes，变更时间
    slice_mask: u16,      // 2 bytes，哪些切面被修改（位掩码）
    // 对于每个被修改的切面，存储 endpoint_idx + delta_value
    // 为简化，假设每次只改一个端点
    endpoint_idx: u8,     // 1 byte
    delta_value: f32,     // 4 bytes，变更量（非绝对值）
    _pad: [u8; 1],
}  // 总计: 16 bytes

// EntityState 总大小: ~16KB（含 1024 × 16B 的 delta ring）
// 183 实体 × 16KB = ~2.9MB，完全可常驻内存
```

### 2.3 稀疏增量语义

**关键洞察**：实体 99.9% 的时间不变。只在以下事件触发时产生 delta：

| 事件类型 | 触发源 | 更新频率 | Delta 大小 |
|----------|--------|----------|-----------|
| 门电路开闭 | 硬件传感器 | 毫秒级 | 1 个端点 × 4 bytes |
| 心跳探针 | 实体健康检查 | 秒级 | 1 个端点 × 4 bytes |
| 事实话题变更 | NLP 管道 | 秒级~分钟级 | 1-2 个端点 × 4 bytes |
| 股票实时交易 | 行情接入 | 毫秒级 | 1 个端点 × 4 bytes |
| 切面扩展 | 管理员操作 | 极低频 | 整行追加 |

**Delta 编码**：

```
Base:  entity.coordinates[power][capital] = 0.55
Event: 交易记录显示 AUM 增加 10%
Delta: coordinates[power][capital] += 0.03
Ring:  (timestamp=1708732800123456, slice_mask=0b0001, endpoint=1, delta=+0.03)
```

**查询时刻 T 的状态**：

```rust
fn state_at(entity: &EntityState, query_time_us: u64) -> [[f32; 8]; 16] {
    // 1. 从基态复制
    let mut result = entity.coordinates;
    
    // 2. 回放增量环中时间戳 ≤ query_time_us 的所有 delta
    for i in entity.ring_tail..entity.ring_head {
        let delta = &entity.delta_ring[i % 1024];
        if delta.timestamp_us > query_time_us {
            break;  // 增量环按时间排序
        }
        // 应用 delta
        let slice = delta.slice_mask.trailing_zeros() as usize;
        let endpoint = delta.endpoint_idx as usize;
        result[slice][endpoint] += delta.delta_value;
    }
    
    // 3. 投影回单纯形（如果 delta 破坏了约束）
    for s in 0..entity.num_slices {
        project_to_simplex(&mut result[s], entity.slice_dims[s]);
    }
    
    result
}
```

**延迟分析**：
- 基态复制：512 bytes memcpy ≈ **~50ns**
- Delta 回放：平均 < 10 个 delta（99% 情况下） ≈ **~100ns**
- 单纯形投影：O(K_s) ≈ **~200ns**
- **总查询延迟：~350ns**（满足 μs 级要求）

---

## 3. 计算引擎：SIMD 向量级联

### 3.1 邻接矩阵预计算

级联推理的核心不是"图遍历"，而是**稀疏矩阵 × 向量乘法**。

```rust
// 预计算的邻接矩阵
// 形状: (N_entities, N_entities)
// 稀疏存储: CSR 格式（Compressed Sparse Row）
struct CascadeMatrix {
    n: u32,                          // 实体数
    row_ptr: Vec<u32>,              // 每行起始索引
    col_idx: Vec<u32>,              // 列索引
    weights: Vec<f32>,              // 耦合权重
    time_lag_us: Vec<u32>,          // 传导时滞（微秒）
}

// 预计算步骤（离线，非查询路径）
// 1. 从关系定义构建邻接矩阵
// 2. 根据实体类型的衰减系数 λ 调整权重
// 3. 稀疏化：权重 < 0.01 的边丢弃
// 4. CSR 编码
```

### 3.2 SIMD 级联传播

```rust
use std::simd::*;

fn cascade_simd(
    initial_vector: &[f32],      // 初始冲击向量 (N,)
    matrix: &CascadeMatrix,      // CSR 稀疏矩阵
    max_hops: u8,
    theta: f32,
    decay_params: &[f32],        // 每实体类型的衰减系数
) -> Vec<f32> {
    let n = matrix.n as usize;
    let mut current = initial_vector.to_vec();
    let mut next = vec![0.0f32; n];
    
    for _hop in 0..max_hops {
        // SIMD 稀疏矩阵向量乘法
        for i in 0..n {
            let row_start = matrix.row_ptr[i] as usize;
            let row_end = matrix.row_ptr[i + 1] as usize;
            
            // 使用 SIMD 累加（AVX-512: 16 × f32）
            let mut sum = f32x16::splat(0.0);
            let mut j = row_start;
            
            while j + 16 <= row_end {
                let cols = f32x16::from_slice(&matrix.col_idx[j..j+16]);
                let vals = f32x16::from_slice(&matrix.weights[j..j+16]);
                // gather: sum += vals * current[cols]
                sum += vals * gather(current, cols);
                j += 16;
            }
            
            // 标量收尾
            let mut scalar_sum = sum.reduce_add();
            for k in j..row_end {
                scalar_sum += matrix.weights[k] * current[matrix.col_idx[k] as usize];
            }
            
            // 应用衰减
            next[i] = scalar_sum * decay_params[i];
            
            // 阈值剪枝
            if next[i] < theta {
                next[i] = 0.0;
            }
        }
        
        std::mem::swap(&mut current, &mut next);
    }
    
    current
}
```

**延迟分析**（N = 10,000 实体，稀疏度 1%）：
- CSR SpMV：~1000 个非零元/行 × 10,000 行 = 10M 次乘加
- AVX-512：每个 SIMD 寄存器 16 × f32，峰值 ~3 TFLOPS
- 实际：~10 GFLOPS（内存带宽限制）
- 单次推理：**~1ms → 需要优化**

**优化路径**：
1. **实体数控制**：实际活跃实体 < 1000（Schema 剪枝后）
2. **预过滤**：只展开与事件相关的子图（Lazy Cascade）
3. **批处理**：单次事件只影响 ~50 个直接目标 → 子矩阵 50 × 50
4. **优化后延迟**：50 × 50 × 1% = 25 次乘加/行 × 50 行 = 1,250 次操作
   - SIMD：**~5μs**

### 3.3 帕累托投影的 SIMD 实现

```rust
fn pareto_project_simd(
    raw: &[[f32; 8]; 16],
    constraints: &[Constraint],
    slice_dims: &[u8],
) -> [[f32; 8]; 16] {
    // 对于无约束冲突的情况（绝大多数）
    // 使用 SIMD 并行投影到单纯形
    let mut result = *raw;
    
    for s in 0..slice_dims.len() {
        let k = slice_dims[s] as usize;
        // 加载到 SIMD 寄存器
        let row = f32x8::from_slice(&result[s]);
        // Duchi 投影（SIMD 实现）
        let projected = project_simplex_simd(row, k);
        projected.copy_to_slice(&mut result[s]);
    }
    
    result
}
```

---

## 4. 切面动态扩展协议

### 4.1 张量维度动态增长

```rust
// 切面注册表（运行时）
struct SliceRegistry {
    slices: Vec<SliceDef>,      // 动态增长
    k_max: u8,                  // 当前最大端点数
    
    // 预分配的扩展槽位
    reserved_slices: u8,        // 预留的切面槽位（如 16）
    reserved_endpoints: u8,     // 预留的端点槽位（如 8）
}

// 切面热加载
impl SliceRegistry {
    fn add_slice(&mut self, slice: SliceDef) -> Result<(), Error> {
        if self.slices.len() >= self.reserved_slices as usize {
            return Err(Error::SliceLimitExceeded);
        }
        
        if slice.num_endpoints > self.reserved_endpoints {
            return Err(Error::EndpointLimitExceeded);
        }
        
        let slice_idx = self.slices.len();
        self.slices.push(slice);
        
        // 为所有实体初始化新切面的坐标
        // 并行执行（Rayon）
        self.entity_pool.par_iter_mut().for_each(|entity| {
            entity.slice_dims[slice_idx] = slice.num_endpoints;
            entity.num_slices += 1;
            
            // 新切面初始化为均匀分布（或先验值）
            for k in 0..slice.num_endpoints {
                entity.coordinates[slice_idx][k as usize] = 1.0 / slice.num_endpoints as f32;
            }
        });
        
        // 重建邻接矩阵（后台异步）
        self.rebuild_matrix_async();
        
        Ok(())
    }
}
```

**关键设计**：预分配槽位。

- `EntityState.coordinates` 预分配 `[f32; 8]` × 16 = 512 bytes
- 实际使用 `slice_dims[0..num_slices]` 个切面
- 新增切面只需填充下一个槽位，**无需重新分配内存**
- 实体数组预分配连续内存（如 65,536 个槽位），支持动态增长

### 4.2 稳态维度扩展

用户提到"后续可能切出其他稳态维度"。稳态维度 = 不随时间频繁变化的属性（如地理位置、行业分类）。

```rust
// 稳态维度（冷数据，不参与级联计算）
struct SteadyState {
    geography: u8,        // 国家/地区编码
    industry: u16,        // 行业分类编码
    ownership_type: u8,   // 所有制类型
    regulatory_framework: u16,  // 监管框架编码
}

// 稳态维度用于：
// 1. 查询过滤（"只看中国实体"）
// 2. 约束检查（"央企脆性上限"）
// 3. 可视化分组（"按行业着色"）
// 不参与级联传播的 SIMD 计算
```

---

## 5. 二进制传输协议

### 5.1 自定义帧格式

拒绝 JSON。使用紧凑二进制帧。

```
// 通用帧头（8 bytes）
┌──────────┬──────────┬──────────┬──────────┐
│  Magic   │ Version  │  Type    │ Payload  │
│  2 bytes │  1 byte  │  1 byte  │  4 bytes │
│  0xCAFE  │   0x01   │  enum    │  length  │
└──────────┴──────────┴──────────┴──────────┘

// 帧类型枚举
type FrameType = 
    | STATE_UPDATE      = 0x01   // 实体状态增量更新
    | STATE_QUERY       = 0x02   // 查询实体状态
    | CASCADE_RUN       = 0x03   // 执行级联推理
    | CASCADE_RESULT    = 0x04   // 级联结果
    | SNAPSHOT_REQUEST  = 0x05   // 请求快照
    | SNAPSHOT_RESPONSE = 0x06   // 快照响应
    | HEARTBEAT         = 0xFF   // 心跳

// STATE_UPDATE 帧体
┌──────────┬──────────┬──────────┬──────────┬──────────┐
│ Entity   │ Timestamp│ Slice    │ Endpoint │ Delta    │
│  ID      │ (μs)     │  Index   │  Index   │ Value    │
│  4 bytes │  8 bytes │  1 byte  │  1 byte  │ 4 bytes  │
└──────────┴──────────┴──────────┴──────────┴──────────┘
// 总计: 18 bytes + 8 bytes 帧头 = 26 bytes

// CASCADE_RUN 帧体
┌──────────┬──────────┬──────────┬──────────┬──────────┐
│ Event    │ Start    │ Max      │ Theta    │ Num      │
│  Entity  │  Time    │  Hops    │          │ Targets  │
│  ID      │ (μs)     │  1 byte  │ 4 bytes  │  2 bytes │
├──────────┼──────────┼──────────┼──────────┼──────────┤
│ Target Entity IDs (4 bytes each)                           │
└────────────────────────────────────────────────────────────┘
// 典型大小: 8 + 17 + 4×50 = 225 bytes

// CASCADE_RESULT 帧体（稀疏向量）
┌──────────┬──────────┬──────────┬──────────────────────────┐
│ Num      │ Affected │ Total    │ (Entity ID, Confidence)  │
│ Affected │  Max Hop │  Lag     │  pairs                   │
│  2 bytes │  1 byte  │ 4 bytes  │  4 + 4 bytes each        │
└──────────┴──────────┴──────────┴──────────────────────────┘
// 典型: 100 个受影响实体 → 7 + 800 = 807 bytes
```

### 5.2 HTTP/2 流式传输

```rust
// 服务器端（Rust + hyper）
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Error> {
    let body = hyper::body::to_bytes(req.body()).await?;
    let frame = Frame::decode(&body)?;
    
    match frame.frame_type {
        FrameType::STATE_UPDATE => {
            let update = StateUpdate::decode(&frame.payload)?;
            engine.apply_delta(update)?;
            Ok(Response::new(Body::from(vec![0x01])))  // ACK
        }
        
        FrameType::CASCADE_RUN => {
            let request = CascadeRequest::decode(&frame.payload)?;
            let result = engine.cascade(request).await;
            let response = CascadeResult::encode(&result);
            Ok(Response::new(Body::from(response)))
        }
        
        FrameType::STATE_QUERY => {
            let query = StateQuery::decode(&frame.payload)?;
            let state = engine.query_state(query.entity_id, query.at_time);
            let encoded = SimplexCodec::encode(&state, &slice_dims);
            Ok(Response::new(Body::from(encoded)))
        }
        
        _ => Ok(Response::builder().status(400).body(Body::empty())?),
    }
}
```

**传输效率对比**（单次实体状态更新）：

| 格式 | 大小 | 说明 |
|------|------|------|
| JSON | ~200 bytes | `{"entity_id": "soros", "timestamp": 1708732800, "slice": "power", ...}` |
| Protobuf | ~50 bytes | 二进制但仍有字段标签开销 |
| **自定义二进制** | **26 bytes** | 无字段标签，固定偏移 |

---

## 6. 完整系统架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Data Sources                                  │
│  硬件传感器 | 行情网关 | NLP 管道 | 心跳探针 | 用户输入              │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼ 二进制帧 (26-50 bytes)
┌─────────────────────────────────────────────────────────────────────┐
│                      Ingestion Gateway                               │
│  - 协议解码（自定义二进制帧）                                        │
│  - 时序校验（timestamp 单调性）                                      │
│  - 批量聚合（μs 级窗口内合并同一实体的多个 delta）                    │
│  - 写入 Delta Ring（无锁 CAS 操作）                                  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼ 内存直写
┌─────────────────────────────────────────────────────────────────────┐
│                      Core Engine (Rust, pinned memory)               │
│                                                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │ Entity Pool     │  │ Slice Registry  │  │ Cascade Matrix  │     │
│  │ (N × 16KB)      │  │ (动态切面定义)   │  │ (CSR, 稀疏)      │     │
│  │ 常驻内存         │  │ 热加载          │  │ 预计算           │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ SIMD Compute Unit (AVX-512 / SVE)                             │  │
│  │ - SpMV: 稀疏矩阵 × 向量                                       │  │
│  │ - Simplex Projection: 并行投影到单纯形                        │  │
│  │ - Pareto Solve: 约束检查 + 凸二次规划（无冲突时 SIMD 快速路径）│  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ Snapshot Log (后台异步线程)                                     │  │
│  │ - 每 100ms 批量刷盘                                             │  │
│  │ - 增量编码 (Delta-of-Delta)                                     │  │
│  │ - 顺序写 SSD，只追加                                            │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼ HTTP/2 流
┌─────────────────────────────────────────────────────────────────────┐
│                      Query / Export API                              │
│  /state       → 当前状态 (SimplexCodec 编码, ~48 bytes)              │
│  /cascade     → 级联结果 (稀疏向量编码, ~800 bytes)                    │
│  /snapshot    → 全量快照 (批量编码, ~9KB for 183 entities)            │
│  /stream      → WebSocket/HTTP2 Server-Sent Events (实时 delta 流)   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 7. 性能基准目标

| 指标 | 目标 | 实现路径 |
|------|------|----------|
| 单实体 Delta 写入 | < 1μs | 无锁 CAS 写入环形缓冲区 |
| 单实体状态查询 | < 1μs | 基态 memcpy + 平均 <10 delta 回放 |
| 级联推理（100 活跃实体，5 跳） | < 100μs | SIMD SpMV + 子图剪枝 |
| 帕累托投影（4 切面，无冲突） | < 5μs | SIMD 并行投影 |
| 切面热加载（10,000 实体） | < 100ms | 并行初始化 + 后台矩阵重建 |
| 状态更新帧大小 | < 30 bytes | 自定义二进制，无字段标签 |
| 级联结果帧大小 | < 1KB | 稀疏向量编码 |
| 全量快照 | < 10KB | 183 实体 × 48 bytes = 8.8KB |
| 增量日志刷盘 | < 10ms / 批次 | 每 100ms 异步批量写 |
| 系统启动（冷启动） | < 500ms | 读取最新快照 + replay 增量环 |

---

## 8. 生产部署

### 8.1 部署拓扑

```
┌─────────────────────────────────────────────────────────────┐
│                    Load Balancer (L4)                       │
│              轮询 / 最少连接                                  │
└─────────────────────────────────────────────────────────────┘
              │
    ┌─────────┼─────────┐
    ▼         ▼         ▼
┌──────┐ ┌──────┐ ┌──────┐
│Node 1│ │Node 2│ │Node 3│   Σ⁴ Engine Cluster (3+ 节点)
│16GB  │ │16GB  │ │16GB  │   每个节点独立内存，无共享状态
│4core │ │4core │ │4core │   通过 gossip 协议同步实体元数据
└──────┘ └──────┘ └──────┘
    │         │         │
    └─────────┴─────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────┐
│              SSD Snapshot Store (RAID 0)                    │
│  顺序写日志 + 定期快照                                       │
│  恢复: 读取最新快照 → replay 日志                            │
└─────────────────────────────────────────────────────────────┘
```

### 8.2 容错设计

| 故障场景 | 处理策略 |
|----------|----------|
| 单节点崩溃 | 其他节点继续服务；重启节点从 SSD 恢复快照 |
| 网络分区 | 分区节点进入只读模式；恢复后合并增量日志 |
| SSD 写满 | 自动压缩旧日志；保留最近 30 天增量 |
| 内存不足 | LRU 驱逐冷实体到 SSD；查询时按需加载 |

---

## 9. 与 v3.1 的兼容性

| 组件 | v3.1 | v4.0 |
|------|------|------|
| 语言 | Python | Rust (热路径), Python (管理面) |
| 存储 | DuckDB + PostgreSQL | 内存常驻 + SSD 快照 |
| 时间精度 | 秒级 Unix 时间戳 | 微秒级 Unix 时间戳 |
| 传输 | HTTP JSON | HTTP/2 自定义二进制帧 |
| 切面扩展 | YAML 配置 + 重启加载 | 运行时热加载 |
| 计算延迟 | ~5ms | ~100μs |
| 实体容量 | ~1,000 | ~65,536 (预分配槽位) |

**迁移路径**：
1. 用 v3.1 的切面配置生成 v4.0 的初始状态
2. 导出 CSV → Rust 初始化脚本 → 内存 Entity Pool
3. 管理面（切面定义、约束配置）仍用 Python + gRPC
4. 计算面（级联推理、状态查询）全部 Rust

---

## 10. 下一步行动

1. **确认技术栈**：Rust（热路径）+ Python（管理面）是否可接受？还是全部 Rust？
2. **确认预分配槽位**：MAX_SLICES=16, MAX_ENDPOINTS=8 是否足够？未来 3-5 年的扩展需求是什么？
3. **确认传输协议**：自定义二进制帧 vs gRPC vs FlatBuffers，倾向哪个？
4. **确认部署模式**：单节点内存（简单）vs 多节点集群（高可用）？

确认后即可开始 Rust 核心引擎的 PoC 实现。
