# SCS-GlobalCapital：多实体级联依赖推理系统架构方案

> **版本**：v1.0-draft  
> **基线数据**：`docs/global_capital_players_full_index.csv`（183 实体，5 分类）  
> **目标**：从 0 到 Production 的确定性白盒级联推理基础设施

---

## 1. 执行摘要

本方案以全球资本全景表（183 实体）为真实数据基座，构建一个**形式化、时序化、可回测**的多实体级联依赖推理系统。系统核心能力：

- **实体建模**：尊重表格颗粒度，通过 Schema 约束确保每个实体的属性、行为模板、可观测信号结构化
- **关系建模**：显式因果边（市场传导）+ 隐式组织边（控股/家族/人事关联），边带权重、方向、时滞、置信度
- **级联推理**：Lazy Cascade 引擎，事件注入→局部子图展开→置信度传播→N 跳推理链输出
- **时序管理**：VersionedFactStore 管理所有事实的生效/失效时间轴，支持历史回测
- **中国主体**：独立标记（`is_chinese: true`），行为模板与欧美资本差异化定义

拒绝 LLM 黑盒提取，所有推理链为**确定性白盒推导**，可审计、可验证、可复现。

---

## 2. 系统定位与核心假设

### 2.1 显式声明架构假设

| 假设编号 | 假设内容 | 理由 |
|----------|----------|------|
| A1 | **显式标识符优先于向量相似度** | 实体对齐以人工/权威来源的显式 ID（如 CIK、LEI、工商注册号）为地基，而非向量计算 |
| A2 | **LLM 仅负责「丢记忆进来」** | 实体关联、级联更新、置信度传播由系统层自动完成，推理过程确定性 |
| A3 | **分类即行为模板** | A/B/C/D/E 不是标签，而是级联响应函数的「基类」，中国主体在此基础上差异化 |
| A4 | **时间是第一公民** | 所有事实、关系、属性均带时间区间 `[valid_from, valid_until)`，支持历史版本查询 |
| A5 | **组织关系是级联的放大器** | 同一家族/控股体系下的实体间关联权重可设为极大值（如 0.99），近似「同节点」 |

### 2.2 与 MEME 基准的对齐

MEME 暴露的问题是：当实体数 > 阈值后，中间跳丢失。本系统通过以下机制解决：

1. **Lazy Cascade**：不展开全图，只展开与事件相关的子图
2. **Schema 约束的剪枝**：A 类实体对「利率变化」敏感，C 类对「能源价格」敏感，按分类模板剪枝无关分支
3. **置信度阈值截断**：每跳传播后低于阈值的推理链直接丢弃，避免噪声扩散

---

## 3. 数据模型层（Data Model Layer）

### 3.1 Entity Schema（实体定义）

```yaml
Entity:
  id: string            # 全局唯一标识符，如 "soros_fund_mgmt"
  display_name: string  # 显示名，如 "Soros Fund Management"
  aliases: [string]     # 别名列表，用于检索匹配
  
  entity_type: enum     # A/B/C/D/E
  is_chinese: boolean   # 中国主体标记，独立筛选分析
  
  # 时间轴
  founded_at: date      # 创立时间，如 "1969-01-01"
  dissolved_at: date    # 解散时间（如已终结的历史实体）
  
  # 行为模板（继承自 entity_type，可覆盖）
  behavior_template:    # 见 3.3 节
    sensitivity_matrix: # 对各类冲击的敏感度函数
      interest_rate: function
      commodity_price: function
      geopolitical_risk: function
      currency_fluctuation: function
  
  # 当前快照（VersionedFactStore 的最新版本）
  current_snapshot:
    key_personnel: [AgentRef]      # 负责人/关键人
    core_beliefs: [Belief]         # 核心观点/投资哲学
    domains: [MarketDomain]        # 主要阵地
    payoff_function: PayoffDesc    # 获利方式
    observables: [Observable]      # 公开追踪渠道
    aum: number                    # 管理规模（如适用）
    headquarters: string           # 总部所在地
  
  # 元数据
  data_source: string   # 数据来源（如 "user_csv_v1"）
  confidence: float     # 该实体信息的整体置信度 [0,1]
```

