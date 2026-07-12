# Σ-System：多维单纯形级联推理架构 v2.0

> **版本**：v2.0  
> **核心抽象**：四维单纯形空间 Σ = 𝓟 × 𝓣 × 𝓔 × 𝓒  
> **设计约束**：拒绝扁平标签，所有实体必须在至少 3 个切面上有凸组合坐标  
> **数学基础**：(k−1)-单形上的凸分析、跨切面耦合矩阵、冲击向量的确定性传播

---

## 1. 设计原则：为什么必须是单纯形

上一版 Schema 的缺陷是**把世界压扁了**——实体被表示为一组键值对，关系被表示为带权重的边。这在实体数 < 30 时可用，一旦超过 MEME 基准的崩溃阈值，系统会退化为「带权图遍历 + 启发式剪枝」，本质上仍是黑盒工程。

**正确的数学起点**：每个实体不是图中的一个「节点」，而是一个在多维状态空间中的**形状**。这个空间的每个维度是一个「切面」（Slice），切面的极端状态由「端点」（Endpoint）定义。实体的身份 = 它在各切面上的**单纯形坐标**。

> **定义 1.1（切面）**：切面 𝓢 是一个 (k−1)-单形，由 k 个端点 {𝓢₁, 𝓢₂, ..., 𝓢ₖ} 张成。任何实体 e 在 𝓢 上的投影是一个凸组合：
> 
> **π𝓢(e) = Σᵢ₌₁ᵏ αᵢ · 𝓢ᵢ，其中 Σαᵢ = 1，αᵢ ≥ 0**
> 
> 坐标向量 (α₁, α₂, ..., αₖ) 称为 e 在切面 𝓢 上的**单纯形坐标**。

**为什么凸组合？**因为端点之间不是正交基，而是**极端理想型**。没有一个实体是「纯主权」或「纯资本」，但每个实体可以被精确地定位在极端之间的某个比例位置。这避免了向量空间假设（用户之前纠正过的隐式假设）。

---

## 2. 四个切面定义

本系统定义 **4 个切面**，每个切面由 **4 个端点** 张成（3-单形）。4 个端点构成四面体，比三角形多一个自由度，能容纳更丰富的拓扑结构（如「混合态」与「中立带」）。

### 2.1 切面 𝓟：权力拓扑（Power Topology）

**问题：这个实体靠什么影响世界？**

| 端点 | 符号 | 定义 | 极端理想型 |
|------|------|------|-----------|
| 主权 | 𝓟₁ (Sovereignty) | 垄断合法暴力的权力，或由国家法权背书的强制力 | 国家、央行、军队、法律体系 |
| 资本 | 𝓟₂ (Capital) | 通过金融定价权、资本配置、杠杆放大来重构资源流向 | 对冲基金、主权基金、投行、PE |
| 生产 | 𝓟₃ (Production) | 控制物理世界的物质转化与供应链节点 | 军工、能源、粮商、矿产、制造 |
| 叙事 | 𝓟₄ (Narrative) | 控制符号、意义、注意力与集体信念的生产 | 媒体、智库、NGO、社交平台、宗教 |

**映射示例**（从表格数据推导）：

```
Soros Fund Management        → (0.10, 0.55, 0.00, 0.35)  # 资本为主，叙事为辅
Open Society Foundations     → (0.05, 0.20, 0.00, 0.75)  # 叙事主导
Lockheed Martin              → (0.45, 0.10, 0.45, 0.00)  # 主权+生产双核
Rheinmetall                  → (0.35, 0.05, 0.60, 0.00)  # 生产为主，主权背书
中国兵器工业集团             → (0.80, 0.00, 0.20, 0.00)  # 纯主权-生产混合
Murdoch家族 / Fox Corp       → (0.10, 0.25, 0.00, 0.65)  # 叙事主导，资本辅助
BlackRock                    → (0.05, 0.70, 0.05, 0.20)  # 资本绝对主导
Koch Industries              → (0.15, 0.30, 0.45, 0.10)  # 生产+资本，主权游说
```

