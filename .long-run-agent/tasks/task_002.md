# task_002

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_002.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T1] Entity/DeltaEvent/Relation 内存布局 (SPEC §3.1 §3.4)


## 需求 (requirements)

定义常量 MAX_SLICES=16 / K_MAX=8 / MAX_ENTITIES=65536 / DELTA_RING_CAPACITY=1024(编译期常量)。定义 Entity(repr(C,align(64))): id u32 / entity_type u8 / flags u8 / num_slices u8 / _pad0 u8 / valid_from i64 / valid_until i64 / coordinates [[f32;K_MAX];MAX_SLICES] / slice_dims [u8;MAX_SLICES] / delta_ring [DeltaEvent;1024] / ring_head u32 / ring_tail u32 / steady_state SteadyState / name_ptr *const u8 / name_len u16 / _pad1 [u8;6]。DeltaEvent(repr(C,packed)): timestamp_us u64 / slice_mask u16 / endpoint_idx u8 / delta_value f32 / _pad u8。SteadyState(repr(C)): geography u16 / industry u16 / ownership_type u8 / _pad [u8;5]。Relation(repr(C)): from_id u32 / to_id u32 / relation_type u8 / weight f32 / time_lag_us u32(微秒) / valid_from i64 / valid_until i64。严格保持字段声明顺序(repr(C) 依赖)。不引入 String/Vec 到 Entity。valid_from/until 用 i64(历史时间可能为负)。



## 验收标准 (acceptance)


- cargo test entity_layout 全绿

- Entity 对齐=64

- Entity 大小<=65536

- DeltaEvent 大小=16

- Relation repr(C) 布局可编译




## 交付物 (deliverables)

- `src/constants.rs`
- `src/entity.rs`(Entity/DeltaEvent/SteadyState/Relation + 布局断言)



## 设计方案 (design)

src/constants.rs + src/entity.rs。仅数据结构与 #[cfg(test)] 布局断言,无方法。坐标二维数组语义=[切面行][端点列]。


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