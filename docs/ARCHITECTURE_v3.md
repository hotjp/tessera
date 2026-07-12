# Σ³-System：跨类型单纯形级联推理架构 v3.0

> **版本**：v3.0  
> **核心抽象**：统一实体空间 Ξ = ORG ∪ EVENT ∪ PERSON，每个实体是切面-端点权重矩阵的帕累托投影  
> **设计约束**：端点可量化、切面可独立计算、抽象矩阵位置唯一、类型可扩展、多约束帕累托求解  
> **数学基础**：凸分析（单纯形约束）、多面体投影（Duchi 2008）、帕累托前沿（多目标优化）、张量拼接唯一性

---

## 1. 修正声明：从 v2.0 到 v3.0 的关键升级

| v2.0 缺陷 | v3.0 修正 |
|-----------|-----------|
| 实体类型扁平（只有 ORG） | 统一实体论：ORG / EVENT / PERSON 共享同一数学框架 |
| 端点无量化指标 | 每个端点绑定可观测 `metrics`，权重从数据独立计算 |
| 切面坐标计算隐式依赖全局知识 | 每个切面有独立的 `compute_slice()` 协议，输入只有实体观测数据 |
| "抽象矩阵"语义模糊 | 明确定义为 **S × K_max 的实矩阵**，Frobenius 范数下位置唯一 |
| 中国主体约束硬编码 | 配置化 `ConstraintSet`，支持任意实体的多约束帕累托投影 |
| 事件未纳入状态空间 | EVENT 作为「可越界实体」（冲击向量允许突破单纯形），通过共享切面与 ORG/PERSON 交叉 |

---

## 2. 统一实体论（Entity Ontology）

### 2.1 核心公理

**公理 1（实体同一性）**：系统中所有可独立命名的对象——组织、事件、个人——都是**实体** `ξ ∈ Ξ`。实体的身份不由类型标签定义，而由其在**切面-端点空间**中的坐标唯一确定。

**公理 2（切面不变性）**：切面（Slice）是系统的分析维度，不由实体类型决定。但不同类型的实体**可激活的切面子集**可以不同。例如 EVENT 实体激活 `impact_topology` 切面，而 ORG 实体不激活。

**公理 3（端点可量化）**：每个端点（Endpoint）必须绑定一组**可观测指标** `Metrics = {m₁, m₂, ..., mₙ}`，每个指标有明确的**数据来源**、**提取规则**和**归一化函数**。端点权重 = 指标加权聚合后投影到单纯形。

**公理 4（独立计算）**：实体在切面 s 上的坐标 `πₛ(ξ)` 的计算**仅依赖** ξ 自身的可观测数据 `Obs(ξ)` 和切面 s 的端点定义 `End(s)`，不依赖其他切面或其他实体。

**公理 5（抽象矩阵唯一性）**：定义拼接算子 `⊕` 将各切面坐标堆叠为矩阵 `M_ξ ∈ ℝ^{S×K_max}`。在固定切面配置下，映射 `ξ ↦ M_ξ` 是**单射**（injective）：不同观测数据的实体必然有不同的抽象矩阵。

### 2.2 类型继承体系

```
Entity (抽象基类)
├── id: UUID                    # 全局唯一标识
├── type: enum                  # ORG | EVENT | PERSON
├── display_name: str
├── temporal_bound: [DateTime, DateTime]  # [valid_from, valid_until)
├── observable_vector: Dict     # 原始观测数据（可验证）
├── slice_projections: Dict[str, List[float]]  # 切面 → 单纯形坐标
├── abstract_matrix: ndarray    # S × K_max 的实矩阵
├── constraint_set: List[Constraint]  # 多约束帕累托输入
├── paretor_frontier: List[ndarray]   # 当约束冲突时的前沿解集
└── provenance: List[str]       # 坐标推导的审计轨迹

    ORG (继承 Entity)
    ├── org_name: str
    ├── aum: Optional[float]
    ├── classification: str       # A/B/C/D/E（仅 ORG 有）
    ├── is_chinese: bool
    └── headquarters: str

    EVENT (继承 Entity)
    ├── event_type: str           # GEOPOLITICAL | MONETARY | COMMODITY | CORPORATE
    ├── occurred_at: datetime
    ├── impact_magnitude: float   # 冲击强度 [0, 1]
    ├── affected_domains: List[str]
    └── is_boundary_breaking: bool  # 是否允许越界（突破单纯形）

    PERSON (继承 Entity)
    ├── person_name: str
    ├── birth_date: Optional[datetime]
    ├── death_date: Optional[datetime]
    ├── net_worth: Optional[float]
    └── affiliations: List[(org_id, role, from, to)]
```

### 2.3 为什么事件也要进入同一空间

事件不是「发生后就消失」的东西。在级联推理中，事件是**瞬时实体**，它的坐标在发生时刻定义，之后通过 `temporal_bound.valid_until` 标记失效。但事件的历史坐标永久保留，用于：

1. **回测**：在 `occurred_at` 时刻，EVENT 作为活跃实体参与级联
2. **模式学习**：同类 EVENT 的 abstract_matrix 聚类 → 形成「事件原型」
3. **交叉分析**：PERSON（如 George Soros）与 EVENT（1992 英镑危机）通过 `POWER` 和 `TEMPORAL` 切面建立跨类型关系

**关键设计**：EVENT 的 `is_boundary_breaking = true` 时，其坐标**不投影回单纯形**，保留原始冲击向量的越界特性。例如「俄乌全面升级」的 `cascade.brittle` 分量可能是 +0.80（超出单纯形边界），这表示它对脆性实体的冲击超过了正常传导范围。