**关键洞察**：
- 同一「家族」内的不同实体可以坐标完全不同（Soros Fund vs OSF）
- 中国央企与欧美同行在同一产业中，权力来源不同（主权占比高 vs 资本/生产占比高）
- 「叙事」端点是欧美资本独有的自由度，中国主体在该端点的坐标天然偏低（行为模板覆盖）

---

### 2.2 切面 𝓣：时间存在（Temporal Ontology）

**问题：这个实体在时间中以什么模式存在？**

| 端点 | 符号 | 定义 | 极端理想型 |
|------|------|------|-----------|
| 瞬时 | 𝓣₁ (Instant) | 零维时间事件，无前后延展 | 单笔交易、一次做空、一次攻击 |
| 持存 | 𝓣₂ (Endurance) | 在一段时间内保持同一性，可累积记忆 | 企业、机构、国家、家族王朝 |
| 循环 | 𝓣₃ (Recurrence) | 周期性重复，前后相似但不相同 | OPEC 月度会议、财报季、选举周期、收获季 |
| 生成 | 𝓣₄ (Becoming) | 处于从一种状态向另一种状态不可逆转变的过程中 | 泡沫膨胀、政权更迭、技术范式转移、战争升级 |

**映射示例**：

```
Bridgewater Associates       → (0.05, 0.90, 0.05, 0.00)  # 持存机构
单次做空英镑（1992）         → (0.95, 0.05, 0.00, 0.00)  # 瞬时交易
OPEC 产量决策                → (0.10, 0.15, 0.70, 0.05)  # 循环为主
2024 加密牛市                → (0.15, 0.00, 0.10, 0.75)  # 生成中（泡沫/范式）
LTCM（1998 崩溃前）          → (0.20, 0.30, 0.00, 0.50)  # 持存向生成/瞬时滑落
COFCO 国家储备粮轮换         → (0.05, 0.30, 0.60, 0.05)  # 循环+持存
```

**时间维度的级联意义**：
- 瞬时事件（𝓣₁）的冲击传播快但衰减慢（如闪崩）
- 持存实体（𝓣₂）有惯性，响应有延迟但持续时间长
- 循环节点（𝓣₃）是**共振放大器**——当外部冲击频率与循环周期匹配时，级联被放大
- 生成节点（𝓣₄）是**相变预警器**——当大量实体从持存滑向生成时，系统接近临界点

---

### 2.3 切面 𝓔：认知可达（Epistemic Accessibility）

**问题：作为观测者，我们能在多大程度上知道这个实体在做什么？**

| 端点 | 符号 | 定义 | 极端理想型 |
|------|------|------|-----------|
| 黑箱 | 𝓔₁ (Opaque) | 无结构性披露，所有信息来自间接推断或泄漏 | 家族办公室、私有公司、离岸实体、部分主权基金 |
| 披露 | 𝓔₂ (Disclosed) | 按规范定期披露，数据可被独立验证 | 上市公司（SEC 13F/10-K）、上市银行、监管备案私募 |
| 推断 | 𝓔₃ (Inferred) | 不直接披露，但行为痕迹可通过关联分析重建 | 宏观基金的期货持仓（从交易所数据推断）、航运路线（AIS） |
| 操纵 | 𝓔₄ (Manipulated) | 主动生产信息环境，混淆信号与噪声 | 做空机构报告、政治叙事操作、媒体议程设置、社交媒体水军 |

**映射示例**：