### 3.2 Agent Schema（关键人/家族节点）

关键人作为独立节点，与组织形成「人事关联」边：

```yaml
Agent:
  id: string            # 如 "george_soros"
  name: string
  type: enum            # INDIVIDUAL | FAMILY | GOVERNMENT
  
  # 时间轴
  birth_date: date
  death_date: date
  
  # 关联组织（动态，随时间变化）
  affiliations:
    - entity_id: string
      role: string      # "founder", "ceo", "board_member", "family_heir"
      from: date
      to: date
```

**设计理由**：
- George Soros 1992 年做空英镑时，他是 Soros Fund Management 的实控人；2023 年后 Alex Soros 接管 OSF，同一组织的行为模式发生了代际切换
- 没有 Agent 节点时序管理，级联推理会在代际切换时产生系统性错误

### 3.3 Relation Schema（关系定义）

关系是级联推理的核心载体。每条关系必须带时序和权重：

```yaml
Relation:
  id: string
  from_entity: string   # 源实体 ID
  to_entity: string     # 目标实体 ID
  relation_type: enum   # 见下表
  
  # 时间维度（支持回测的关键）
  valid_from: date
  valid_until: date     # null 表示当前仍有效
  
  # 级联属性
  weight: float         # 关联权重 [0, 1]，极大权重（如 0.99）用于组织强绑定
  direction: enum       # DIRECTED | BIDIRECTIONAL
  time_lag: duration    # 传导时滞，如 "3M"（3个月）、"1D"（1天）
  
  # 激活条件（某些关系只在特定情境下激活）
  activation_condition:
    type: enum          # ALWAYS | EVENT_TRIGGERED | BELIEF_DEPENDENT
    trigger_event: string  # 如 "geopolitical_conflict_escalation"
    required_belief: string # 如 "contrarian_macro"
  
  # 置信度
  confidence: float     # [0, 1]，基于数据源可靠性
  evidence: [string]    # 支撑该关系的证据来源
```

**关系类型枚举**：

| 类型 | 说明 | 示例 |
|------|------|------|
| `ORGANIZATIONAL` | 组织关系：控股、子公司、家族分支 | Koch Industries → Koch Family Foundation |
| `PERSONNEL` | 人事关联：任职、创始人、家族继承 | George Soros → Soros Fund Management |
| `CAUSAL_MARKET` | 市场因果：价格/政策传导 | Saudi Aramco(OPEC决策) → 全球油价 |
| `CAUSAL_FINANCIAL` | 金融因果：资金链、债务担保 | 中投 → 中央汇金(控股) → 国内大行 |
| `NARRATIVE` | 叙事关联：媒体/智库/舆论影响 | OSF 资助 → 受资助媒体 → 市场叙事 |
| `COMPETITIVE` | 竞争关系：同一市场争夺 | Glencore ↔ Trafigura（大宗商品贸易） |
| `SYMBIOTIC` | 共生关系：互补依赖 | Boeing ↔ 美国国防部（长期合同绑定） |

### 3.4 Event Schema（事件定义）

```yaml
Event:
  id: string
  event_type: enum      # GEOPOLITICAL | MONETARY | COMMODITY | CORPORATE | REGULATORY
  description: string
  
  # 时间定位
  occurred_at: datetime
  announced_at: datetime  # 公开披露时间（可能与发生时间不同）
  
  # 影响参数
  impact_vector:        # 对各维度的冲击强度
    geopolitical_risk: float   # [-1, 1]，+1 表示风险急剧上升
    interest_rate: float       # 基点变化
    commodity_price:           # 按商品类型细分
      crude_oil: float
      natural_gas: float
      copper: float
  
  # 直接影响实体
  direct_targets: [string]   # 实体 ID 列表
  
  # 数据源
  source: string        # 如 " Reuters_2024_03_15 "
  confidence: float
```

