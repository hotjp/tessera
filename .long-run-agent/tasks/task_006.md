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

- [x] **实现证明**: entity.rs 追加 EntitySnapshot(全栈分配) + impl Entity{apply_delta_singlethreaded(写 ring[head%CAP]+head++，head-tail>=CAP panic), query_state(深拷贝基态→回放 ring[tail..head] 中 ts<=query 的 delta，slice=mask.trailing_zeros()→对每切面 Duchi 投影)}。为满足「查询无堆分配」，重构 simplex.rs：抽 duchi_inplace 核心 + 新增 project_onto_simplex_inplace(栈 scratch [f32;K_MAX])，project_onto_simplex(Vec) 改为薄封装（task_003 行为不变）。DeltaEvent 加 #[derive(Clone,Copy)] 以支持环数组初始化。
- [x] **测试验证**: `cargo test state_query` → 7 passed（含 <500ns 无delta、<2μs 100delta、环溢出 panic、早/晚查询边界、slice_mask 定位）；全套 22 passed；clippy 无告警；fmt 通过。
- [x] **影响范围**: simplex.rs 重构对外 API 兼容（task_003 测试不变）；entity.rs 追加方法不破坏布局（task_002 断言仍绿）。下游级联/网络层可复用 query_state。

### 测试步骤
1. `cargo test state_query` → 7/7 ok
2. `cargo clippy --all-targets` → 无告警
3. `cargo fmt --check` → exit 0

### 验证结果
- 无delta查询 <500ns（实测远低，空环仅栈拷贝）
- 100 delta 回放净0→[0.5,0.5] 正确，<2μs
- 环填满 1024 → 第 1024 次 panic（catch_unwind 捕获）
- 早查询(50<100)→基态；晚查询(MAX)→回放全部+投影
- slice_mask=1<<2 → 切面2 [1.0,0.5]→投影→[0.75,0.25]
- 查询路径零堆分配（EntitySnapshot 栈分配 + 投影栈 scratch）