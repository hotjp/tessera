# task_008

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_008.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T7] SIMD SpMV + 运行时硬件检测 fallback (SPEC §6.2 SIMD)


## 需求 (requirements)

spmv_csr(x, matrix, y) 运行时分发:cfg(target_arch=x86_64) 下 is_x86_feature_detected!(avx512f/avx2) 走 target_feature intrinsics;cfg(target_arch=aarch64) 走 NEON 或退化为标量;任何平台必有标量 fallback。开发机为 Apple Silicon(arm64),AVX 路径必须 cfg 门控,保证 arm64 上 cargo test 可编译可运行(此时以标量/NEON 为准)。不直接用不稳定 std::simd 全局启用。



## 验收标准 (acceptance)


- cargo test spmv 全绿

- SIMD 路径结果==标量误差<1e-5

- 本机 darwin/arm64 可编译可测

- 不假设 AVX-512 存在

- 必有标量 fallback




## 交付物 (deliverables)

- `src/matrix.rs`(追加 spmv_csr,基于 nightly std::simd/portable_simd + 测试)
- `src/lib.rs` 顶部 `#![feature(portable_simd)]`
- 标量 oracle 复用 task_007 的 `src/matrix.rs`



## 设计方案 (design)

src/matrix.rs(追加)。x86_64: #[target_feature(enable="avx512f")] unsafe + is_x86_feature_detected! 分发;aarch64: NEON 或保守标量;标量 fallback 始终存在。⚠️ SIMD 目标平台决策点见任务正文(arm64 vs x86),默认按可移植方案。


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