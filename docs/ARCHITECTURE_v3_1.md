# Σ³-System v3.1：时间主轴化与可逆矩阵向量存储

> **版本**：v3.1（基于 v3.0 增量更新）  
> **核心变更**：时间从切面降维为公共主轴；秒级精度统一；可逆矩阵向量压缩存储  
> **设计约束**：切面不包含时间值；所有时间戳精确到秒；矩阵编码过程 100% 可逆

---

## 1. 变更摘要（v3.0 → v3.1）

| 变更项 | v3.0 状态 | v3.1 修正 |
|--------|-----------|-----------|
| **时间定位** | TEMPORAL 是第 2 个切面（4 端点） | 时间**不是切面**，是**公共主轴**；所有实体共享统一时间轴 |
| **存在模式** | 作为 TEMPORAL 切面端点（instant/endurance/recurrence/becoming） | 提取为独立切面 **DYNAMICS**（稳定/周期/瞬时/变革），与时间值解耦 |
| **时间精度** | 未定义精度协议 | 秒级 Unix 时间戳，人类输入兜底：年→01-01 00:00:00，月→01日 00:00:00，日→00:00:00 |
| **存储编码** | 抽象矩阵以 float32 直接存储 | 单纯形约束压缩（每行存 K_s−1 个值）+ 差分编码，过程 100% 可逆 |
| **级联协议** | 事件注入无显式时间上下文 | 级联在**指定时间戳 T** 执行，只加载 `[valid_from, valid_until)` 有效的实体；传播链含时间累积 |

---

## 2. 时间作为公共主轴

### 2.1 为什么时间不能是切面

**切面的定义**：切面是实体的**分析维度**，端点是该维度上的**极端理想型**。实体在切面上的坐标是**比例关系**（凸组合，和为 1）。

**时间的问题**：
- 时间是**绝对值**（1975年、2024-03-15 14:32:18），不是比例关系。把 1975 年映射为 [0, 0.8, 0.1, 0.1] 是**伪凸组合**——端点不是理想型，而是时间值本身。
- 不同实体的时间值之间没有可比性。Soros Fund 成立于 1969，BlackRock 成立于 1988，这两个数字的"距离"没有意义，除非放在历史上下文中。
- 时间需要参与**运算**（比较、区间查询、时滞累加），而单纯形坐标只参与**距离度量**。

**正确的定位**：时间是所有实体的**公共属性轴**（common axis），独立于切面系统。查询/级联/回测都在指定时间戳 `T` 上执行，系统返回该时刻的**实体快照**（snapshot）。

### 2.2 公共时间轴设计

```yaml
# 所有实体（ORG / EVENT / PERSON）共享的时间属性
TemporalAxis:
  # 实体存在区间
  valid_from:  int   # 秒级 Unix 时间戳，实体生效起始
  valid_until: int   # 秒级 Unix 时间戳，实体失效截止；null = 当前仍有效
  
  # 记录时间（审计）
  created_at:  int   # 该记录写入系统的时间
  updated_at:  int   # 该记录最后修改的时间
  
  # 精度标记（关键！用于不确定性传播）
  valid_from_precision: str   # "year" | "month" | "day" | "hour" | "minute" | "second"
  valid_until_precision: str  # 同上
  
  # 时间语义标签
  temporal_tags: [str]  # 如 ["founded", "merged", "dissolved", "restructured"]
```

**查询时的快照语义**：

```python
def snapshot_at(time: int) -> List[Entity]:
    """
    返回在时刻 T 有效的所有实体。
    条件: entity.valid_from <= T < entity.valid_until（或 valid_until is null）
    """
    return db.query(
        "SELECT * FROM entities WHERE valid_from <= ? AND (valid_until > ? OR valid_until IS NULL)",
        time, time
    )
```

**级联推理的时间上下文**：

```python
# 在 2024-02-24 00:00:00 UTC 注入"俄乌冲突升级"事件
event_time = 1708732800  # Unix timestamp

# 级联引擎在该时刻的上下文中执行
cascade_graph = engine.run(
    event=event,
    at_time=event_time,  # 公共主轴时间戳
    max_hops=5,
    theta=0.3
)
```

### 2.3 秒级时间戳与精度兜底协议