```
Renaissance Technologies     → (0.85, 0.10, 0.05, 0.00)  # 极端黑箱
Citadel (SEC 13F + 采访)     → (0.20, 0.60, 0.15, 0.05)  # 披露为主
敦和资管（期货持仓可推断）    → (0.30, 0.20, 0.45, 0.05)  # 推断为主
Hindenburg Research          → (0.10, 0.15, 0.20, 0.55)  # 操纵+推断
中国航天科技集团              → (0.40, 0.35, 0.20, 0.05)  # 半黑箱+部分披露
Soros OSF（资助透明）         → (0.15, 0.50, 0.20, 0.15)  # 披露为主，但有叙事操纵成分
```

**认知维度与权力维度的耦合**：
- 高 𝓔₁（黑箱）+ 高 𝓟₂（资本）= **隐蔽的系统性风险**（如 Archegos 的 TRS 头寸）
- 高 𝓔₄（操纵）+ 高 𝓟₄（叙事）= **信息战节点**（如 OSF 资助媒体网络）
- 高 𝓔₂（披露）+ 高 𝓟₁（主权）= **可预期的政策工具**（如美联储利率决议）

---

### 2.4 切面 𝓒：级联响应（Cascade Response）

**问题：当外部冲击到达时，这个实体以什么模式响应？**

| 端点 | 符号 | 定义 | 极端理想型 |
|------|------|------|-----------|
| 弹性 | 𝓒₁ (Elastic) | 冲击后快速恢复原状，损耗可逆 | Citadel（多策略快速再平衡）、高频做市商 |
| 塑性 | 𝓒₂ (Plastic) | 冲击后发生不可逆形变，但不崩溃 | 传统制造业向新能源转型、银行重组 |
| 脆性 | 𝓒₃ (Brittle) | 存在明确阈值，超过即突然断裂 | LTCM（1998）、Archegos（2021）、雷曼（2008） |
| 吸收 | 𝓒₄ (Absorptive) | 将冲击内部化，延迟释放或不释放 | 主权基金（中投）、央行、长期养老金 |

**映射示例**：

```
Citadel (多策略平台)          → (0.75, 0.15, 0.05, 0.05)  # 极弹性
Rokos Capital (激进杠杆)       → (0.35, 0.20, 0.40, 0.05)  # 弹性向脆性靠近
Bridgewater (风险平价)         → (0.40, 0.25, 0.10, 0.25)  # 弹性+吸收混合
中国投资有限责任公司          → (0.10, 0.20, 0.05, 0.65)  # 吸收为主
Hayman Capital (集中做空)      → (0.30, 0.15, 0.45, 0.10)  # 高脆性（集中敞口）
传统家族办公室                 → (0.25, 0.30, 0.10, 0.35)  # 塑性+吸收（长期持有）
```

---

## 3. 实体张量表示

### 3.1 形式化定义

**定义 3.1（实体状态）**：实体 E 的状态是一个四元组：

> **State(E) = (p, t, e, c) ∈ 𝓟 × 𝓣 × 𝓔 × 𝓒**
>
> 其中：
> - p = (p₁, p₂, p₃, p₄) ∈ Δ³，Σpᵢ = 1，pᵢ ≥ 0
> - t = (t₁, t₂, t₃, t₄) ∈ Δ³，Σtᵢ = 1，tᵢ ≥ 0
> - e = (e₁, e₂, e₃, e₄) ∈ Δ³，Σeᵢ = 1，eᵢ ≥ 0
> - c = (c₁, c₂, c₃, c₄) ∈ Δ³，Σcᵢ = 1，cᵢ ≥ 0
>
> Δ³ 表示 3-单形（标准单纯形）。

**定义 3.2（实体距离）**：两个实体 Eₐ 和 Eᵦ 在切面 𝓢 上的距离使用 Jensen-Shannon 散度（对称、平滑、有界）：

> **D𝓢(Eₐ, Eᵦ) = √[½ · KL(π𝓢(Eₐ) || M) + ½ · KL(π𝓢(Eᵦ) || M)]**
>
> 其中 M = ½(π𝓢(Eₐ) + π𝓢(Eᵦ))，KL 为 Kullback-Leibler 散度。