### 3.5 中国主体的差异化标记

```yaml
ChineseEntityExtension:
  entity_id: string     # 关联到主 Entity
  
  # 中国特有属性
  ownership_type: enum  # SOVEREIGN_FUND | CENTRAL_ENTERPRISE | PRIVATE_CONGLOMERATE | PRIVATE_EQUITY
  regulatory_framework: string  # 如 "国资委", "证监会私募监管", "香港SFC"
  
  # 行为模板覆盖（继承自基类，差异化覆盖）
  behavior_override:
    # 中国机构不存在 NGO 舆论干预模式
    narrative_influence: null
    # 以产业周期、政策导向、期现对冲为主
    policy_sensitivity: HIGH
    # 主权资金以稳定市场为目标，不做空收割
    short_selling: PROHIBITED   # 对主权/央企而言
  
  # 国内追踪渠道补充
  domestic_observables:
    - 私募排排网净值
    - 期货交易所持仓
    - 国资委公告
    - 中国海关总署数据
```

---

## 4. 级联推理引擎（Cascade Inference Engine）

### 4.1 核心算法：Lazy Cascade with Schema-Driven Pruning

```
输入: 初始事件 E, 最大跳数 N_max, 置信度阈值 θ, 分类剪枝集 Φ
输出: 级联推理图 G = (V, E, confidence)

1. 初始化:
   - active_nodes ← {E.direct_targets}
   - G ← 空图
   - queue ← [(node, hop=0, confidence=1.0) for node in active_nodes]

2. 当 queue 非空:
   a. (current, hop, conf) ← queue.pop()
   b. 若 hop >= N_max 或 conf < θ: 跳过
   
   c. 获取 current 的 entity_type → behavior_template
   
   d. 按以下顺序筛选候选关系:
      i.   时间过滤: 关系 valid_from/valid_until 必须覆盖 E.occurred_at
      ii.  分类模板过滤: 若 E.impact_vector 的维度 ∉ behavior_template.sensitivity_matrix.keys()，剪枝
      iii. 激活条件过滤: 检查 relation.activation_condition
      iv.  组织权重放大: 若 relation.type == ORGANIZATIONAL 且 weight > 0.9，
           则该关系优先级置顶（近似同节点传导）
   
   e. 对通过筛选的每条关系 R:
      - next_node ← R.to_entity
      - next_conf ← conf × R.weight × R.confidence × decay(hop)
      - G.add_edge(current, next_node, weight=R.weight, lag=R.time_lag)
      - queue.push((next_node, hop+1, next_conf))

3. 返回 G
```

### 4.2 置信度衰减函数

```python
def decay(hop: int, entity_type: str) -> float:
    """
    每跳置信度衰减，不同类型衰减速度不同。
    A类（宏观对冲）对远程传导更敏感，衰减较慢；
    D类（家族/政治）受人事变动影响大，衰减较快。
    """
    base_decay = {
        'A': 0.92,   # 宏观基金跨市场传导能力强
        'B': 0.85,   # 军工依赖政策周期，传导较确定
        'C': 0.88,   # 大宗依赖实物供需链
        'D': 0.75,   # 家族/政治受人事变动影响大
        'E': 0.70,   # 其他，信息不透明
    }
    return base_decay.get(entity_type, 0.80) ** hop
```

### 4.3 组织关系的「极大权重」机制

当两个实体属于同一家族/控股体系时，设置 `weight >= 0.99`，这意味着：

