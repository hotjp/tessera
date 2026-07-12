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

- [ ] **实现证明**: 简要说明如何实现
- [ ] **测试验证**: 如何验证功能正常（测试步骤/截图/命令输出）
- [ ] **影响范围**: 是否影响其他功能

### 测试步骤
1. 
2. 
3. 

### 验证结果
<!-- 粘贴验证截图、命令输出或测试结果 -->