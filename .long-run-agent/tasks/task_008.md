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

- [x] **实现证明**: 按 ADR-001（覆盖任务 requirements 正文的 x86 intrinsics 方案）实现可移植 SIMD。lib.rs 顶部加 `#![feature(portable_simd)]`；matrix.rs 加 `spmv_csr`：LANES=8，values 用 `Simd::from_slice` 连续装入、x 按 col_idx 用 `core::array::from_fn` 聚集入 `Simd::from_array`、向量乘 + `reduce_sum` 水平归约，尾部标量收尾。`std::simd` 编译期自动降级（aarch64→NEON/SVE，x86_64→AVX2/AVX-512），**不用 `is_x86_feature_detected!`**（ADR-001 明确禁用）；标量 fallback/真值参照复用 task_007 `spmv_csr_scalar`。
- [x] **测试验证**: `cargo test spmv` → 6 passed（含 simd_matches_scalar_all_paths：17/8/3 nz 行覆盖 2块+尾/1块/纯尾）；全套 39 passed；clippy 无告警；fmt 通过。本机 darwin/aarch64 编译运行通过（SIMD→NEON）。
- [x] **影响范围**: lib.rs 加 unstable feature gate（nightly 钉版已在 rust-toolchain.toml，升级需跑全量回归）；matrix.rs 追加 spmv_csr 不改既有 API。级联推理(后续)可调用 spmv_csr。

### ⚠️ 与 requirements 正文的偏离（ADR-001 授权）
requirements 正文要求 `is_x86_feature_detected!(avx512f/avx2)` + target_feature intrinsics 分发；**ADR-001 覆盖此方案**：改用 nightly `std::simd` 可移植 SIMD，禁用 x86 专属 intrinsic 与运行时检测。deliverables 正文（「基于 nightly std::simd/portable_simd」「src/lib.rs 顶部 #![feature(portable_simd)]」）与本实现一致。

### 测试步骤
1. `cargo test spmv` → 6/6 ok（SIMD==标量<1e-5，覆盖多块/单块/纯尾路径）
2. `cargo clippy --all-targets` → 无告警
3. `cargo fmt --check` → exit 0

### 验证结果
- SIMD 路径(spmv_csr) == 标量(spmv_csr_scalar) 逐元素误差<1e-5（17nz 行：2 SIMD 块 + 1 标量尾）
- aarch64 darwin 编译运行通过（portable_simd 自动降级为 NEON，不依赖 AVX-512）
- 标量 fallback 始终存在（spmv_csr_scalar）