---

## 3. 可扩展切面系统（SliceRegistry）

### 3.1 注册表协议

切面、端点、指标全部是**配置驱动**，不是 hardcode。系统启动时从 YAML 加载：

```yaml
# config/slices.yaml
slice_registry:
  version: "3.0.0"
  
  slices:
    # ============================================
    # 切面 1：权力拓扑 (POWER)
    # ============================================
    - id: "power"
      name: "权力拓扑"
      description: "实体靠什么影响世界"
      endpoints:
        - id: "sovereignty"
          name: "主权"
          metrics:
            - id: "govt_contract_revenue_ratio"
              description: "政府合同收入占总营收比例"
              source: "SEC_10K_item1 / 年报"
              extractor: "regex_government_revenue"
              normalize: 
                method: "sigmoid"
                params: {scale: 10.0, shift: 0.5}
              weight: 0.35
            
            - id: "regulatory_capture_index"
              description: "监管俘获指数 = 游说支出 / 总营收"
              source: "senate_lobbying_disclosure / opensecrets"
              extractor: "lobbying_to_revenue_ratio"
              normalize:
                method: "min_max"
                params: {clip: [0.0, 0.05]}
              weight: 0.25
            
            - id: "state_ownership_ratio"
              description: "国有股权比例"
              source: "股权结构披露 / 国资委公告"
              extractor: "direct_state_equity"
              normalize:
                method: "identity"
              weight: 0.40
          
          # 端点权重计算 = Σ(weight_i · normalized_metric_i)
          # 然后与其他端点一起投影到单纯形
        
        - id: "capital"
          name: "资本"
          metrics:
            - id: "aum_log"
              description: "管理资产规模（十亿美元对数）"
              source: "industry_reports / SEC_13F"
              extractor: "aum_from_13f"
              normalize:
                method: "log_sigmoid"
                params: {center: 2.0, scale: 3.0}  # log10(AUM/1B)
              weight: 0.30
            
            - id: "leverage_ratio"
              description: "资产负债表杠杆率"
              source: "SEC_10K / 监管披露"
              extractor: "total_assets_over_equity"
              normalize:
                method: "sigmoid"
                params: {scale: 5.0, shift: 3.0}
              weight: 0.30
            
            - id: "market_maker_share"
              description: "做市市场份额（如 Citadel Securities）"
              source: "FINRA / 交易所数据"
              extractor: "volume_share_percent"
              normalize:
                method: "min_max"
                params: {clip: [0.0, 0.30]}
              weight: 0.20
            
            - id: "pe_vc_deal_count"
              description: "年度投资/并购交易数量（对数）"
              source: "crunchbase / pitchbook"
              extractor: "annual_deal_count_log"
              normalize:
                method: "log_sigmoid"
                params: {center: 1.5, scale: 2.0}
              weight: 0.20
        
        - id: "production"
          name: "生产"
          metrics:
            - id: "physical_output_value"
              description: "实物产出价值（对数，亿美元）"
              source: "年报 / 行业报告"
              extractor: "cogs_plus_inventory"
              normalize:
                method: "log_sigmoid"
                params: {center: 2.0, scale: 2.5}
              weight: 0.35
            
            - id: "supply_chain_criticality"
              description: "供应链关键度 = 不可替代输入品数量 / 总输入品"
              source: "供应链披露 / 行业分析"
              extractor: "critical_input_ratio"
              normalize:
                method: "sigmoid"
                params: {scale: 8.0, shift: 0.3}
              weight: 0.35
            
            - id: "commodity_pricing_power"
              description: "大宗商品定价权指数"
              source: "LME / ICE / 期货持仓数据"
              extractor: "market_share_in_commodity"
              normalize:
                method: "min_max"
                params: {clip: [0.0, 0.50]}
              weight: 0.30
        
        - id: "narrative"
          name: "叙事"
          metrics:
            - id: "media_ownership_reach"
              description: "媒体受众覆盖（百万人）"
              source: "Nielsen / 发行量 / 月活数据"
              extractor: "total_audience_millions"
              normalize:
                method: "log_sigmoid"
                params: {center: 2.0, scale: 2.0}
              weight: 0.25
            
            - id: "ngo_funding_outflow"
              description: "NGO/智库拨款规模（对数，百万美元）"
              source: "IRS_990 / 基金会年报"
              extractor: "annual_grant_outflow_log"
              normalize:
                method: "log_sigmoid"
                params: {center: 1.0, scale: 2.0}
              weight: 0.25
            
            - id: "social_media_influence"
              description: "社交媒体影响力得分"
              source: "推特/X / 微博 / 平台API"
              extractor: "engagement_rate_times_followers"
              normalize:
                method: "log_sigmoid"
                params: {center: 3.0, scale: 3.0}
              weight: 0.25
            
            - id: "policy_whitepaper_output"
              description: "年度政策白皮书/研究报告数量"
              source: "智库网站 / 学术数据库"
              extractor: "annual_publication_count"
              normalize:
                method: "sigmoid"
                params: {scale: 0.5, shift: 10.0}
              weight: 0.25
    
    # ============================================
    # 切面 2：时间存在 (TEMPORAL)
    # ============================================
    - id: "temporal"
      name: "时间存在"
      description: "实体在时间中以什么模式存在"
      endpoints:
        - id: "instant"
          name: "瞬时"
          metrics:
            - id: "transaction_frequency"
              description: "日均交易/操作频次（对数）"
              source: "交易所 / 内部系统"
              extractor: "daily_operation_count_log"
              normalize:
                method: "log_sigmoid"
                params: {center: 3.0, scale: 2.0}
              weight: 1.0
        
        - id: "endurance"
          name: "持存"
          metrics:
            - id: "age_years"
              description: "存在年限（年）"
              source: "工商注册 / 创立日期"
              extractor: "years_since_founded"
              normalize:
                method: "sigmoid"
                params: {scale: 0.1, shift: 50.0}
              weight: 0.50
            
            - id: "balance_sheet_stability"
              description: "资产负债表稳定性 = 长期资产 / 总资产"
              source: "年报"
              extractor: "long_term_asset_ratio"
              normalize:
                method: "identity"
              weight: 0.50
        
        - id: "recurrence"
          name: "循环"
          metrics:
            - id: "reporting_cycle_regularity"
              description: "报告周期规律性得分"
              source: "财报发布日期序列"
              extractor: "date_regularity_score"
              normalize:
                method: "identity"
              weight: 0.40
            
            - id: "seasonal_revenue_variation"
              description: "营收季节性变异系数"
              source: "季报"
              extractor: "coefficient_of_variation_quarterly"
              normalize:
                method: "sigmoid"
                params: {scale: 5.0, shift: 0.3}
              weight: 0.30
            
            - id: "election_policy_cycle_exposure"
              description: "选举/政策周期暴露度"
              source: "政治事件时间线"
              extractor: "cycle_correlation_score"
              normalize:
                method: "identity"
              weight: 0.30
        
        - id: "becoming"
          name: "生成"
          metrics:
            - id: "revenue_growth_rate"
              description: "营收增长率（年化）"
              source: "财报"
              extractor: "yoy_revenue_growth"
              normalize:
                method: "sigmoid"
                params: {scale: 2.0, shift: 0.0}
              weight: 0.30
            
            - id: "rd_intensity"
              description: "研发投入强度 = R&D / 营收"
              source: "年报"
              extractor: "rd_over_revenue"
              normalize:
                method: "sigmoid"
                params: {scale: 10.0, shift: 0.1}
              weight: 0.30
            
            - id: "paradigm_shift_indicator"
              description: "范式转换指标（专利/新产品占比）"
              source: "专利数据库 / 产品公告"
              extractor: "new_product_revenue_share"
              normalize:
                method: "sigmoid"
                params: {scale: 5.0, shift: 0.2}
              weight: 0.40
    
    # ============================================
    # 切面 3：认知可达 (EPISTEMIC)
    # ============================================
    - id: "epistemic"
      name: "认知可达"
      description: "我们能在多大程度上知道实体在做什么"
      endpoints:
        - id: "opaque"
          name: "黑箱"
          metrics:
            - id: "days_since_last_disclosure"
              description: "距上次披露的天数"
              source: "监管数据库"
              extractor: "days_since_filing"
              normalize:
                method: "sigmoid"
                params: {scale: 0.01, shift: 180.0}
              weight: 0.50
            
            - id: "private_company_flag"
              description: "是否为非上市公司（0/1）"
              source: "工商 / SEC"
              extractor: "is_private_binary"
              normalize:
                method: "identity"
              weight: 0.50
        
        - id: "disclosed"
          name: "披露"
          metrics:
            - id: "filing_completeness_score"
              description: "申报文件完整度得分"
              source: "SEC / 交易所"
              extractor: "filing_section_count"
              normalize:
                method: "min_max"
                params: {clip: [0.0, 1.0]}
              weight: 0.40
            
            - id: "audit_quality_rating"
              description: "审计质量评级（Big4=1, 其他=0.5, 无=0）"
              source: "审计报告"
              extractor: "auditor_tier"
              normalize:
                method: "identity"
              weight: 0.30
            
            - id: "realtime_data_availability"
              description: "实时数据可得性（API/订阅）"
              source: "数据供应商"
              extractor: "has_realtime_feed"
              normalize:
                method: "identity"
              weight: 0.30
        
        - id: "inferred"
          name: "推断"
          metrics:
            - id: "exchange_position_data_quality"
              description: "交易所持仓数据质量得分"
              source: "CFTC COT / 交易所持仓"
              extractor: "position_data_granularity"
              normalize:
                method: "identity"
              weight: 0.50
            
            - id: "alternative_data_coverage"
              description: "另类数据覆盖度（卫星/信用卡/物流）"
              source: "数据供应商"
              extractor: "alt_data_source_count"
              normalize:
                method: "sigmoid"
                params: {scale: 0.5, shift: 5.0}
              weight: 0.50
        
        - id: "manipulated"
          name: "操纵"
          metrics:
            - id: "narrative_control_index"
              description: "叙事控制指数 = 自有媒体产出 / 总媒体提及"
              source: "媒体监测"
              extractor: "controlled_media_ratio"
              normalize:
                method: "sigmoid"
                params: {scale: 10.0, shift: 0.1}
              weight: 0.40
            
            - id: "social_bot_activity"
              description: "社交机器人活动检测得分"
              source: "平台API /  bot检测服务"
              extractor: "bot_engagement_ratio"
              normalize:
                method: "sigmoid"
                params: {scale: 20.0, shift: 0.05}
              weight: 0.30
            
            - id: "short_report_frequency"
              description: "做空/质疑报告发布频率"
              source: "Hindenburg / Muddy Waters 等"
              extractor: "reports_per_year"
              normalize:
                method: "sigmoid"
                params: {scale: 2.0, shift: 1.0}
              weight: 0.30
    
    # ============================================
    # 切面 4：级联响应 (CASCADE)
    # ============================================
    - id: "cascade"
      name: "级联响应"
      description: "冲击到达时实体以什么模式响应"
      endpoints:
        - id: "elastic"
          name: "弹性"
          metrics:
            - id: "strategy_diversity_index"
              description: "策略多样性指数 = 有效策略数 / 总策略数"
              source: "投资者信 / 年报"
              extractor: "strategy_count"
              normalize:
                method: "sigmoid"
                params: {scale: 0.5, shift: 5.0}
              weight: 0.40
            
            - id: "portfolio_turnover"
              description: "组合换手率（年化）"
              source: "SEC 13F"
              extractor: "annual_turnover_rate"
              normalize:
                method: "sigmoid"
                params: {scale: 2.0, shift: 1.0}
              weight: 0.30
            
            - id: "rebalancing_frequency"
              description: "再平衡频率（次/年）"
              source: "投资者信"
              extractor: "rebalance_per_year"
              normalize:
                method: "sigmoid"
                params: {scale: 0.3, shift: 4.0}
              weight: 0.30
        
        - id: "plastic"
          name: "塑性"
          metrics:
            - id: "strategic_pivot_count"
              description: "历史上重大战略转型次数"
              source: "传记 / 年报历史"
              extractor: "major_pivot_events"
              normalize:
                method: "sigmoid"
                params: {scale: 0.5, shift: 3.0}
              weight: 0.40
            
            - id: "long_term_investment_ratio"
              description: "长期投资占比"
              source: "资产负债表"
              extractor: "lt_investment_over_total"
              normalize:
                method: "identity"
              weight: 0.30
            
            - id: "restructuring_history"
              description: "重组历史得分"
              source: "新闻报道 / SEC 文件"
              extractor: "restructuring_event_count"
              normalize:
                method: "sigmoid"
                params: {scale: 0.3, shift: 2.0}
              weight: 0.30
        
        - id: "brittle"
          name: "脆性"
          metrics:
            - id: "concentration_risk_index"
              description: "集中度风险 = 最大单一敞口 / 总资产"
              source: "SEC 13F / 监管披露"
              extractor: "max_position_concentration"
              normalize:
                method: "sigmoid"
                params: {scale: 15.0, shift: 0.15}
              weight: 0.35
            
            - id: "leverage_stress_test"
              description: "杠杆压力测试 = 当前杠杆 / 历史最大安全杠杆"
              source: "财务数据"
              extractor: "leverage_to_max_safe"
              normalize:
                method: "sigmoid"
                params: {scale: 3.0, shift: 1.0}
              weight: 0.35
            
            - id: "liquidity_mismatch_ratio"
              description: "流动性错配 = 短期负债 / 流动资产"
              source: "资产负债表"
              extractor: "liquidity_mismatch"
              normalize:
                method: "sigmoid"
                params: {scale: 2.0, shift: 1.5}
              weight: 0.30
        
        - id: "absorptive"
          name: "吸收"
          metrics:
            - id: "sovereign_backing_score"
              description: "主权支持得分（0-1）"
              source: "政府公告 / 法律文件"
              extractor: "explicit_govt_guarantee"
              normalize:
                method: "identity"
              weight: 0.35
            
            - id: "reserve_buffer_ratio"
              description: "储备缓冲 = 现金及等价物 / 总资产"
              source: "资产负债表"
              extractor: "cash_reserve_ratio"
              normalize:
                method: "identity"
              weight: 0.35
            
            - id: "diversified_revenue_streams"
              description: "收入来源多样化指数"
              source: "收入拆分"
              extractor: "revenue_herfindahl_index"
              normalize:
                method: "sigmoid"
                params: {scale: -5.0, shift: 0.3}  # 负scale表示反向
              weight: 0.30

  # 中国主体默认约束（可覆盖）
  default_constraints:
    - type: "upper_bound"
      slice: "power"
      endpoint: "narrative"
      value: 0.25
      applies_to: "is_chinese == true"
    
    - type: "lower_bound"
      slice: "epistemic"
      endpoint: "disclosed"
      value: 0.15
      applies_to: "is_chinese == true"
    
    - type: "upper_bound"
      slice: "cascade"
      endpoint: "brittle"
      value: 0.30
      applies_to: "ownership_type == 'SOVEREIGN_FUND' or ownership_type == 'CENTRAL_ENTERPRISE'"

  # 事件类型映射（冲击方向模板）
  event_templates:
    geopolitical_conflict:
      power: [+0.60, -0.10, +0.20, +0.30]
      temporal: [+0.40, +0.10, -0.20, +0.70]
      epistemic: [+0.30, -0.20, +0.40, +0.50]
      cascade: [-0.20, +0.10, +0.60, +0.30]
    
    monetary_policy_shift:
      power: [+0.10, +0.50, -0.05, +0.05]
      temporal: [+0.20, +0.05, +0.10, +0.30]
      epistemic: [+0.10, +0.30, +0.20, +0.10]
      cascade: [+0.30, +0.20, +0.30, -0.10]
    
    commodity_supply_shock:
      power: [+0.05, +0.10, +0.70, +0.15]
      temporal: [+0.30, +0.10, +0.30, +0.30]
      epistemic: [+0.20, +0.10, +0.40, +0.20]
      cascade: [-0.10, +0.15, +0.45, +0.20]
```