```python
from datetime import datetime, timezone
from typing import Optional, Tuple

class TemporalPrecision:
    """时间精度等级，从粗到细。"""
    YEAR   = "year"    # 只知道年份
    MONTH  = "month"   # 知道年月
    DAY    = "day"     # 知道年月日
    HOUR   = "hour"    # 知道到小时
    MINUTE = "minute"  # 知道到分钟
    SECOND = "second"  # 完整秒级

class TimestampNormalizer:
    """
    将人类可读的各种精度时间归一化为秒级 Unix 时间戳。
    
    兜底规则（缺失字段用最小值填充）：
    ┌─────────────────┬──────────────────────────┐
    │ 人类输入         │ 兜底后                   │
    ├─────────────────┼──────────────────────────┤
    │ 1969 年          │ 1969-01-01 00:00:00 UTC  │
    │ 1969 年 8 月     │ 1969-08-01 00:00:00 UTC  │
    │ 1969-08-17       │ 1969-08-17 00:00:00 UTC  │
    │ 1969-08-17 14:00 │ 1969-08-17 14:00:00 UTC  │
    │ 完整时间戳         │ 直接使用                 │
    └─────────────────┴──────────────────────────┘
    
    精度标记用于后续的不确定性传播：
    - "year" 精度 → 实体存在区间视为 [Y-01-01, Y+1-01-01)，区间内状态不确定
    - "month" 精度 → [Y-M-01, Y-M+1-01)，区间内状态不确定
    """
    
    @staticmethod
    def normalize(
        year: int,
        month: Optional[int] = None,
        day: Optional[int] = None,
        hour: Optional[int] = None,
        minute: Optional[int] = None,
        second: Optional[int] = None,
        precision_hint: Optional[str] = None,
        tz: timezone = timezone.utc
    ) -> Tuple[int, str]:
        """
        返回: (unix_timestamp_seconds, precision_level)
        
        precision_level 是系统推断或用户指定的精度等级。
        """
        # 兜底填充：缺失字段用最小值
        m = month if month is not None else 1
        d = day if day is not None else 1
        h = hour if hour is not None else 0
        mi = minute if minute is not None else 0
        s = second if second is not None else 0
        
        dt = datetime(year, m, d, h, mi, s, tzinfo=tz)
        timestamp = int(dt.timestamp())  # 秒级整数
        
        # 推断精度（如果用户未指定）
        if precision_hint:
            precision = precision_hint
        elif second is not None:
            precision = TemporalPrecision.SECOND
        elif minute is not None:
            precision = TemporalPrecision.MINUTE
        elif hour is not None:
            precision = TemporalPrecision.HOUR
        elif day is not None:
            precision = TemporalPrecision.DAY
        elif month is not None:
            precision = TemporalPrecision.MONTH
        else:
            precision = TemporalPrecision.YEAR
        
        return timestamp, precision
    
    @staticmethod
    def human_readable(timestamp: int, precision: str) -> str:
        """反向：时间戳 + 精度 → 人类可读字符串。"""
        dt = datetime.fromtimestamp(timestamp, tz=timezone.utc)
        fmt_map = {
            TemporalPrecision.YEAR:   "%Y",
            TemporalPrecision.MONTH:  "%Y-%m",
            TemporalPrecision.DAY:    "%Y-%m-%d",
            TemporalPrecision.HOUR:   "%Y-%m-%d %H:00",
            TemporalPrecision.MINUTE: "%Y-%m-%d %H:%M",
            TemporalPrecision.SECOND: "%Y-%m-%d %H:%M:%S",
        }
        return dt.strftime(fmt_map.get(precision, "%Y-%m-%d %H:%M:%S"))
```

**183 实体的典型时间精度分布**（预估）：

| 精度 | 实体数 | 示例 |
|------|--------|------|
| YEAR | ~30 | 家族/历史实体（如 Rothschild & Co "200年历史"） |
| MONTH | ~10 | 央企改制（如中国兵器工业集团 1999年7月改制） |
| DAY | ~80 | 上市公司 SEC 披露、基金创立 |
| SECOND | ~60 | 事件（如 2024-02-24T00:00:00Z） |

### 2.4 时间精度对级联推理的影响

当实体的 `valid_from_precision` 是 "year" 时，级联推理在该年内的任何时刻都面临**状态不确定性**：