**定义 3.3（综合距离）**：跨切面加权距离：

> **D(Eₐ, Eᵦ) = Σ𝓢 w𝓢 · D𝓢(Eₐ, Eᵦ)**
>
> 权重 w𝓢 由当前事件类型决定（见 §5.2）。

### 3.2 中国主体的坐标覆盖

中国主体不单独建 Schema，而是通过**切面坐标的硬性约束**实现差异化：

```yaml
ChineseEntityConstraint:
  # 在权力拓扑切面上，叙事端点有上限
  power_topology:
    narrative_max: 0.25   # 𝓟₄ ≤ 0.25
    sovereignty_min: 0.30 # 𝓟₁ ≥ 0.30（国企/主权基金）
  
  # 在认知可达切面上，披露端点有下限（监管要求）
  epistemic_access:
    disclosed_min: 0.20   # 𝓔₂ ≥ 0.20
  
  # 在级联响应切面上，脆性端点有上限（政策维稳目标）
  cascade_response:
    brittle_max: 0.30     # 𝓒₃ ≤ 0.30
```

这避免了为中美各建一套 Schema 的碎片化，同时保留了**同一数学空间内的可比性**。

---

## 4. 关系：跨切面耦合矩阵

### 4.1 关系的本质不再是「边」

传统图数据库中，关系是 `(from, to, weight)`。在本系统中，关系是**两个实体状态之间的跨切面响应映射**。

**定义 4.1（关系算子）**：从实体 E₁ 到 E₂ 的关系 R 是一个算子：

> **R: Δ³ × Δ³ → [0, 1]**
>
> 具体地，R 由四个 4×4 耦合矩阵定义：
> - **M𝓟𝓟**：E₁ 的权力坐标如何影响 E₂ 的权力坐标
> - **M𝓣𝓣**：E₁ 的时间坐标如何影响 E₂ 的时间坐标
> - **M𝓔𝓒**：E₁ 的认知坐标如何影响 E₂ 的响应坐标（信息冲击 → 行为响应）
> - **M𝓟𝓒**：E₁ 的权力坐标如何影响 E₂ 的响应坐标（权力压制 → 强制响应）

**为什么不是完整的 4×4 矩阵？**因为某些切面之间不存在直接因果（如 𝓔→𝓣：认知状态不会直接改变时间存在模式），矩阵稀疏性是剪枝的自然来源。

### 4.2 关系类型与耦合矩阵模板

| 关系类型 | 激活矩阵 | 物理意义 |
|----------|----------|----------|
| `ORGANIZATIONAL` | M𝓟𝓟 = diag(0.95, 0.95, 0.95, 0.95) | 同组织内权力结构高度同步 |
| `PERSONNEL` | M𝓟𝓟 = I（单位阵） | 人事变动导致权力坐标复制 |
| `CAUSAL_MARKET` | M𝓟𝓟 + M𝓒𝓒 | 市场价格冲击权力结构+响应模式 |
| `CAUSAL_FINANCIAL` | M𝓟𝓒 | 资金链断裂直接触发响应模式切换 |
| `NARRATIVE` | M𝓔𝓒 | 信息操纵改变目标实体的响应行为 |
| `COMPETITIVE` | M𝓟𝓟 = -I（负耦合） | 此消彼长 |

### 4.3 极大权重机制的形式化

组织强绑定不是简单的 weight = 0.99，而是**耦合矩阵的范数约束**：

> 若 E₁ 和 E₂ 的 `ORGANIZATIONAL` 关系激活，则：
> **||M𝓟𝓟||₂ ≥ 0.95**
>
> 这意味着 E₁ 的权力坐标变化，E₂ 的权力坐标在 1 跳内跟随变化的比例 ≥ 95%。

