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

- [x] **实现证明**: src/constants.rs(MAX_SLICES=16/K_MAX=8/MAX_ENTITIES=65536/DELTA_RING_CAPACITY=1024) + src/entity.rs(Entity repr(C,align64)/DeltaEvent repr(C,packed)/SteadyState/Relation repr(C))。字段顺序严格按需求，无 String/Vec，valid_from/until 用 i64，name_ptr 用 *const u8。坐标语义 [切面][端点]。lib.rs 挂 pub mod constants/entity。
- [x] **测试验证**: `cargo test entity_layout` → 6 passed；clippy 无告警；cargo fmt --check 通过。
- [x] **影响范围**: 纯新增布局类型，无方法，不改既有行为；task_003(query/apply_delta)/task_004+(依赖这些类型)向后兼容。

### 测试步骤
1. `cargo test entity_layout` → 6/6 ok
2. `cargo clippy --all-targets` → 无告警
3. `cargo fmt --check` → exit 0

### 验证结果
- Entity align=64, size=17024(≤65536)，header 偏移 id@0/valid_from@8/valid_until@16/coordinates@24(repr(C) 确定)
- DeltaEvent size=16, align=1(packed)
- SteadyState size=10, align=2
- Relation repr(C) 可构造，align=8(含 i64)
- LRA 质量检查：测试通过 + lint 通过 + 验收满足（set completed 时自动跑）