# Σ⁴-Engine 架构决策记录 (DECISIONS.md)

> **文档性质**:对 `SIGMA4_SPEC_v1_1.md` 的正式偏离与补充决策。
> **优先级**:本文件 **>** SPEC 历史条款。AI Agent 与审阅者在遇到冲突时,**以本文件为准**。
> **创建时间**:2026-07-12

---

## ADR-001: SIMD 内核采用 nightly + `std::simd` 可移植 SIMD

**状态**:✅ 已采纳(覆盖 SPEC §6.2 与 附录B 决策树相关条款)

### 背景
- SPEC §6.2 与附录B决策树要求「`target_feature` + x86 intrinsic + 标量 fallback」,并明确警告「不要直接写 `std::simd`(不稳定)」。
- 原任务 `task_008` 设计为 x86_64 AVX-512/AVX2 intrinsics + `is_x86_feature_detected!!` 运行时分发。
- 用户要求系统具备**跨平台**能力(见 ADR-002 的 4 个目标平台)。在 4 个架构上维护两套手写 intrinsic(x86_64 + aarch64)成本与出错率高于接受一份可移植代码。

### 决策
- SIMD 内核改用 **nightly Rust + `#![feature(portable_simd)]` + `std::simd`**(如 `std::simd::Simd<f32, 16>` + gather)实现 SpMV。
- **一份代码**自动降级到各平台 SIMD:x86_64 → AVX-512/AVX2,aarch64 → NEON/SVE。
- **不再使用**任何 `std::arch::x86_64::*` 专属 intrinsic 与 `is_x86_feature_detected!`。
- `task_007` 的**标量 SpMV 保留**,作为正确性 oracle;测试断言 SIMD 路径结果 == 标量(误差 < 1e-5)。

### 理由
4 个目标平台用一份可移植代码,优于手写并维护 x86_64 + aarch64 两套 intrinsic 内核;`portable_simd` 成熟度已可承担本工作负载。

### 后果 / 代价
- 工具链必须钉 nightly(`rust-toolchain.toml`:带日期的 nightly + `rust-src` 组件),由 `task_001` 产出。
- 4 平台 CI 均需安装该固定 nightly(`task_012`)。
- `portable_simd` 仍为 unstable:钉版本后风险可控,但**升级 nightly 前必须跑全量回归**。
- **性能不受影响**:目标负载(100 实体 / 5 跳稀疏 SpMV)标量即可 < 100μS,SIMD 非承重墙——跨平台不以性能为代价。

### 覆盖范围
`task_008`(实现)、`task_001`(工具链)、`task_012`(CI)。

---

## ADR-002: 目标平台矩阵

**状态**:✅ 已采纳

### 决策
正式支持的构建 / 测试平台:
1. **aarch64 macOS**(Apple Silicon,开发机)
2. **x86_64 Linux**(服务器部署)
3. **aarch64 Linux**(ARM 服务器 / AWS Graviton)
4. **Windows**(x86_64)

### 后果
- CI 矩阵须覆盖上述 4 平台(`task_012`)。
- 所有文件路径(TLS 证书 / 日志 / 快照)**必须用 `std::path::PathBuf`**,禁止硬编码 POSIX 路径如 `/var/lib/...`(影响 `task_010` 及快照层)。
- 传输层 **rustls + tokio** 在 4 平台原生可用(Windows 异步 IO 走 IOCP),无需额外适配。
- 线协议 `to_be_bytes`(大端)与快照 `to_le_bytes`(小端)在上述架构上一致,无需调整。

---

## 与 SPEC 的冲突索引

| SPEC 条款 | SPEC 原意 | 本文件覆盖为 | 影响任务 |
|-----------|----------|-------------|---------|
| §6.2 / 附录B 决策树 | 勿用 `std::simd`,用 `target_feature`+标量 | nightly `std::simd` 可移植 | task_008 |
| §10 环境变量(POSIX 路径) | `/var/lib/...` 硬编码 | 改 `PathBuf`,跨平台 | task_010 |
| 附录C 骨架 Cargo.toml | 未含 nightly 工具链 | 增加 `rust-toolchain.toml` | task_001 |

---

*本文件为活文档。任何对 SPEC 的新偏离均应在此追加 ADR,不得仅在代码或对话中隐式存在。*