```python
def state_at(entity: Entity, query_time: int) -> Optional[EntityState]:
    """
    获取实体在查询时刻的状态。
    处理精度不足的情况。
    """
    # 情况 1：实体存在区间完全确定
    if entity.valid_from <= query_time and (entity.valid_until is None or query_time < entity.valid_until):
        # 但精度可能不足
        if entity.valid_from_precision == TemporalPrecision.YEAR:
            # 只知道创立于某年，但不知道是年初还是年末
            # 在 query_time 位于该年内时，存在不确定性
            year_start = entity.valid_from
            year_end = year_start + 365 * 86400  # 近似
            if year_start <= query_time < year_end:
                # 标记为 UNCERTAIN，置信度降低
                return EntityState(
                    coordinates=entity.coordinates,
                    confidence=entity.base_confidence * 0.5,  # 精度不足惩罚
                    uncertainty_reason="valid_from precision is YEAR"
                )
        
        return EntityState(
            coordinates=entity.coordinates,
            confidence=entity.base_confidence,
            uncertainty_reason=None
        )
    
    # 情况 2：实体在该时刻不存在
    return None
```

**精度惩罚系数表**：

| valid_from 精度 | 当年内查询的置信度惩罚 | 说明 |
|-----------------|----------------------|------|
| YEAR | ×0.50 | 年内任何时刻都可能未成立 |
| MONTH | ×0.80 | 月内可能存在不确定性 |
| DAY | ×0.95 | 日内00:00前可能未成立 |
| HOUR/MINUTE/SECOND | ×1.00 | 无惩罚 |

---

## 3. 切面系统修订（3 切面 + 1 动态切面）

### 3.1 DYNAMICS 切面（原 TEMPORAL 的重定义）

原 TEMPORAL 切面的 4 个端点（instant/endurance/recurrence/becoming）被误解为"时间值"。修正后：

| 端点 | 符号 | 定义 | 量化指标 |
|------|------|------|----------|
| 稳定 | 𝓓₁ (Stable) | 实体状态在长时间内保持高度一致，变化速率极低 | 存在年限、资产负债表稳定性、策略变更频率 |
| 周期 | 𝓓₂ (Cyclic) | 实体行为呈现可预测的周期性重复模式 | 报告周期规律性、营收季节性变异、政策周期暴露 |
| 瞬时 | 𝓓₃ (Episodic) | 实体以离散事件的形式存在，单次操作后即消散 | 日均交易频次、事件驱动型操作占比、存在时长 < 1年 |
| 变革 | 𝓓₄ (Transformative) | 实体处于不可逆的范式转换过程中，旧状态正在被新状态替代 | 营收增长率、研发投入强度、新产品/业务占比 |

**关键修正**：DYNAMICS 描述的是实体**如何变化**（变化模式），不是**何时变化**（时间值）。时间值由公共时间轴管理。

**映射示例**：

```
Bridgewater Associates       → 𝓓(0.05, 0.15, 0.05, 0.75)  # 持存+变革（All Weather 策略迭代）
单次做空英镑（1992）         → 𝓓(0.90, 0.05, 0.05, 0.00)  # 极端瞬时
OPEC 产量决策                → 𝓓(0.10, 0.70, 0.10, 0.10)  # 周期主导
加密牛市（2024）             → 𝓓(0.10, 0.05, 0.10, 0.75)  # 变革主导
LTCM（1998 崩溃前）          → 𝓓(0.15, 0.30, 0.00, 0.55)  # 持存向变革滑落
COFCO 国家储备粮轮换          → 𝓓(0.20, 0.65, 0.10, 0.05)  # 周期+稳定
```

### 3.2 3+1 切面体系

| 编号 | 切面 ID | 端点数 | 描述 |
|------|---------|--------|------|
| 1 | `power` | 4 | 权力拓扑（主权/资本/生产/叙事） |
| 2 | `dynamics` | 4 | 动态模式（稳定/周期/瞬时/变革） |
| 3 | `epistemic` | 4 | 认知可达（黑箱/披露/推断/操纵） |
| 4 | `cascade` | 4 | 级联响应（弹性/塑性/脆性/吸收） |

**抽象矩阵维度**：`M_ξ ∈ ℝ^{4×4}`（4 切面 × 最多 4 端点）。

### 3.3 切面扩展协议（不变）

