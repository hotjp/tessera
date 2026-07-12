# task_003

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_003.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T2] Duchi 单纯形投影 + Frobenius 距离 (SPEC §2.1 §2.4)


## 需求 (requirements)

实现 project_onto_simplex(v: &[f32], k: usize) -> Vec<f32>(Duchi 投影,O(k log k) 由排序主导;输入可为任意实数含负数/大于1,不假设已归一化;投影后保持 k 维,退化权重 0 不移除维度)。实现 frobenius_distance(a,b,slice_dims: &[u8]) -> f32: 仅对每切面前 k 项求差平方和开方,padding 列(i>=k)显式跳过。NaN 处理用 unwrap_or(Equal),注释说明调用方需保证无 NaN。



## 验收标准 (acceptance)


- cargo test simplex 通过

- 投影后行和=1 容差1e-5

- 投影后全>=0

- 投影任意实向量含负数合法

- cargo test frobenius: padding 不同值距离仍<1e-6




## 交付物 (deliverables)

- `src/simplex.rs`(project_onto_simplex / frobenius_distance + 测试)



## 设计方案 (design)

src/simplex.rs。排序 sort_by partial_cmp unwrap_or(Equal)。退化维度只置 0 不降维。


## 验证证据（完成前必填）

<!-- 标记完成前，请提供以下证据： -->

- [x] **实现证明**: src/simplex.rs。project_onto_simplex 用 Duchi O(k log k) 算法（降序排序→找 rho 阈值 theta→max(v-theta,0)），输入任意实数(含负/>1)，输出保持 k 维、退化 0 不降维，排序 partial_cmp.unwrap_or(Equal)。frobenius_distance 逐切面仅累加前 slice_dims[s] 项差平方和，padding 列(i>=k)显式跳过。
- [x] **测试验证**: `cargo test simplex` → 9 passed；`cargo test frobenius` → 3 passed；clippy 无告警；fmt 通过。
- [x] **影响范围**: 纯新增数学工具，无副作用；后续 task_005(查询回放后投影)/cascade 可复用。

### 测试步骤
1. `cargo test simplex` → 9/9 ok（行和=1 容差1e-5、全>=0、含负数合法、>1合法、归一化不动点、退化保持维度、长输入取前k）
2. `cargo test frobenius` → 3/3 ok（相同=0、padding差异忽略<1e-6、单点差已知值）
3. `cargo clippy --all-targets` + `cargo fmt --check` → clean

### 验证结果
- Duchi 投影：[1,1,1]→[1/3,1/3,1/3]；[-1,5,-3,2]→和=1全>=0；[5,0,0]→[1,0,0]保持3维
- Frobenius：前k列同/padding 不同 → 距离<1e-6（padding 显式跳过生效）
- LRA 质量检查：测试+lint+验收 全过（set completed 时自动跑）