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

- [ ] **实现证明**: 简要说明如何实现
- [ ] **测试验证**: 如何验证功能正常（测试步骤/截图/命令输出）
- [ ] **影响范围**: 是否影响其他功能

### 测试步骤
1. 
2. 
3. 

### 验证结果
<!-- 粘贴验证截图、命令输出或测试结果 -->