新增切面仍通过 `SliceRegistry` 热加载，不影响现有矩阵维度。如果新切面有 >4 个端点，`K_max` 自动扩展，旧矩阵 padding 补零。

---

## 4. 可逆矩阵向量存储

### 4.1 单纯形约束压缩（无损）

**核心洞察**：每个切面的坐标是单纯形上的点，满足 `Σαᵢ = 1`。因此每行只有 `K_s − 1` 个自由度。

**编码**：

```python
import numpy as np
from typing import List

class SimplexMatrixCodec:
    """
    将抽象矩阵 M ∈ ℝ^{S×K_max} 编码为紧凑字节流，过程 100% 可逆。
    
    压缩原理：
    - 每行有 K_s 个元素，但满足 Σαᵢ = 1
    - 只存储前 K_s−1 个元素（float32）
    - 第 K_s 个元素 = 1 − Σ(前 K_s−1)
    - 解码时恢复完整行
    
    压缩率：
    - K_s = 4 时，每行存 3 个 float32 = 12 bytes，原 16 bytes → 节省 25%
    - K_s = 3 时，每行存 2 个 float32 = 8 bytes，原 12 bytes → 节省 33%
    - S = 4, K_max = 4 时，原 64 bytes → 压缩后 48 bytes
    """
    
    DTYPE = np.float32
    BYTES_PER_ELEM = 4  # float32
    
    @classmethod
    def encode(
        cls,
        M: np.ndarray,
        slice_dims: List[int]
    ) -> bytes:
        """
        编码抽象矩阵为字节流。
        
        Args:
            M: 抽象矩阵，形状 (S, K_max)
            slice_dims: 每个切面的实际端点数 [K_0, K_1, ..., K_{S-1}]
        
        Returns:
            字节流，长度 = Σ(K_s − 1) × 4 bytes
        """
        assert M.shape[0] == len(slice_dims)
        
        parts = []
        for s, K_s in enumerate(slice_dims):
            # 验证行和 ≈ 1（数值容差）
            row_sum = M[s, :K_s].sum()
            assert abs(row_sum - 1.0) < 1e-5, f"Row {s} sum = {row_sum}, expected 1.0"
            
            # 存储前 K_s−1 个元素
            to_store = M[s, :K_s - 1].astype(cls.DTYPE)
            parts.append(to_store.tobytes())
        
        return b''.join(parts)
    
    @classmethod
    def decode(
        cls,
        data: bytes,
        slice_dims: List[int],
        K_max: int
    ) -> np.ndarray:
        """
        从字节流解码为完整抽象矩阵。
        
        Args:
            data: 编码后的字节流
            slice_dims: 每个切面的实际端点数
            K_max: 最大端点数（矩阵列数）
        
        Returns:
            完整抽象矩阵，形状 (S, K_max)
        """
        S = len(slice_dims)
        M = np.zeros((S, K_max), dtype=cls.DTYPE)
        
        offset = 0
        for s, K_s in enumerate(slice_dims):
            num_store = K_s - 1
            num_bytes = num_store * cls.BYTES_PER_ELEM
            
            # 读取前 K_s−1 个元素
            partial = np.frombuffer(data[offset:offset + num_bytes], dtype=cls.DTYPE)
            M[s, :num_store] = partial
            
            # 恢复第 K_s 个元素（单纯形约束）
            M[s, K_s - 1] = 1.0 - partial.sum()
            
            # 剩余列保持为 0（padding）
            offset += num_bytes
        
        return M
    
    @classmethod
    def compute_size(cls, slice_dims: List[int]) -> int:
        """计算编码后的字节数。"""
        return sum((K_s - 1) * cls.BYTES_PER_ELEM for K_s in slice_dims)
```

**示例**：

```python
# Soros Fund 的抽象矩阵
M = np.array([
    [0.10, 0.55, 0.00, 0.35],  # power: sovereignty=0.10, capital=0.55, production=0.00, narrative=0.35
    [0.05, 0.15, 0.05, 0.75],  # dynamics: stable=0.05, cyclic=0.15, episodic=0.05, transformative=0.75
    [0.30, 0.40, 0.15, 0.15],  # epistemic: opaque=0.30, disclosed=0.40, inferred=0.15, manipulated=0.15
    [0.20, 0.25, 0.35, 0.20],  # cascade: elastic=0.20, plastic=0.25, brittle=0.35, absorptive=0.20
], dtype=np.float32)

slice_dims = [4, 4, 4, 4]  # 每个切面 4 个端点

# 编码
encoded = SimplexMatrixCodec.encode(M, slice_dims)
print(f"编码前: {M.nbytes} bytes")      # 64 bytes
print(f"编码后: {len(encoded)} bytes")  # 48 bytes

# 解码（100% 可逆）
M_recovered = SimplexMatrixCodec.decode(encoded, slice_dims, K_max=4)
assert np.allclose(M, M_recovered)  # True
```