### 3.2 切面扩展协议

新增一个切面的操作：

1. 在 `slices.yaml` 中新增 `slice` 条目
2. 定义该切面的 `endpoints` 和每个端点的 `metrics`
3. 运行 `slice_registry.reload()` → 系统自动为所有实体计算新切面的坐标
4. 重新构建 `abstract_matrix`（维度增加）

```python
class SliceRegistry:
    def add_slice(self, slice_config: SliceConfig):
        """热加载新切面，无需停机。"""
        self.slices[slice_config.id] = slice_config
        # 为所有实体异步计算新坐标
        for entity in self.entity_store.all():
            projection = self.compute_slice_projection(entity, slice_config.id)
            entity.slice_projections[slice_config.id] = projection
        # 重建抽象矩阵
        self.rebuild_abstract_matrices()
```

---

## 4. 独立计算协议（Independent Computation Protocol）

### 4.1 切面坐标计算

每个切面的坐标计算完全独立，输入只有**该实体自身的可观测数据**：

```python
def compute_slice_projection(entity: Entity, slice_id: str) -> List[float]:
    """
    独立计算实体在单个切面上的单纯形坐标。
    
    输入: entity.observable_vector（该实体的原始观测数据）
    输出: 单纯形坐标 [α₁, α₂, ..., αₖ]，Σαᵢ = 1, αᵢ ≥ 0
    """
    slice_config = registry.get_slice(slice_id)
    raw_scores = []
    
    for endpoint in slice_config.endpoints:
        # Step 1: 提取并归一化每个指标
        metric_values = []
        for metric in endpoint.metrics:
            raw_value = extract_metric(entity.observable_vector, metric)
            normalized = metric.normalize.apply(raw_value)
            weighted = metric.weight * normalized
            metric_values.append(weighted)
        
        # Step 2: 端点原始分数 = 指标加权和
        endpoint_score = sum(metric_values)
        raw_scores.append(endpoint_score)
    
    # Step 3: 投影到单纯形（Duchi et al. 2008）
    simplex_coords = project_onto_simplex(np.array(raw_scores))
    
    return simplex_coords.tolist()
```