1. **级联时近似合并节点**：Koch Industries 受到能源政策冲击时，Koch Family Foundation 的响应几乎同步
2. **人事变动触发重组**：当 `affiliation.to` 日期到达（如 George Soros 退休），系统自动将 Soros Fund Management 与 Alex Soros 的关联权重提升，旧关联权重下降
3. **回测时精确还原**：1992 年英镑做空事件发生时，George Soros 是实控人，Alex Soros 尚未接管，系统按时间版本正确路由

---

## 5. 时序管理与回测框架（Temporal Layer）

### 5.1 VersionedFactStore

所有可变的实体属性、关系、人事关联都必须通过 VersionedFactStore 管理：

```yaml
Fact:
  id: string
  entity_id: string     # 关联实体
  attribute: string     # 属性名，如 "key_personnel", "aum", "headquarters"
  
  # 值（JSON 序列化）
  value: any
  
  # 时间区间
  valid_from: date
  valid_until: date     # null 表示当前有效
  
  # 来源与置信度
  source: string
  confidence: float
  
  # 版本控制
  created_at: datetime
  superseded_by: string # 若被更新，指向新 Fact ID
```

**关键操作**：

| 操作 | 说明 |
|------|------|
| `insert_fact` | 插入新事实，自动将旧事实的 valid_until 设为新事实的 valid_from |
| `query_at_time` | 查询某实体在特定时间点的完整快照 |
| `query_history` | 查询某属性的全部历史版本 |
| `temporal_join` | 跨实体的时间对齐查询（回测核心） |

### 5.2 回测协议

```python
class BacktestProtocol:
    """
    回测 = 在历史的某个时间点 T，注入已知事件 E，
    运行级联推理，与 T+N 的实际观测对比。
    """
    
    def run(
        self,
        event: Event,           # 历史事件（带 occurred_at）
        observation_horizon: datetime,  # 验证时间点
        expected_targets: [str],        # 预期受影响的实体（人工标注或来自公开数据）
        metrics: [Metric]               # 评估指标
    ) -> BacktestResult:
        
        # 1. 构建 T 时刻的知识图谱快照
        kg_at_t = self.versioned_store.snapshot_at(event.occurred_at)
        
        # 2. 运行级联推理
        cascade_graph = self.cascade_engine.run(
            event=event,
            knowledge_graph=kg_at_t,
            max_hops=5,
            theta=0.3
        )
        
        # 3. 提取预测结果
        predicted_targets = cascade_graph.get_affected_entities()
        
        # 4. 与实际观测对比
        return self.evaluate(predicted_targets, expected_targets, metrics)
```

**评估指标**：

| 指标 | 定义 |
|------|------|
| `Precision@K` | 预测的前 K 个受影响实体中，实际受影响的比例 |
| `Recall@K` | 实际受影响的实体中，被预测在前 K 个的比例 |
| `HopAccuracy` | 预测的传导跳数与实际跳数的平均误差 |
| `ConfidenceCalibration` | 预测置信度与实际命中率的校准曲线 |

### 5.3 时间轴数据补齐任务

当前表格缺少精确创立时间，需要后续调研补齐：

| 实体 | 已知时间信息 | 需补齐字段 |
|------|-------------|-----------|
| Soros Fund Management | 1969 年创立 | `founded_at: 1969-01-01` |
| Bridgewater Associates | 1975 年创立 | `founded_at: 1975-01-01` |
| 敦和资管 | 表格无 | 需调研工商注册时间 |
| 中投公司 | 2007 年 9 月成立 | `founded_at: 2007-09-29` |
| 中国兵器工业集团 | 1999 年改制 | `founded_at: 1999-07-01` |

---

## 6. 组织架构关系建模（Organization Layer）

### 6.1 需要调研整理的组织关系类型

基于表格内容，以下组织关系需要独立建模：

#### 6.1.1 家族控股网络

```yaml
FamilyNetwork:
  family_id: string     # 如 "koch_family"
  members: [Agent]
  controlled_entities:  # 控股或实质控制的组织
    - entity_id: string
      control_type: enum   # MAJORITY_OWNERSHIP | VOTING_CONTROL | FOUNDATION_GRANT
      control_ratio: float # 持股比例（如适用）
  
  # 时间轴
  established_at: date
  active_periods: [(from, to)]  # 家族影响力时间区间
```