### 4.2 差分编码（时序版本间）

对于同一实体的时间序列版本，相邻版本的变化通常很小，可用**差分 + 变长编码**进一步压缩：

```python
class DeltaCodec:
    """
    时序版本间的差分编码。
    
    存储策略：
    - 基准版本（v0）：完整 SimplexMatrixCodec 编码
    - 后续版本（v1, v2, ...）：存储与前一版本的差分 Δ
    
    差分特性：
    - Δ 的幅值通常很小（< 0.1）
    - 可用更少的位表示（如 float16 或定点数）
    """
    
    @staticmethod
    def encode_delta(prev_M: np.ndarray, curr_M: np.ndarray, slice_dims: List[int]) -> bytes:
        delta = curr_M - prev_M
        # 对差分进行量化（float16 通常足够，误差 < 0.001）
        delta_q = delta.astype(np.float16)
        return delta_q.tobytes()
    
    @staticmethod
    def decode_delta(prev_M: np.ndarray, delta_data: bytes, shape: tuple) -> np.ndarray:
        delta = np.frombuffer(delta_data, dtype=np.float16).reshape(shape)
        return prev_M + delta.astype(np.float32)
```

**存储成本对比**（单实体，100 个时间版本）：

| 方案 | 单版本大小 | 100 版本总大小 | 特点 |
|------|-----------|---------------|------|
| 原始 float32 | 64 B | 6.4 KB | 简单，无压缩 |
| 单纯形压缩 | 48 B | 4.8 KB | 无损，每版本独立可解码 |
| 单纯形 + 差分 | 48 B + 32 B×99 | 3.2 KB | 基准版完整 + 差分，需顺序解码 |
| uint8 量化 | 16 B | 1.6 KB | 有损，误差 ~0.4%，严格说不可逆 |

**推荐**：生产环境使用「单纯形压缩 + 可选差分」。差分只用于高频更新的实体（如 EVENT），低频更新的实体（如 ORG 的年度坐标刷新）用独立压缩。

### 4.3 跨实体批量压缩（可选：低秩 SVD）

当实体数量 N 很大时，可以跨实体做低秩近似：

```python
def batch_compress(entities: List[Entity], rank: int = 8) -> Tuple[np.ndarray, np.ndarray]:
    """
    对所有实体的抽象矩阵做 SVD 低秩近似。
    
    Args:
        entities: N 个实体
        rank: 保留的奇异值数量
    
    Returns:
        U: (N, rank) 每个实体在隐空间的系数
        Vt: (rank, S×K_max) 基矩阵
    
    可逆性：严格说有损，但可控制误差 < 1%（rank=8 时）。
    若要求 100% 可逆，不使用此方案。
    """
    # 堆叠所有矩阵为 (N, S*K_max)
    X = np.stack([e.abstract_matrix.flatten() for e in entities])
    
    U, S_diag, Vt = np.linalg.svd(X, full_matrices=False)
    U_r = U[:, :rank]
    S_r = S_diag[:rank]
    Vt_r = Vt[:rank, :]
    
    # 每个实体存储 U_r[i, :]（rank 个 float32）
    # 基矩阵 Vt_r 全局存储
    coeffs = U_r * S_r  # (N, rank)
    
    return coeffs, Vt_r
```

**适用场景**：N > 10,000 时考虑；N = 183 时完全不必要。

### 4.4 行业实践：DuckDB + 内存映射