### 4.2 从表格数据到观测向量的映射

原始 CSV 的 7 列需要转换为 `observable_vector`：

```python
CSV_TO_OBSERVABLE_MAPPING = {
    "分类": {
        "A. 宏观对冲基金": {
            "aum_log": {"prior": 2.5, "variance": 1.0},  # 先验均值和标准差
            "strategy_diversity_index": {"prior": 0.7},
            "portfolio_turnover": {"prior": 1.5},
        },
        "B. 军工防务复合体": {
            "govt_contract_revenue_ratio": {"prior": 0.60},
            "state_ownership_ratio": {"prior": 0.0},  # 美国军工不是国企
        },
        # ... 其他分类
    },
    
    "获利方式": {
        "高杠杆做空": {
            "leverage_ratio": {"prior": 5.0},
            "concentration_risk_index": {"prior": 0.30},
        },
        "长期持有": {
            "long_term_investment_ratio": {"prior": 0.80},
            "portfolio_turnover": {"prior": 0.3},
        },
        # ... 关键词映射
    },
    
    "公开追踪渠道": {
        "SEC 13F": {
            "filing_completeness_score": {"prior": 0.90},
            "realtime_data_availability": {"prior": 0.80},
        },
        "私募排排网": {
            "filing_completeness_score": {"prior": 0.30},
            "exchange_position_data_quality": {"prior": 0.60},
        },
        # ... 其他渠道
    },
}
```

