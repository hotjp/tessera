# task_005

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_005.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T4] 帕累托约束投影 (SPEC §2.5)


## 需求 (requirements)

定义 Constraint { slice, endpoint, value, kind } 与 ConstraintKind { LowerBound, UpperBound, Linear }(Linear 含 coefficients/target)。pareto_project(raw, constraints, slice_dims): 固定顺序应用约束 lower -> upper -> linear,之后对每行重新执行 Duchi 投影(约束后行和可能!=1,必须重投影)。



## 验收标准 (acceptance)


- cargo test pareto 通过

- 约束后每行和=1

- lower/upper 约束 violation=0

- 固定顺序 lower 先于 upper

- 约束后必重投影




## 交付物 (deliverables)

- `src/constraint.rs`(Constraint/ConstraintKind/pareto_project + 测试)



## 设计方案 (design)

src/constraint.rs。复用 simplex::project_onto_simplex。约束顺序用三次 filter 迭代保证固定。


## 验证证据（完成前必填）

<!-- 标记完成前，请提供以下证据： -->

- [x] **实现证明**: 新建 src/constraint.rs。Constraint{slice,endpoint,value,kind} + ConstraintKind{LowerBound,UpperBound,Linear{coefficients,target}}。拆 apply_constraints（私有不重投影，便于测顺序/clamp）+ pareto_project（调用前者后对每行 Duchi 重投影）。固定顺序用三次 filter 迭代保证（lower→upper→linear）。Linear 用正交投影到超平面 Σ coeff·coord=target（step=(target-dot)/||coeff||²）。
- [x] **测试验证**: `cargo test pareto` → 6 passed；全套 33 passed；clippy 无告警；fmt 通过。
- [x] **影响范围**: 纯新增约束投影模块，复用 simplex::project_onto_simplex_inplace；不改既有 API。级联推理(后续)可在投影后施加业务约束。

### 测试步骤
1. `cargo test pareto` → 6/6 ok
2. `cargo clippy --all-targets` → 无告警
3. `cargo fmt --check` → exit 0

### 验证结果
- lower/upper clamp 正确（0.2→0.5 / 0.9→0.3）
- 固定顺序：同一端点 lower=0.5 后 upper=0.3 → 0.3（upper 后应用胜出）
- Linear：[0.5,0.5]·[1,1]=1.0 → 投影到 0.8，误差<1e-6
- 重投影：upper 破坏行和后 pareto_project 恢复 Σ=1（row0/row1 均为 1）
- 相容界（lower=0/upper=1，单纯形天然范围）violation=0 + Σ=1