```python
import duckdb

class MatrixStore:
    """
    使用 DuckDB 存储编码后的抽象矩阵，支持高效查询。
    """
    
    def __init__(self, db_path: str):
        self.conn = duckdb.connect(db_path)
        self.conn.execute("""
            CREATE TABLE IF NOT EXISTS entity_matrices (
                entity_id TEXT PRIMARY KEY,
                entity_type TEXT,
                valid_from BIGINT,       -- 秒级 Unix 时间戳
                valid_until BIGINT,      -- null = 当前有效
                encoded_matrix BLOB,     -- SimplexMatrixCodec 编码后的字节流
                slice_dims INT[],        -- [4, 4, 4, 4]
                K_max INT,               -- 4
                created_at BIGINT
            )
        """)
        
        # 创建时间索引（支持快照查询）
        self.conn.execute("""
            CREATE INDEX IF NOT EXISTS idx_time 
            ON entity_matrices(valid_from, valid_until)
        """)
    
    def insert(self, entity: Entity):
        encoded = SimplexMatrixCodec.encode(
            entity.abstract_matrix,
            entity.slice_dims
        )
        self.conn.execute("""
            INSERT INTO entity_matrices 
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            entity.id,
            entity.type.value,
            entity.valid_from,
            entity.valid_until,
            encoded,
            entity.slice_dims,
            entity.K_max,
            entity.created_at
        ))
    
    def snapshot_at(self, time: int) -> List[Entity]:
        """查询在时刻 T 有效的所有实体。"""
        results = self.conn.execute("""
            SELECT entity_id, entity_type, encoded_matrix, slice_dims, K_max
            FROM entity_matrices
            WHERE valid_from <= ? 
              AND (valid_until > ? OR valid_until IS NULL)
        """, (time, time)).fetchall()
        
        entities = []
        for row in results:
            entity_id, entity_type, encoded, slice_dims, K_max = row
            M = SimplexMatrixCodec.decode(encoded, slice_dims, K_max)
            entities.append(Entity(
                id=entity_id,
                type=entity_type,
                abstract_matrix=M
            ))
        return entities
```

**性能指标**（183 实体）：

| 操作 | 目标延迟 | 实际估算 |
|------|----------|----------|
| 单实体编码 | — | ~5 μs |
| 单实体解码 | — | ~3 μs |
| 快照查询（183 实体） | < 10 ms | ~2 ms（DuckDB 内存模式） |
| 全量 KNN（Frobenius 距离） | < 100 ms | ~5 ms（numpy 向量化） |

---

## 5. 时序感知的级联推理协议

### 5.1 协议修订

```python
def cascade_at_time(
    event: Event,
    at_time: int,          # 公共主轴时间戳（秒级）
    max_hops: int,
    theta: float
) -> CascadeGraph:
    """
    在指定时刻 T 执行级联推理。
    
    关键步骤：
    1. 加载 T 时刻的实体快照
    2. 检查每个实体的状态确定性（精度惩罚）
    3. 标准级联展开，但传播链累积时间
    """
    
    # Step 1: 快照
    kg = entity_store.snapshot_at(at_time)
    
    # Step 2: 事件初始化
    queue = [(event, hop=0, conf=1.0, accumulated_lag=0)]
    
    while queue:
        node, hop, conf, lag = queue.pop()
        if hop >= max_hops or conf < theta:
            continue
        
        # Step 3: 状态确定性检查
        state = node.state_at(at_time + lag)
        if state is None:
            continue  # 实体在传播到达时已失效
        
        conf *= state.confidence  # 应用精度惩罚
        
        # Step 4: 标准级联展开（同 v3.0）
        for relation in node.relations_at(at_time + lag):
            next_node = relation.to
            next_conf = conf * relation.coupling_strength
            next_lag = lag + relation.time_lag_seconds
            
            # 检查时间有效性：传播到达时，目标实体是否仍有效？
            if not next_node.is_valid_at(at_time + next_lag):
                continue
            
            if next_conf >= theta:
                cascade_graph.add_edge(node, next_node, hop+1, next_conf, next_lag)
                queue.push((next_node, hop+1, next_conf, next_lag))
    
    return cascade_graph
```

### 5.2 时间累积与时滞建模

级联传播不是瞬时的。每条关系有 `time_lag_seconds`：

```yaml
Relation:
  time_lag_seconds: int  # 传导时滞，秒级
  time_lag_model: enum   # FIXED | PROPORTIONAL | LOGARITHMIC
```

**时滞模型**：

| 模型 | 公式 | 适用场景 |
|------|------|----------|
| FIXED | `lag = constant` | 政策公告到市场反应（固定 1-3 天） |
| PROPORTIONAL | `lag = base × impact_magnitude` | 冲击越大，反应越快（如闪崩） |
| LOGARITHMIC | `lag = base × log(1 + distance)` | 跨实体类型传播（个人→组织→市场） |