**重要**：这些先验只是初始值。当真实指标数据（如 SEC 13F 的精确 AUM）可用时，先验被观测数据覆盖。

---

## 5. 抽象矩阵实体与唯一性

### 5.1 形式化定义

**定义 5.1（抽象矩阵）**：设系统有 S 个切面，第 s 个切面有 Kₛ 个端点，K_max = maxₛ Kₛ。实体 ξ 的抽象矩阵定义为：

> **M_ξ ∈ ℝ^{S×K_max}**
>
> **M_ξ[s, k] = πₛ(ξ)ₖ  if k < Kₛ**
>
> **M_ξ[s, k] = 0       if k ≥ Kₛ**（padding）

其中 πₛ(ξ)ₖ 是 ξ 在切面 s 的第 k 个端点上的单纯形权重。

**定义 5.2（矩阵距离）**：两个实体 ξ₁, ξ₂ 的抽象矩阵距离为 Frobenius 范数：

> **d(ξ₁, ξ₂) = ||M_ξ₁ − M_ξ₂||_F = √[Σₛ Σₖ (M_ξ₁[s,k] − M_ξ₂[s,k])²]**

### 5.2 唯一性定理

**定理 5.1（抽象矩阵单射性）**：在固定切面配置下，若两个实体 ξ₁ ≠ ξ₂ 的**可观测数据**不完全相同（即至少有一个指标的原始值不同），则它们的抽象矩阵不同：M_ξ₁ ≠ M_ξ₂。

**证明概要**：
1. 每个切面的坐标计算 `compute_slice_projection` 是确定性函数
2. 每个端点的分数是指标归一化值的加权和，权重 > 0
3. 若观测数据不同，则至少一个端点的原始分数不同
4. Duchi 单纯形投影是连续单射（在非退化情况下）
5. 因此不同原始分数 → 不同单纯形坐标
6. 拼接后的抽象矩阵必然不同