**高优先级家族网络**（来自表格）：
- Koch 家族（Koch Industries + Americans for Prosperity + Stand Together）
- Soros 家族（Soros Fund Management + OSF）
- Rockefeller 家族（Rockefeller Foundation + Rockefeller Brothers Fund + Venrock）
- Murdoch 家族（News Corp + Fox Corp）
- Walton 家族（Walmart ~50% 持股）
- Adelson 家族（LVS + Israel Hayom）
- 郭鹤年家族（Wilmar International）
- 马云/阿里系（阿里巴巴 + 蚂蚁集团 + 云锋基金）
- 马化腾/腾讯系

#### 6.1.2 央企集团架构

```yaml
StateOwnedConglomerate:
  root_entity: string   # 如 "中粮集团"
  parent_regulator: string  # "国资委"
  
  subsidiaries:
    - entity_id: string
      business_scope: string
      ownership_ratio: float
  
  listed_vehicles: [string]  # 上市公司代码
```

**高优先级央企**（来自表格）：
- 中粮集团 → 中粮国际（海外贸易板块）
- 中国五矿集团 → 各上市平台（中国中冶、五矿资源等）
- 中国兵器工业集团 → 北方工业等军贸平台
- 中国航空工业集团 → 中航系上市公司群
- 中国航天科技集团 → 航天系上市公司群

#### 6.1.3 跨国资管网络

```yaml
AssetManagerNetwork:
  # 如 Tiger Cub 网络
  parent_firm: string   # "Tiger Management"（Julian Robertson）
  alumni_funds: 
    - fund_id: string   # "Lone Pine Capital", "Viking Global", "Coatue", "Tiger Global"
      founder: string
      founded_at: date
      shared_strategy: string  # "growth_equity_long_short"
```

### 6.2 关联权重赋值规则

| 关系类型 | 默认权重 | 极大权重触发条件 |
|----------|----------|-----------------|
| 同一家族控股（>50% 投票权） | 0.95 | 家族完全控制私有企业 → 0.99 |
| 央企母子公司（国资委全资） | 0.90 | 同一集团核心子公司 → 0.95 |
| Tiger Cub 校友基金 | 0.60 | 共享同一 PM / 同一办公室 → 0.80 |
| 市场因果关系 | 0.40-0.80 | 取决于历史回测验证的传导强度 |
| 叙事/媒体关联 | 0.30-0.50 | 直接资金资助关系 → 0.70 |

---

## 7. 技术架构与生产部署

### 7.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                        API Gateway                              │
│  /inject_event  /query_snapshot  /run_cascade  /backtest        │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐   ┌─────────────────┐   ┌─────────────────┐
│  Ingestion    │   │  Query /        │   │  Cascade        │
│  Service      │   │  Snapshot       │   │  Engine         │
│               │   │  Service        │   │                 │
│ - CSV import  │   │                 │   │ - Lazy expand   │
│ - Fact insert │   │ - temporal_join │   │ - Schema prune  │
│ - Event log   │   │ - version query │   │ - Conf. propagate│
└───────┬───────┘   └─────────────────┘   └─────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────┐
│                  Storage Layer                              │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐   │
│  │  Entity     │  │  Versioned   │  │  Graph          │   │
│  │  Registry   │  │  Fact Store  │  │  Index (关系)    │   │
│  │  (PostgreSQL│  │  (PostgreSQL │  │  (Neo4j /       │   │
│  │   + JSONB)  │  │   + 时序扩展) │  │   DuckDB Graph) │   │
│  └─────────────┘  └──────────────┘  └─────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 7.2 存储选型