**传播链的时间线**：

```
T=0:        俄乌冲突升级（EVENT 注入）
T+86,400s:  油价跳涨（COMMODITY 市场响应，1天 lag）
T+259,200s: 通胀预期上升（FED 监测，3天 lag）
T+604,800s: A类基金调整仓位（宏观对冲响应，1周 lag）
T+2,592,000s: 主权债务 CDS 利差扩大（1月 lag）
```

级联图输出包含每条边的 `accumulated_lag`，可用于回答「冲击发生后多久会影响某实体」。

---

## 6. 实体 Schema 修订（v3.1 版）

```yaml
Entity:
  # === 身份 ===
  id: UUID
  type: enum            # ORG | EVENT | PERSON
  display_name: str
  aliases: [str]
  
  # === 公共时间轴（新增）===
  valid_from: int               # 秒级 Unix 时间戳
  valid_until: int              # null = 当前有效
  valid_from_precision: str     # "year" | "month" | "day" | ...
  valid_until_precision: str
  
  # === 切面坐标（4 切面）===
  slice_projections:
    power: [float]×4     # 主权/资本/生产/叙事
    dynamics: [float]×4  # 稳定/周期/瞬时/变革（原 TEMPORAL 重定义）
    epistemic: [float]×4 # 黑箱/披露/推断/操纵
    cascade: [float]×4   # 弹性/塑性/脆性/吸收
  
  slice_dims: [int]      # [4, 4, 4, 4]，支持切面扩展
  K_max: int             # 4
  
  # === 抽象矩阵（编码存储）===
  abstract_matrix: ndarray  # 4×4，运行时使用
  encoded_matrix: bytes     # SimplexMatrixCodec 编码后存储
  
  # === 约束与帕累托 ===
  constraint_set: [Constraint]
  pareto_frontier: [ndarray]  # 约束冲突时的前沿解集
  
  # === 元数据 ===
  is_chinese: bool
  data_source: str
  provenance: [str]
  created_at: int          # 秒级 Unix 时间戳
  updated_at: int          # 秒级 Unix 时间戳
  
  # === 类型特定 ===
  # ORG 特有
  classification: str      # A/B/C/D/E
  aum: Optional[float]
  headquarters: str
  
  # EVENT 特有
  event_type: str
  impact_magnitude: float
  is_boundary_breaking: bool  # 允许越界
  
  # PERSON 特有
  affiliations: [Affiliation]  # 时序化人事关联
```

---

## 7. 查询协议更新

### 7.1 时序查询模式

```python
class QueryEngine:
    def query(self, query: Query) -> QueryResult:
        if query.mode == QueryMode.SNAPSHOT:
            # 新增：时间快照查询
            return self._query_snapshot(
                entity_id=query.entity_id,
                at_time=query.at_time,  # 秒级时间戳
                slices=query.slices
            )
        
        elif query.mode == QueryMode.TIMELINE:
            # 新增：时间线查询（查看实体坐标随时间的变化）
            return self._query_timeline(
                entity_id=query.entity_id,
                time_range=(query.from_time, query.to_time),
                slice=query.slice
            )
        
        elif query.mode == QueryMode.CROSS_TIME:
            # 新增：跨时间对比（如 Soros Fund 1992 vs 2024）
            return self._query_cross_time(
                entity_id=query.entity_id,
                time_points=query.time_points,
                metric=query.metric  # "frobenius" | "jsd" | "cosine"
            )
        
        # v3.0 已有模式保持不变
        elif query.mode == QueryMode.SINGLE_SLICE:
            ...
        elif query.mode == QueryMode.ABSTRACT_MATRIX:
            ...
```

### 7.2 查询示例