**推论 5.1**：抽象矩阵空间中的每个点（矩阵）对应**至多一个**具有该观测数据的实体。实体的全局唯一性由 `(id, M_ξ)` 二元组保证。

### 5.3 抽象矩阵的可视化

```python
# 示例：Soros Fund Management 的抽象矩阵
import numpy as np

M_soros = np.array([
    # sovereignty, capital, production, narrative  (POWER 切面)
    [0.10, 0.55, 0.00, 0.35],
    # instant, endurance, recurrence, becoming  (TEMPORAL 切面)
    [0.05, 0.90, 0.03, 0.02],
    # opaque, disclosed, inferred, manipulated  (EPISTEMIC 切面)
    [0.30, 0.40, 0.15, 0.15],
    # elastic, plastic, brittle, absorptive  (CASCADE 切面)
    [0.20, 0.25, 0.35, 0.20],
])

# 与其他实体的距离
M_citadel = np.array([...])
distance = np.linalg.norm(M_soros - M_citadel, 'fro')  # Frobenius 距离
```

---

## 6. 多约束帕累托求解

### 6.1 约束类型

系统支持三类约束：

```yaml
Constraint:
  # 类型 1：端点上下界
  - type: "BOUND"
    slice: "power"
    endpoint: "narrative"
    lower: 0.0
    upper: 0.25
    applies_to: "is_chinese == true"
  
  # 类型 2：切面内线性约束（如"主权+资本 >= 0.5"）
  - type: "LINEAR"
    slice: "power"
    coefficients: {sovereignty: 1.0, capital: 1.0}
    lower: 0.50
    applies_to: "classification == 'B'"
  
  # 类型 3：跨切面约束（如"若权力.叙事 > 0.3，则认知.披露 >= 0.5"）
  - type: "CROSS_SLICE"
    if: "power.narrative > 0.30"
    then: "epistemic.disclosed >= 0.50"
```

### 6.2 帕累托投影算法

```python
def pareto_project(
    raw_coords: Dict[str, List[float]],  # 各切面原始坐标
    constraints: List[Constraint],
    entity_type: str
) -> Dict[str, List[float]]:
    """
    将原始坐标投影到满足所有约束的凸集上。
    
    若约束兼容（可行域非空）：
      求解 min ||x - raw||², s.t. x ∈ C（凸二次规划）
    
    若约束冲突（可行域为空）：
      求解帕累托前沿：最小化最大违反度
      min max_i violation_i(x)
      返回前沿解集，供人工选择或按优先级自动选择
    """
    
    # Step 1: 构建可行域 C
    C = build_feasible_region(constraints)
    
    # Step 2: 检查可行性
    if is_feasible(C):
        # 标准投影（CVXOPT / OSQP）
        return solve_qp(objective=||x - raw||², constraints=C)
    else:
        # 帕累托模式：多目标优化
        # 目标 1: 最小化与原始坐标的距离
        # 目标 2: 最小化约束违反度
        # 使用 ε-约束法或加权求和法
        pareto_set = solve_multiobjective(
            objectives=[dist_to_raw, max_violation],
            constraints=relax(C, epsilon=0.01)
        )
        return select_from_pareto_frontier(pareto_set, strategy="min_distance")
```

### 6.3 中国主体的约束求解示例

原始数据推导：中投公司的 `power.narrative = 0.15`（由于对外投资活动有叙事成分）

约束：
- `power.narrative ≤ 0.25`（中国主体叙事上限）✓ 已满足
- `epistemic.disclosed ≥ 0.15`（监管披露下限）✓ 已满足
- `cascade.brittle ≤ 0.30`（主权基金脆性上限）

原始推导：中投的 `cascade.brittle = 0.05`（极低，因为是吸收型主权基金）
→ 已满足，无需投影。

冲突示例：某中国私募基金的原始推导 `power.narrative = 0.35`（因其大量参与媒体投资），但约束要求 `≤ 0.25`。
→ 帕累托投影将 `narrative` 压至 0.25，多出的 0.10 按比例分配到 `sovereignty` 和 `capital`（因为单纯形约束要求行和为1）。

---

## 7. 跨类型级联推理

### 7.1 类型间的共享切面

| 切面 | ORG 激活 | EVENT 激活 | PERSON 激活 | 说明 |
|------|----------|------------|-------------|------|
| POWER | ✓ | ✓ | ✓ | 通用：组织权力/事件冲击力/个人影响力 |
| TEMPORAL | ✓ | ✓ | ✓ | 通用：时间存在模式 |
| EPISTEMIC | ✓ | ✓（冲击信息透明度） | ✓（个人信息可得性） | 通用 |
| CASCADE | ✓ | ✓（冲击传导模式） | ✓（个人决策风格） | 通用 |
| IMPACT | ✗ | ✓（独有） | ✗ | EVENT 独有：冲击强度与方向 |
| INFLUENCE | ✗ | ✗ | ✓（独有） | PERSON 独有：网络中心度 |

### 7.2 跨类型关系算子

