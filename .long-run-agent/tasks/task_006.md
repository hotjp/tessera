# task_006

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_006.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T5] 增量环写入 + query_state (SPEC §3.2 §6.1)


## 需求 (requirements)

impl Entity::apply_delta_singlethreaded(&mut self, delta: DeltaEvent): SPSC 单线程,写入 delta_ring[ring_head%1024],ring_head+=1;若 ring_head-ring_tail>=1024 则 panic(需要快照刷盘)。query_state(&self, query_time_us: u64) -> EntitySnapshot: 深拷贝基态 coordinates(禁止返回引用),回放 ring tail..head 中 timestamp_us<=query_time 的 delta(按 slice_mask 的 trailing_zeros 定位切面),回放后对每行做 Duchi 投影。查询函数内禁止堆分配(EntitySnapshot 栈分配)。



## 验收标准 (acceptance)


- cargo test state_query 通过

- 无delta返回基态且<500ns

- 100 delta回放正确且<2μs

- 环溢出 panic

- 查询早于所有delta返回基态

- 晚于返回基态+全部delta




## 交付物 (deliverables)

- `src/entity.rs`(追加 impl Entity: apply_delta_singlethreaded / query_state / EntitySnapshot + 测试)



## 设计方案 (design)

src/entity.rs(追加 impl)。EntitySnapshot{coords,slice_dims,num_slices} 全栈分配。投影复用 simplex::project_onto_simplex。默认单线程版本,不引入原子操作。


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