人事变动的时序处理：
- George Soros 在 `affiliation.from = 1969` 到 `to = 2023` 期间，与 Soros Fund 的 M𝓟𝓟 = I
- Alex Soros 在 `from = 2023` 起，M𝓟𝓟 切换为 I，George 的 M𝓟𝓟 衰减为 0

---

## 5. 事件与级联推理

### 5.1 事件作为冲击向量

**定义 5.1（冲击向量）**：事件 ℰ 是一个在四维空间中的冲击向量：

> **Impact(ℰ) = (δp, δt, δe, δc)**
>
> 其中每个分量是所在切面上的**非归一化偏移向量**（不要求和为1，允许极端冲击打破单纯形边界）。

**示例：「俄乌冲突全面升级」**

```
δp = (+0.60, -0.10, +0.20, +0.30)  # 主权暴力↑，资本避险↓，生产中断↑，叙事战争↑
δt = (+0.40, +0.10, -0.20, +0.70)  # 瞬时攻击↑，循环被打破（制裁），生成（战争升级趋势）↑
δe = (+0.30, -0.20, +0.40, +0.50)  # 黑箱增加（情报战），披露减少，推断和操纵增加
δc = (-0.20, +0.10, +0.60, +0.30)  # 弹性下降，脆性暴露（能源依赖国），吸收尝试（储备释放）
```

### 5.2 事件-切面权重映射

不同类型的事件对不同切面的冲击权重不同：

| 事件类型 | w𝓟 | w𝓣 | w𝓔 | w𝓒 | 说明 |
|----------|-----|-----|-----|-----|------|
| GEOPOLITICAL | 0.50 | 0.20 | 0.15 | 0.15 | 权力切面主导 |
| MONETARY | 0.35 | 0.10 | 0.20 | 0.35 | 资本+响应双主导 |
| COMMODITY | 0.25 | 0.30 | 0.20 | 0.25 | 生产+时间（季节性） |
| CORPORATE | 0.20 | 0.15 | 0.40 | 0.25 | 认知切面主导（信息披露） |
| REGULATORY | 0.45 | 0.25 | 0.20 | 0.10 | 主权+时间（政策周期） |

### 5.3 级联传播算法（确定性白盒）

```
算法: Σ-CASCADE
输入: 初始事件 ℰ, 初始实体集 S₀, 最大跳数 N_max, 阈值 θ
输出: 级联图 G = (V, E, {hop, confidence, lag})

1. 将 ℰ 分解为冲击向量 Impact(ℰ)
2. 确定事件类型 → 权重向量 w = (w𝓟, w𝓣, w𝓔, w𝓒)
3. 计算加权冲击: Impact_w = w ⊙ Impact(ℰ)  # Hadamard 积

4. 初始化:
   V ← S₀
   queue ← [(s, hop=0, conf=1.0, lag=0) for s in S₀]

5. While queue ≠ ∅:
   a. (v, hop, conf, lag) ← Dequeue(queue)
   b. If hop ≥ N_max or conf < θ: Continue
   
   c. # 计算 v 对冲击的敏感度
      sensitivity = ⟨State(v), Impact_w⟩  # 内积
      # 注：内积在单纯形上的几何意义 = 冲击方向与实体状态的夹角余弦
   
   d. # 获取 v 的所有出边关系 R(v, u)
      For each R in RelationsFrom(v):
         i.   # 时间过滤
              If ℰ.occurred_at ∉ [R.valid_from, R.valid_until]: Skip
         
         ii.  # 计算跨切面响应
              response_vector = R.coupling_matrix · Impact_w
              response_magnitude = ||response_vector||₂
         
         iii. # 计算下一跳置信度
              next_conf = conf × sensitivity × response_magnitude × decay(hop, v.entity_type)
         
         iv.  # 计算时滞
              next_lag = lag + R.time_lag × f(v.temporal_ontology)
              # f(Instant) = 1, f(Endurance) = 3, f(Recurrence) = 0.5, f(Becoming) = 2
         
         v.   # 剪枝：Schema 驱动的响应模式匹配
              If v.cascade_response 以 Brittle 为主 (c₃ > 0.5):
                 # 脆性实体只在阈值被突破时响应
                 If response_magnitude < v.brittle_threshold: Skip
              
              If v.cascade_response 以 Absorptive 为主 (c₄ > 0.5):
                 # 吸收性实体延迟响应，但可能累积
                 next_conf *= 0.7  # 置信度衰减，但不过滤
         
         vi.  If next_conf ≥ θ:
              AddEdge(G, v, u, hop+1, next_conf, next_lag)
              Enqueue(queue, (u, hop+1, next_conf, next_lag))

6. Return G
```

