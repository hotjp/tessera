# task_007

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_007.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T6] CascadeMatrix(CSR) + 标量 SpMV (SPEC §6.2 scalar)


## 需求 (requirements)

定义 CascadeMatrix { n: u32, row_ptr: Vec<u32>, col_idx: Vec<u32>, values: Vec<f32>, time_lag_us: Vec<u32> }。实现 spmv_csr_scalar(x: &[f32], matrix: &CascadeMatrix, y: &mut [f32]): 标准 CSR 行点积,y[i]=Σ values[k]*x[col_idx[k]] for k in row_ptr[i]..row_ptr[i+1]。作为 SIMD 路径的真值参照与最终 fallback。



## 验收标准 (acceptance)


- cargo test spmv 通过

- 标量结果与稠密计算逐元素误差<1e-5

- 空行无非零结果0

- 单元素行正确




## 交付物 (deliverables)

- `src/matrix.rs`(CascadeMatrix / spmv_csr_scalar / from_edges + 测试)



## 设计方案 (design)

src/matrix.rs。先只实现标量;SIMD 留给 T7。构造辅助 fn from_edges 构建 CSR。


## 验证证据（完成前必填）

<!-- 标记完成前，请提供以下证据： -->

- [x] **实现证明**: 新建 src/matrix.rs。CascadeMatrix{n,row_ptr(n+1),col_idx,values,time_lag_us}。from_edges 用计数+前缀和+游标填充构建 CSR（非法 from>=n 边丢弃）。spmv_csr_scalar 逐行点积 y[i]=Σ values[k]·x[col_idx[k]]（row_ptr.windows(2) zip y.iter_mut，无下标循环）。仅标量实现，作为 task_008 SIMD 的真值参照与 fallback（ADR-001）。
- [x] **测试验证**: `cargo test spmv` → 5 passed；全套 38 passed；clippy 无告警；fmt 通过。
- [x] **影响范围**: 纯新增矩阵模块；不改既有 API。task_008(SIMD) 将断言 SIMD 结果 == 标量(误差<1e-5)；级联推理(task_009+)复用 CascadeMatrix。

### 测试步骤
1. `cargo test spmv` → 5/5 ok
2. `cargo clippy --all-targets` → 无告警
3. `cargo fmt --check` → exit 0

### 验证结果
- 标量 vs 稠密逐元素误差<1e-5（[1.9,2.4,0.1]）
- 空行(无非零)→ y=0
- 单元素行 0.7·x[1]=2.8 正确
- from_edges 字段一致（row_ptr=[0,1,2], col_idx/values/time_lag_us 保序）
- 非法 from 边被丢弃