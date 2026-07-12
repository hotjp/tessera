# task_009

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_009.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T8] 级联推理(含脆性阈值) (SPEC §6.3 §2.5)


## 需求 (requirements)

定义 EntityStateView { coordinates, brittle_threshold: f32, decay_coefficient: f32, time_lag_us: u32 } 与 CascadeResult { entity_id, confidence, hop, lag_us }。cascade(initial, matrix, entity_states, max_hops, theta): 每跳 spmv_csr;脆性实体(brittle 坐标>0.5)未突破阈值时 impact*0.5^hop 快速衰减,突破 brittle_threshold 后不衰减(置信度透传);非脆性 impact*decay_coefficient;置信度<theta 剪枝置 0;记录首次命中跳数与累积 time_lag。



## 验收标准 (acceptance)


- cargo test cascade 通过

- 星型拓扑叶子收到正确衰减信号

- 脆性突破时 confidence=1.0

- 脆性未突破时快速衰减

- lag 正确累积

- 100实体5跳<100μs release模式




## 交付物 (deliverables)

- `src/cascade.rs`(EntityStateView/CascadeResult/cascade + 测试 + bench)



## 设计方案 (design)

src/cascade.rs。复用 matrix::spmv_csr。脆性分支(brittle_coord>0.5)必须存在,不可统一指数衰减。


## 验证证据（完成前必填）

<!-- 标记完成前，请提供以下证据： -->

- [x] **实现证明**: 新建 src/cascade.rs。EntityStateView{coordinates,brittle_threshold,decay_coefficient,time_lag_us} + CascadeResult{entity_id,confidence,hop,lag_us}。cascade: hop0 处理初始信号，每跳 spmv_csr 传播 → apply_hop 计算置信度（confidence_for：脆性 coordinates>0.5 时 raw>=brittle_threshold→1.0 透传，否则 raw·0.5^hop 快衰减；非脆性 raw·decay_coefficient），conf<theta 剪枝置0，记录首次命中跳数+累积 time_lag。双缓冲(cur/next)+raw_buf 避免逐跳分配。
- [x] **测试验证**: `cargo test cascade` → 6 passed；`cargo test --release cascade_100` perf 通过(<100μs)；全套 45 passed；clippy 无告警；fmt 通过。
- [x] **影响范围**: 新增级联模块，复用 matrix::spmv_csr(SIMD)；不改既有 API。网络层(后续)可暴露 CascadeRun。

### 测试步骤
1. `cargo test cascade` → 6/6 ok
2. `cargo test --release cascade_100_entities_5_hops_under_100us` → ok（release <100μs）
3. `cargo clippy --all-targets` → 无告警；`cargo fmt --check` → exit 0

### 验证结果
- 星型拓扑：中心 conf=1.0@hop0；叶子 conf=0.5·decay=0.5@hop1（w=0.5,decay=1.0）
- 脆性突破(raw=0.5>=thr=0.3)→confidence=1.0
- 脆性未突破(thr=0.8)：0.5·0.5^1=0.25 < 非脆性 0.5·0.9=0.45（快衰减）
- 2-环 lag 累积：e0 命中 hop0+hop2→20，e1 命中 hop1+hop3→40
- conf<theta(0.05<0.1)剪枝，叶不出现在结果
- 100实体5跳 release <100μs（实测远低）