### 5.4 置信度衰减的数学形式

```
decay(hop, entity_type) = λ(entity_type) ^ hop

其中:
λ(A) = 0.92   # 宏观基金：跨市场传导能力强
λ(B) = 0.85   # 军工：政策周期传导确定
λ(C) = 0.88   # 大宗：实物供应链传导
λ(D) = 0.75   # 家族/政治：人事变动不确定性高
λ(E) = 0.70   # 其他：信息不透明

对于脆性实体（c₃ > 0.5）:
  若 response_magnitude ≥ brittle_threshold:
    decay(hop, Brittle) = 1.0  # 阈值突破后，脆性实体以全置信度断裂传导
  Else:
    decay(hop, Brittle) = 0.5 ^ hop  # 未突破时快速衰减
```

---

## 6. 与表格数据的映射协议

### 6.1 从 7 列表头到四维坐标的推导规则

原始表格的 7 列不是 Schema 字段，而是**特征提取的输入**。通过规则引擎将文本映射到单纯形坐标：

```yaml
MappingRules:
  # 列 "分类" → 权力拓扑的强先验
  classification_to_power:
    "A. 宏观对冲基金":
      prior: [0.05, 0.70, 0.05, 0.20]  # 资本主导
      constraint: "narrative ≥ 0.10 if 存在 OSF/Narrative 关键词"
    
    "B. 军工防务复合体":
      prior: [0.40, 0.10, 0.50, 0.00]  # 主权+生产
      constraint: "sovereignty ≥ 0.30"
    
    "C. 能源资源大宗商品":
      prior: [0.10, 0.20, 0.65, 0.05]  # 生产主导
      constraint: "production ≥ 0.50"
    
    "D. 政治游说/家族王朝":
      prior: [0.20, 0.25, 0.05, 0.50]  # 叙事主导
      constraint: "narrative ≥ 0.30"
  
  # 列 "获利方式" → 级联响应的强先验
  payoff_to_response:
    关键词 "高杠杆做空":
      shift: [+0.20, 0, +0.30, -0.10]  # 向 Brittle 移动
    关键词 "长期持有/价值":
      shift: [-0.10, +0.20, -0.10, +0.20]  # 向 Plastic + Absorptive 移动
    关键词 "多策略/分散":
      shift: [+0.30, 0, -0.20, -0.10]  # 向 Elastic 移动
    关键词 "国家军费订单/主权财富":
      shift: [-0.20, 0, -0.10, +0.50]  # 向 Absorptive 移动
  
  # 列 "公开追踪渠道" → 认知可达的强先验
  observable_to_epistemic:
    "SEC 13F/10-K":
      prior: [0.05, 0.85, 0.10, 0.00]  # 披露为主
    "私募排排网/期货持仓":
      prior: [0.20, 0.15, 0.60, 0.05]  # 推断为主
    "行业报道/有限公开":
      prior: [0.40, 0.20, 0.30, 0.10]  # 黑箱+推断
    "社交媒体/公开演讲/叙事":
      prior: [0.10, 0.20, 0.15, 0.55]  # 操纵为主
  
  # 列 "创立时间/历史" → 时间存在的调整
  history_to_temporal:
    "已终结/历史":
      override: [0, 0, 0, 0]  # 特殊标记，不参与活跃级联
    ">100 年历史":
      shift: [0, +0.15, +0.10, 0]  # 持存+循环增强
    "<10 年/加密/Web3":
      shift: [+0.10, -0.10, 0, +0.15]  # 瞬时+生成增强
```

