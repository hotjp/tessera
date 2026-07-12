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

- [ ] **实现证明**: 简要说明如何实现
- [ ] **测试验证**: 如何验证功能正常（测试步骤/截图/命令输出）
- [ ] **影响范围**: 是否影响其他功能

### 测试步骤
1. 
2. 
3. 

### 验证结果
<!-- 粘贴验证截图、命令输出或测试结果 -->