```yaml
RelationType:
  # ORG ↔ ORG
  ORG_CAUSAL_MARKET:
    coupling: "M_power_power + M_cascade_cascade"
    description: "市场因果传导"
  
  ORG_ORGANIZATIONAL:
    coupling: "M_power_power"  # 权重 >= 0.95
    description: "组织强绑定"
  
  # EVENT → ORG
  EVENT_IMPACTS:
    coupling: "M_impact_power + M_impact_cascade"
    description: "事件冲击组织"
    activation: "EVENT.occurred_at within [ORG.valid_from, ORG.valid_until)"
    time_lag: "EVENT.impact_magnitude * 7 days"  # 冲击越强，时滞越短
  
  # PERSON ↔ ORG
  PERSON_AFFILIATION:
    coupling: "M_power_power"  # 人事变动时权重切换
    description: "人事关联"
    activation: "PERSON.affiliation.active_at(time)"
  
  # PERSON → EVENT
  PERSON_TRIGGERS:
    coupling: "M_influence_impact"
    description: "个人触发事件"
    examples: "Soros 公开声明 → 市场波动事件"
```

### 7.3 级联算法（跨类型版）

```python
def cross_type_cascade(
    event: EVENT,
    max_hops: int,
    theta: float
) -> CascadeGraph:
    """
    跨类型级联：EVENT 作为瞬时实体注入，
    通过共享切面（POWER, TEMPORAL, EPISTEMIC, CASCADE）与 ORG/PERSON 交互。
    """
    
    # Step 1: EVENT 越界坐标（允许超出单纯形）
    event_coords = event.get_impact_vector()  # 非归一化冲击向量
    
    # Step 2: 找到与 EVENT 在 POWER 切面上距离最近的 ORG
    candidates = entity_store.query(
        type="ORG",
        slice="power",
        k_nearest_to=event_coords["power"],
        k=50
    )
    
    # Step 3: 对每个候选 ORG，计算冲击敏感度
    for org in candidates:
        sensitivity = np.dot(org.abstract_matrix[power_idx], event_coords["power"])
        if sensitivity > theta:
            cascade_graph.add_edge(event, org, weight=sensitivity)
            queue.push((org, hop=1, conf=sensitivity))
    
    # Step 4: 标准 BFS 级联展开（同 v2.0，但支持跨类型）
    while queue:
        node, hop, conf = queue.pop()
        if hop >= max_hops or conf < theta:
            continue
        
        for relation in graph.relations_from(node):
            next_node = relation.to
            next_conf = conf * relation.coupling_strength(event_coords)
            
            # 类型特异性衰减
            if next_node.type == "PERSON":
                next_conf *= 0.85  # 个人决策不确定性更高
            elif next_node.type == "EVENT":
                next_conf *= 0.90  # 事件触发事件（连锁反应）
            
            if next_conf >= theta:
                cascade_graph.add_edge(node, next_node, hop+1, next_conf)
                queue.push((next_node, hop+1, next_conf))
    
    return cascade_graph
```

---

## 8. 单/多切面查询协议

### 8.1 查询模式

```python
class QueryEngine:
    def query(self, entity_id: str, mode: QueryMode, **kwargs):
        entity = self.store.get(entity_id)
        
        if mode == QueryMode.SINGLE_SLICE:
            # 返回单个切面的投影 + 同切面最近邻居
            slice_id = kwargs["slice"]
            coords = entity.slice_projections[slice_id]
            neighbors = self.store.knn(
                slice=slice_id,
                coords=coords,
                k=kwargs.get("k", 10),
                filter=kwargs.get("filter")
            )
            return SingleSliceResult(coords, neighbors)
        
        elif mode == QueryMode.MULTI_SLICE:
            # 返回多切面联合投影（PCA/t-SNE 降维）
            slices = kwargs["slices"]
            sub_matrix = entity.abstract_matrix[slices, :]
            projection = self.dimensionality_reduce(sub_matrix, method="pca")
            
            # 计算多切面联合距离
            neighbors = self.store.knn_matrix(
                matrix=entity.abstract_matrix,
                metric="frobenius",
                k=kwargs.get("k", 10)
            )
            return MultiSliceResult(projection, neighbors)
        
        elif mode == QueryMode.ABSTRACT_MATRIX:
            # 返回完整抽象矩阵 + 全空间最近邻居
            return AbstractMatrixResult(
                matrix=entity.abstract_matrix,
                flat_vector=entity.abstract_matrix.flatten(),
                neighbors=self.store.knn_matrix(
                    matrix=entity.abstract_matrix,
                    metric="frobenius",
                    k=kwargs.get("k", 20)
                )
            )
        
        elif mode == QueryMode.PARETO_FRONTIER:
            # 返回约束冲突时的帕累托前沿
            constraints = kwargs["constraints"]
            raw = entity.get_raw_projections()
            frontier = self.pareto_solver.solve(raw, constraints)
            return ParetoResult(
                original=raw,
                frontier=frontier,
                selected=self.pareto_solver.select(frontier, strategy="min_distance")
            )
        
        elif mode == QueryMode.CROSS_TYPE:
            # 跨类型最近邻居
            target_type = kwargs["target_type"]  # "ORG" | "EVENT" | "PERSON"
            shared_slices = kwargs.get("slices", ["power", "temporal"])
            neighbors = self.store.knn_cross_type(
                entity=entity,
                target_type=target_type,
                shared_slices=shared_slices,
                k=kwargs.get("k", 10)
            )
            return CrossTypeResult(neighbors)
```

### 8.2 查询示例