### 6.2 坐标归一化

所有先验和 shift 操作后，必须将坐标投影回标准单纯形：

```python
def project_to_simplex(v: np.ndarray) -> np.ndarray:
    """
    将向量投影到标准单纯形（非负 + 和为1）。
    使用 Duchi et al. (2008) 的 O(n) 算法。
    """
    u = np.sort(v)[::-1]
    cssv = np.cumsum(u)
    rho = np.where(u * np.arange(1, len(u)+1) > (cssv - 1))[0][-1]
    theta = (cssv[rho] - 1) / (rho + 1)
    return np.maximum(v - theta, 0)
```

---

## 7. 时序版本管理

### 7.1 切面坐标的时序演变

实体状态不是静态的。`VersionedFactStore` 管理的不是键值对，而是**切面坐标的时序序列**：

```yaml
TemporalCoordinate:
  entity_id: string
  slice: enum          # POWER | TEMPORAL | EPISTEMIC | CASCADE
  
  coordinates: [float]×4  # 单纯形坐标 (α₁, α₂, α₃, α₄)
  
  valid_from: datetime
  valid_until: datetime
  
  # 变化原因
  trigger_event: string   # 如 "ceo_change", "merger", "regulatory_shift"
  
  # 置信度
  confidence: float
  derivation: string      # 坐标推导的来源规则
```

### 7.2 回测协议（形式化）

```python
def backtest(event: Event, expected: Set[Entity], at_time: datetime) -> Metrics:
    """
    在历史时间 at_time 构建知识图谱快照，注入事件，与预期结果对比。
    """
    # 1. 快照：查询 at_time 时刻所有实体的最新坐标
    kg = fact_store.snapshot(at_time)
    
    # 2. 运行级联
    predicted = cascade_engine.run(event, kg, max_hops=5, theta=0.3)
    
    # 3. 计算指标
    precision = len(predicted ∩ expected) / len(predicted)
    recall = len(predicted ∩ expected) / len(expected)
    
    # 4. 跳数误差
    hop_errors = []
    for e in expected:
        if e in predicted:
            pred_hop = predicted.hop_of(e)
            actual_hop = expected.hop_of(e)  # 人工标注
            hop_errors.append(abs(pred_hop - actual_hop))
    
    return Metrics(precision, recall, np.mean(hop_errors))
```

---

## 8. 生产实现路径

### 8.1 存储层：单纯形数据库

不直接使用 PostgreSQL/Neo4j 的扁平表，而是设计一个**单纯形原生存储**：

```sql
-- 实体主表
CREATE TABLE entities (
    id TEXT PRIMARY KEY,
    display_name TEXT,
    entity_type CHAR(1),
    is_chinese BOOLEAN,
    
    -- 当前坐标（最新版本）
    power_coord FLOAT[4] CHECK (array_sum(power_coord) = 1.0 AND forall(x, x >= 0)),
    temporal_coord FLOAT[4] CHECK (array_sum(temporal_coord) = 1.0),
    epistemic_coord FLOAT[4] CHECK (array_sum(epistemic_coord) = 1.0),
    cascade_coord FLOAT[4] CHECK (array_sum(cascade_coord) = 1.0),
    
    -- 约束（中国主体）
    CONSTRAINT chinese_narrative_cap 
        CHECK (NOT is_chinese OR power_coord[4] <= 0.25)
);

-- 坐标历史版本
CREATE TABLE coordinate_history (
    id UUID PRIMARY KEY,
    entity_id TEXT REFERENCES entities(id),
    slice_type SMALLINT,  -- 0=P, 1=T, 2=E, 3=C
    coordinates FLOAT[4],
    valid_from TIMESTAMPTZ,
    valid_until TIMESTAMPTZ,
    trigger_event TEXT
);

-- 关系（耦合矩阵存储为 JSONB）
CREATE TABLE relations (
    id TEXT PRIMARY KEY,
    from_entity TEXT REFERENCES entities(id),
    to_entity TEXT REFERENCES entities(id),
    relation_type SMALLINT,
    
    -- 耦合矩阵（稀疏存储）
    coupling_matrix JSONB,  -- {"PP": [[...]], "EC": [[...]]}
    
    weight FLOAT,
    time_lag INTERVAL,
    valid_from TIMESTAMPTZ,
    valid_until TIMESTAMPTZ
);
```

