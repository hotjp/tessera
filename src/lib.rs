//! Σ⁴-Engine — 确定性微秒级稀疏状态级联推理引擎。
//!
//! 热路径核心 crate。实现顺序见 `docs/CLAUDE.md` 第 3 节：
//! 内存布局 → 单纯形投影 → 增量环 → 稀疏矩阵 → 级联推理 → 网络层。
//!
//! - 唯一现行规范：[`docs/SIGMA4_SPEC_v1_1.md`](../docs/SIGMA4_SPEC_v1_1.md)
//! - 覆盖性决策：[`docs/DECISIONS.md`](../docs/DECISIONS.md)（与 SPEC 冲突时以此为准）
//!
//! 模块按 SPEC §3 逐步落地：
//! - `constants`：编译期常量（内存预算与拓扑边界）
//! - `entity`：Entity / DeltaEvent / SteadyState / Relation 内存布局

pub mod constants;
pub mod entity;
pub mod simplex;