```python
# 示例 1：单切面——Soros Fund 在权力拓扑上的位置
result = engine.query(
    "soros_fund_mgmt",
    mode=QueryMode.SINGLE_SLICE,
    slice="power",
    k=5
)
# 返回：Soros Fund 的权力坐标 + 最近 5 个权力结构相似的实体

# 示例 2：多切面——中投公司在权力+级联联合空间中的位置
result = engine.query(
    "cic_china",
    mode=QueryMode.MULTI_SLICE,
    slices=["power", "cascade"],
    k=10
)
# 返回：PCA 降维后的二维投影 + 最近 10 个实体

# 示例 3：抽象矩阵全空间——找与敦和资管整体最相似的实体
result = engine.query(
    "dunhe_asset",
    mode=QueryMode.ABSTRACT_MATRIX,
    k=20
)
# 返回：完整 4×4 矩阵 + 按 Frobenius 距离排序的 20 个最近邻居

# 示例 4：帕累托——某中国私募在约束下的最优坐标
result = engine.query(
    "some_chinese_pe",
    mode=QueryMode.PARETO_FRONTIER,
    constraints=[
        Constraint(BOUND, "power", "narrative", upper=0.25),
        Constraint(BOUND, "cascade", "brittle", upper=0.30),
    ]
)
# 返回：原始坐标 vs 约束后坐标 + 帕累托前沿

# 示例 5：跨类型——George Soros（PERSON）最可能影响哪些 EVENT
result = engine.query(
    "george_soros",
    mode=QueryMode.CROSS_TYPE,
    target_type="EVENT",
    shared_slices=["power", "narrative"],
    k=10
)
# 返回：与 Soros 权力+叙事坐标最接近的 10 个历史事件
```

---

## 9. 生产部署架构

### 9.1 数据流

```
┌──────────────────────────────────────────────────────────────┐
│                     Data Sources                             │
│  CSV (手工) | SEC API | 交易所 | 新闻 | 社交媒体 | 海关数据    │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                  Observable Ingestion Layer                  │
│  - Metric Extractors (可插拔，每类指标一个 extractor)         │
│  - Normalization Pipeline (sigmoid / min_max / log / identity)│
│  - Provenance Tracker (审计轨迹)                              │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│              Slice Projection Engine (并行)                   │
│  - 每个切面独立计算，无跨切面依赖                              │
│  - 支持批量重算（切面扩展时）                                  │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│              Pareto Projection (约束求解)                     │
│  - 若约束冲突：返回前沿解集                                    │
│  - 若无冲突：标准凸二次规划投影                                │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│              Abstract Matrix Builder                          │
│  - S × K_max 矩阵拼接                                         │
│  - 可选：低秩压缩（SVD 保留 90% 能量）                        │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│              Storage Layer                                    │
│  PostgreSQL: Entity Registry + Versioned Fact Store          │
│  DuckDB:     Abstract Matrix + KNN 索引 (IVF_FLAT)           │
│  Redis:      热点矩阵缓存                                     │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│              Query / Cascade API (FastAPI)                    │
│  - /query (单/多切面/抽象矩阵/帕累托/跨类型)                  │
│  - /cascade (事件注入 → 级联图)                               │
│  - /backtest (历史回测)                                       │
│  - /slice_registry (切面管理)                                 │
└──────────────────────────────────────────────────────────────┘
```

### 9.2 关键性能指标

| 指标 | 目标 | 说明 |
|------|------|------|
| 切面计算延迟 | < 50ms/实体 | 单个切面的独立计算 |
| 抽象矩阵构建 | < 200ms/实体 | 全切面拼接 + 帕累托投影 |
| KNN 查询 | < 100ms | DuckDB IVF 索引，k=20 |
| 级联推理 | < 2s/事件 | 5 跳展开，183 实体全图 |
| 切面热加载 | < 30s | 全量实体重算新切面坐标 |

---

## 10. 开发路线图（修订版）

| Phase | 周数 | 目标 | 验收标准 |
|-------|------|------|----------|
| **P0** | 1 | 数据清洗 + Observable 向量化 | 183 实体全部有 `observable_vector`，CSV 7 列映射完成 |
| **P1** | 2-3 | SliceRegistry + 独立计算协议 | 4 切面 × 4 端点 × 3-4 metrics/端点，配置驱动，切面可热加载 |
| **P2** | 4-5 | 抽象矩阵 + 帕累托求解 | 所有实体有 `abstract_matrix`，约束求解器通过单元测试 |
| **P3** | 6-7 | 跨类型统一（EVENT/PERSON 接入） | EVENT 作为瞬时实体可注入，PERSON 有 affiliations 时序管理 |
| **P4** | 8-9 | 级联引擎 + 查询协议 | Σ-CASCADE 跨类型版，5 种 QueryMode 全部可用 |
| **P5** | 10-11 | 回测验证 | 10+ 历史事件回测，Precision@10 > 0.6，HopAccuracy < 1.5 |
| **P6** | 12 | 生产部署 | API + K8s + 监控，P99 延迟达标 |

---

## 11. 下一步行动

1. **确认切面/端点/指标设计**：§3 的 `slices.yaml` 是否覆盖您的需求？有无遗漏的端点或指标？
2. **确认跨类型设计**：EVENT 的 `IMPACT` 切面和 PERSON 的 `INFLUENCE` 切面是否必要？还是所有类型共享同一套 4 切面即可？
3. **数据源优先级**：183 实体中，哪些有真实的 SEC 13F/年报数据可提取，哪些只能依赖 CSV 先验？这影响 `observable_vector` 的填充策略。
4. **帕累托策略**：约束冲突时，优先「最小距离」（保真原始数据）还是「最小违反度」（保真约束）？

---

*本文档为 v3.0 架构设计。后续随着 183 实体的 observable_vector 填充和切面坐标计算，将持续迭代。*