### 8.2 计算层：级联引擎

```python
class SigmaCascadeEngine:
    def __init__(self, entity_registry, relation_graph, config):
        self.registry = entity_registry
        self.graph = relation_graph
        self.decay_params = config.decay_params
        self.theta = config.confidence_threshold
    
    def run(self, event: Event, snapshot_time: datetime = None) -> CascadeGraph:
        # 1. 加载快照
        if snapshot_time:
            kg = self.registry.snapshot_at(snapshot_time)
        else:
            kg = self.registry.current()
        
        # 2. 分解冲击向量
        impact = self.decompose_impact(event)
        weights = self.event_type_weights[event.type]
        weighted_impact = self.hadamard(impact, weights)
        
        # 3. BFS 级联展开
        return self._cascade_bfs(
            initial=event.direct_targets,
            impact=weighted_impact,
            kg=kg
        )
    
    def _compute_sensitivity(self, entity: Entity, impact: np.ndarray) -> float:
        """实体状态与冲击向量的内积 = 敏感度"""
        state = np.concatenate([
            entity.power_coord,
            entity.temporal_coord,
            entity.epistemic_coord,
            entity.cascade_coord
        ])
        return float(np.dot(state, impact))
```

### 8.3 Phase 路线图（修订）

| Phase | 目标 | 关键产出 |
|-------|------|----------|
| P1 | 单纯形坐标初始化 | 183 实体 × 4 切面 × 4 端点坐标矩阵 |
| P2 | 规则引擎 | 从 7 列表头自动推导坐标的规则系统 |
| P3 | 耦合矩阵标定 | 100+ 核心关系的耦合矩阵（人工标注 + 历史回测拟合） |
| P4 | 级联引擎 | Σ-CASCADE 算法的确定性实现 |
| P5 | 回测验证 | 10+ 历史事件的端到端回测，指标 > 基线 |
| P6 | 生产部署 | API + K8s + 监控 |

---

## 9. 附录：数学符号表

| 符号 | 含义 |
|------|------|
| Σ | 完整状态空间 Σ = 𝓟 × 𝓣 × 𝓔 × 𝓒 |
| 𝓟 | 权力拓扑切面（3-单形） |
| 𝓣 | 时间存在切面（3-单形） |
| 𝓔 | 认知可达切面（3-单形） |
| 𝓒 | 级联响应切面（3-单形） |
| Δ³ | 标准 3-单形 {x ∈ ℝ⁴ : Σxᵢ = 1, xᵢ ≥ 0} |
| π𝓢(e) | 实体 e 在切面 𝓢 上的投影 |
| D𝓢 | 切面上的 Jensen-Shannon 距离 |
| M𝓧𝓨 | 从切面 𝓧 到 𝓨 的耦合矩阵 |
| ⊙ | Hadamard（逐元素）积 |
| ⟨·,·⟩ | 欧几里得内积 |
| λ(·) | 按实体类型的衰减系数 |

---

*本文档将随着实现深入持续迭代。下一迭代重点：P1 阶段——为 183 实体生成初始单纯形坐标。*