| 组件 | 选型 | 理由 |
|------|------|------|
| Entity Registry | PostgreSQL + JSONB | 结构化属性 + 灵活扩展 |
| Versioned Fact Store | PostgreSQL + `temporal_tables` 扩展 | 成熟时序支持，`system_time` 自动管理 |
| Graph Index | Neo4j 或 DuckDB + `graph` 扩展 | Neo4j 适合复杂图查询；DuckDB 适合分析型批量回测 |
| 事件日志 | Apache Kafka 或 Redis Stream | 事件驱动架构，支持回放 |
| 缓存 | Redis | 热点图谱快照缓存 |

### 7.3 生产部署拓扑

```
[Load Balancer]
      │
┌─────┴─────┐
│  API Pod  │ ×3 (Kubernetes Deployment)
│  (FastAPI)│
└─────┬─────┘
      │
┌─────┴─────────────┐
│  Cascade Worker   │ ×N (K8s HPA, 按事件量自动扩缩)
│  (Celery / RQ)    │
└─────┬─────────────┘
      │
┌─────┴─────────────┐
│  PostgreSQL       │ ×1 Primary + ×2 Read Replica
│  (Patroni HA)     │
└─────┬─────────────┘
      │
┌─────┴─────────────┐
│  Neo4j / DuckDB   │ ×1 (分析型，可接受重建)
└───────────────────┘
```

### 7.4 关键 API 设计

```yaml
# 事件注入
POST /api/v1/events
body:
  event_type: "GEOPOLITICAL_CONFLICT_ESCALATION"
  occurred_at: "2024-02-24T00:00:00Z"
  impact_vector:
    geopolitical_risk: 0.8
    commodity_price:
      crude_oil: 0.15    # +15%
      natural_gas: 0.30
  direct_targets: ["saudi_aramco", "glencore", "rheinmetall"]

# 级联推理
POST /api/v1/cascade/run
body:
  event_id: "evt_20240224_001"
  max_hops: 5
  confidence_threshold: 0.3
  entity_filter:          # 可选：只分析中国主体
    is_chinese: true
  
response:
  affected_entities: 42
  max_hop_reached: 4
  inference_chains:
    - path: ["saudi_aramco", "global_oil_price", "inflation_expectation", "fed_hawkish", "element_capital"]
      final_confidence: 0.45
      total_lag: "6M"
```

---

## 8. 开发路线图（从 0 到 Production）

### Phase 1：数据基座（Week 1-2）

| 任务 | 产出 | 验收标准 |
|------|------|----------|
| Schema 定义固化 | `schemas/entity.yaml`, `schemas/relation.yaml` | 能通过 JSON Schema 验证 |
| CSV 数据清洗入库 | `scripts/ingest_csv.py` | 183 实体全部入库，无解析错误 |
| 去重与别名映射 | `data/aliases.json` | 同一实体的不同称呼可正确解析 |
| 中国主体标记 | `data/chinese_entities.json` | 可独立筛选 |

### Phase 2：时序框架（Week 3-4）

| 任务 | 产出 | 验收标准 |
|------|------|----------|
| VersionedFactStore 实现 | `src/fact_store.py` | 支持 insert/query_at_time/query_history |
| 时间轴数据补齐 | 调研 30+ 实体的创立/关键人事时间 | 至少 50% 实体有 `founded_at` |
| 历史事件种子库 | `data/seed_events/` | 5 个已验证的历史事件（如 1992 英镑危机） |
| 回测框架骨架 | `src/backtest.py` | 能跑通单个事件的端到端回测 |

### Phase 3：关系图谱（Week 5-6）

| 任务 | 产出 | 验收标准 |
|------|------|----------|
| 组织关系建模 | `src/org_graph.py` | Koch/Soros/Rockefeller 家族网络可查询 |
| 关系权重标定 | `data/relation_weights.json` | 人工标注 100+ 条核心关系权重 |
| 人事关联时序 | `src/personnel_tracker.py` | George Soros 退休事件正确切换关联权重 |
| 图索引构建 | Neo4j 或 DuckDB 导入 | 全图查询 < 100ms |

