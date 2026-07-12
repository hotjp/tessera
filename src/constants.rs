//! Σ⁴-Engine 编译期常量（SPEC §3.1）。
//!
//! 定义引擎的固定内存预算与拓扑边界，在 ADR-002 的 4 个目标平台上一致。

/// 切面（facet）最大数：权力拓扑 / 动态模式 / 认知可达 / 级联响应等切面上限。
pub const MAX_SLICES: usize = 16;

/// 每切面单纯形端点（顶点）数上限（K_max）。
pub const K_MAX: usize = 8;

/// Entity Pool 预分配槽位上限（u16 可索引）。
pub const MAX_ENTITIES: usize = 65_536;

/// 每实体增量事件环形缓冲区容量（SPSC 模型，SPEC §3.2）。
pub const DELTA_RING_CAPACITY: usize = 1024;