```python
# 示例 1：快照——2024-02-24 00:00:00 UTC 的 Soros Fund 状态
result = engine.query(Query(
    mode=QueryMode.SNAPSHOT,
    entity_id="soros_fund_mgmt",
    at_time=1708732800,  # 2024-02-24T00:00:00Z
    slices=["power", "cascade"]
))

# 示例 2：时间线——Bridgewater 2018-2024 的 cascade 坐标变化
result = engine.query(Query(
    mode=QueryMode.TIMELINE,
    entity_id="bridgewater",
    from_time=1514764800,  # 2018-01-01
    to_time=1704067200,    # 2024-01-01
    slice="cascade"
))

# 示例 3：跨时间对比——Soros 1992 vs 2024（权力结构变迁）
result = engine.query(Query(
    mode=QueryMode.CROSS_TIME,
    entity_id="soros_fund_mgmt",
    time_points=[[709171200, 1708732800]],  # 1992-09 vs 2024-02
    metric="frobenius"
))
# 返回：两个时间点的矩阵距离 + 哪个端点变化最大
```

---

## 8. 开发路线图（v3.1 修订）

| Phase | 周数 | 目标 | 验收标准 |
|-------|------|------|----------|
| **P0** | 1 | 时间戳归一化 + Observable 向量化 | 183 实体全部有秒级 `valid_from`（精度标记）；CSV 7 列映射到 observable_vector |
| **P1** | 2 | SliceRegistry（4 切面）+ 独立计算 | 切面可热加载；DYNAMICS 切面（原 TEMPORAL 重定义） |
| **P2** | 3 | 抽象矩阵 + SimplexCodec | 编码/解码单元测试通过；183 实体 × 4×4 矩阵生成；压缩率 ≥ 25% |
| **P3** | 4 | 帕累托求解 + 中国主体约束 | 约束冲突时返回前沿解集；中投/中金等中国实体坐标通过约束验证 |
| **P4** | 5-6 | 时序快照 + 级联引擎 | `snapshot_at(T)` 查询 < 5ms；级联推理在 3 个历史时间点上通过单元测试 |
| **P5** | 7-8 | 跨类型统一（EVENT/PERSON） | EVENT 瞬时注入 + 时滞传播；PERSON 人事变动权重切换 |
| **P6** | 9-10 | 回测验证 | 10+ 历史事件回测；Precision@10 > 0.6；时间精度惩罚机制验证 |
| **P7** | 11-12 | 生产部署 | API + DuckDB + K8s；P99 延迟达标；矩阵编码/解码性能基准测试 |

---

## 9. 附录

### 9.1 时间戳转换速查表

| 实体 | 人类输入 | 归一化时间戳 | 精度 |
|------|----------|-------------|------|
| Soros Fund Management | "1969年" | -31536000 | YEAR |
| Bridgewater Associates | "1975年" | 157766400 | YEAR |
| 中投公司 | "2007年9月29日" | 1191014400 | DAY |
| 敦和资管 | 需调研 | TBD | TBD |
| 俄乌冲突升级 | "2022-02-24T00:00:00Z" | 1645660800 | SECOND |
| 1992英镑危机 | "1992-09-16" | 716342400 | DAY |

### 9.2 编码效率对比（183 实体，4 切面 × 4 端点）

| 方案 | 单实体 | 183 实体 | 1000 版本 | 特点 |
|------|--------|----------|-----------|------|
| float32 原始 | 64 B | 11.7 KB | 11.7 MB | 基准 |
| SimplexCodec | 48 B | 8.8 KB | 8.8 MB | 无损，节省 25% |
| Simplex + Delta | ~35 B | 6.4 KB | 3.2 MB | 基准+差分，需顺序解码 |
| uint8 量化 | 16 B | 2.9 KB | 2.9 MB | 有损，不推荐 |
| SVD rank=8 | 32 B | 5.9 KB | 5.9 MB | 有损，N>10000 时启用 |

### 9.3 与 v3.0 的兼容性

- `SliceRegistry` API 不变，切面热加载协议不变
- `ParetoSolver` API 不变
- `QueryMode.SINGLE_SLICE` / `MULTI_SLICE` / `ABSTRACT_MATRIX` / `PARETO_FRONTIER` / `CROSS_TYPE` 全部保留
- 新增 `SNAPSHOT` / `TIMELINE` / `CROSS_TIME` 查询模式
- `Entity.valid_from` 从 `date` 类型改为 `int`（秒级 Unix 时间戳）
- `Entity.slice_projections` 中 `temporal` 键更名为 `dynamics`

---

*本文档为 v3.1 增量更新。核心系统（帕累托求解、跨类型级联、查询协议）与 v3.0 保持一致，变更集中在时间主轴化、精度协议、可逆存储三个维度。*