### Phase 4：级联引擎（Week 7-8）

| 任务 | 产出 | 验收标准 |
|------|------|----------|
| Lazy Cascade 核心 | `src/cascade_engine.py` | 单次推理 < 5s（183 实体全图） |
| 分类模板实现 | `src/behavior_templates/` | A/B/C/D/E 各有独立敏感度矩阵 |
| 置信度传播 | `src/confidence_propagator.py` | 跳数衰减曲线可配置 |
| Schema 驱动剪枝 | `src/schema_pruner.py` | 无关分支剪枝率 > 60% |

### Phase 5：回测与验证（Week 9-10）

| 任务 | 产出 | 验收标准 |
|------|------|----------|
| 历史事件回测集 | `tests/backtest_suite/` | 10+ 历史事件，人工标注预期结果 |
| 指标计算 | `src/metrics.py` | Precision@K, Recall@K, HopAccuracy |
| 阈值调优 | 置信度阈值 θ 网格搜索 | 找到 θ 的帕累托前沿 |
| MEME 基准对齐 | 与 MEME 测试集对比 | 在合成数据上超越 LLM 基线 |

### Phase 6：生产化（Week 11-12）

| 任务 | 产出 | 验收标准 |
|------|------|----------|
| API 服务 | FastAPI + OpenAPI 文档 | 响应时间 P99 < 200ms |
| 容器化 | Dockerfile + K8s manifests | 可一键部署到任意 K8s 集群 |
| 监控与告警 | Prometheus + Grafana | 级联推理耗时、命中率实时看板 |
| 数据更新管道 | 定时任务（Airflow / CronJob） | 月度自动更新 CSV/事实数据 |

---

## 9. 附录

### 9.1 命名规范

| 类型 | 命名规则 | 示例 |
|------|----------|------|
| 实体 ID | `snake_case`，组织名缩写 | `soros_fund_mgmt`, `cic_china` |
| 代理人 ID | `snake_case` | `george_soros`, `ye_qingjun` |
| 关系 ID | `{from}__{type}__{to}` | `soros_fund_mgmt__personnel__george_soros` |
| 事件 ID | `evt_{YYYYMMDD}_{seq}` | `evt_19920916_001` |
| 事实 ID | UUID v4 | `f47ac10b-58cc-4372-a567-0e02b2c3d479` |

### 9.2 数据源优先级

| 优先级 | 来源类型 | 置信度 |
|--------|----------|--------|
| P0 | SEC 13F/10-K、央行年报、交易所披露 | 0.95 |
| P1 | 行业排名（LCH、HFR）、权威媒体（Reuters/Bloomberg） | 0.85 |
| P2 | 财新、私募排排网、海关数据 | 0.75 |
| P3 | 公开演讲、传记、行业报道 | 0.60 |
| P4 | 推测/模型补全 | 0.40（需标注） |

### 9.3 中国主体独立分析接口

```python
# 示例：只分析中国主体在特定事件下的级联响应
result = cascade_engine.run(
    event=event,
    entity_filter={"is_chinese": True},
    behavior_override={          # 差异化覆盖
        "narrative_influence": None,
        "policy_sensitivity": "HIGH",
        "short_selling": "PROHIBITED"
    }
)
```

---

## 10. 下一步行动

1. **确认 Schema 设计**：请审阅 Entity / Relation / Event Schema，确认字段完备性
2. **优先级排序**：家族网络、央企架构、Tiger Cub 网络，哪一类组织关系优先建模？
3. **时间轴补齐**：是否需要我先调研一批核心实体（如中投、敦和、中国兵器工业）的创立时间？
4. **技术栈确认**：Neo4j vs DuckDB 图扩展，倾向哪个？

---

*本文档为架构设计初稿，后续随实现迭代更新。*
