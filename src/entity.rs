//! 数据实体内存布局（SPEC §3.1 / §3.4）。
//!
//! 所有热路径结构为 `repr(C)`，保证跨平台（ADR-002 矩阵）确定性布局与字段偏移。
//! `Entity` 整体 64 字节对齐（缓存行友好）。本文件**仅布局，无方法**（task_002）。
//!
//! 坐标二维数组语义：`coordinates[切面行][端点列]`。

use crate::constants::{DELTA_RING_CAPACITY, K_MAX, MAX_SLICES};

/// 实体：一个资本主体（宏观基金 / 主权基金 / 家族网络 / 央企集团等）。
///
/// 字段顺序即内存顺序（`repr(C)`），**不得调整**。
#[repr(C, align(64))]
pub struct Entity {
    /// 全局唯一实体 id（Entity Pool 内索引）。
    pub id: u32,
    /// 实体类型（A/B/C/D/E 五类，编码为枚举值）。
    pub entity_type: u8,
    /// 标志位（中国主体标记、活跃、冻结等位域）。
    pub flags: u8,
    /// 当前有效切面数（≤ MAX_SLICES）。
    pub num_slices: u8,
    _pad0: u8,
    /// 有效起始时间（微秒；i64，历史时间可为负，SPEC §3.3 双向语义）。
    pub valid_from: i64,
    /// 有效终止时间（微秒；i64::MAX 表示永久有效）。
    pub valid_until: i64,
    /// 多切面单纯形坐标：`[切面][端点] = f32` 权重。
    pub coordinates: [[f32; K_MAX]; MAX_SLICES],
    /// 每切面有效维度数（≤ K_MAX；padding 列必须显式置 0）。
    pub slice_dims: [u8; MAX_SLICES],
    /// 增量事件环形缓冲区（SPSC；溢出由 `ring_head - ring_tail >= DELTA_RING_CAPACITY` 检测，task_003）。
    pub delta_ring: [DeltaEvent; DELTA_RING_CAPACITY],
    /// 环头（下一个写入位）。
    pub ring_head: u32,
    /// 环尾（下一个读取位）。
    pub ring_tail: u32,
    /// 稳态属性（地理 / 行业 / 所有权，低频变更）。
    pub steady_state: SteadyState,
    /// 实体显示名指针（非热路径；指向外部字节，**非 String**）。
    pub name_ptr: *const u8,
    /// 显示名字节长度。
    pub name_len: u16,
    _pad1: [u8; 6],
}

/// 增量事件：单点坐标更新（SPEC §3.4）。`repr(packed)` 紧凑打包到 16 字节。
///
/// 注意：packed 结构的字段访问可能非对齐；读取应先拷贝到局部再解引用（task_003 起的查询路径）。
#[repr(C, packed)]
pub struct DeltaEvent {
    /// 事件时间戳（微秒，单调）。
    pub timestamp_us: u64,
    /// 受影响切面位掩码（bit i → slice i）。
    pub slice_mask: u16,
    /// 切面内端点索引（< K_MAX）。
    pub endpoint_idx: u8,
    /// 该端点坐标的增量值。
    pub delta_value: f32,
    _pad: u8,
}

/// 稳态属性（地理 / 行业 / 所有权类型）。
#[repr(C)]
pub struct SteadyState {
    /// 地理区域编码。
    pub geography: u16,
    /// 行业编码。
    pub industry: u16,
    /// 所有权类型（主权 / 家族 / 央企 / 对冲基金 等）。
    pub ownership_type: u8,
    _pad: [u8; 5],
}

/// 实体间关系边（CSR 级联矩阵的语义来源，SPEC §3.4）。
#[repr(C)]
pub struct Relation {
    /// 源实体 id。
    pub from_id: u32,
    /// 目标实体 id。
    pub to_id: u32,
    /// 关系类型（持股 / 影响力 / 情报共享 等）。
    pub relation_type: u8,
    /// 关系权重 ∈ [0,1]（级联传播系数）。
    pub weight: f32,
    /// 传播时滞（微秒）。
    pub time_lag_us: u32,
    /// 关系有效起始（微秒，i64）。
    pub valid_from: i64,
    /// 关系有效终止（微秒，i64）。
    pub valid_until: i64,
}

#[cfg(test)]
mod entity_layout {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn entity_align_is_64() {
        assert_eq!(align_of::<Entity>(), 64);
    }

    #[test]
    fn entity_size_within_budget() {
        let sz = size_of::<Entity>();
        assert!(sz <= 65_536, "Entity size {sz} exceeds 64KB budget");
        // 预期 ~17KB（header 24B + coords 512B + dims 16B + ring 16KB + 尾部）
        assert_eq!(sz, 17_024, "Entity exact size drifted: {sz}");
    }

    #[test]
    fn entity_header_offsets_are_repr_c() {
        // repr(C) 字段顺序断言（跨平台确定性）
        assert_eq!(offset_of!(Entity, id), 0);
        assert_eq!(offset_of!(Entity, entity_type), 4);
        assert_eq!(offset_of!(Entity, flags), 5);
        assert_eq!(offset_of!(Entity, num_slices), 6);
        assert_eq!(offset_of!(Entity, valid_from), 8);
        assert_eq!(offset_of!(Entity, valid_until), 16);
        assert_eq!(offset_of!(Entity, coordinates), 24);
    }

    #[test]
    fn delta_event_is_16_bytes_packed() {
        assert_eq!(size_of::<DeltaEvent>(), 16);
        assert_eq!(align_of::<DeltaEvent>(), 1); // repr(packed) → align 1
    }

    #[test]
    fn steady_state_layout() {
        assert_eq!(size_of::<SteadyState>(), 2 + 2 + 1 + 5);
        assert_eq!(align_of::<SteadyState>(), 2);
    }

    #[test]
    fn relation_compiles_and_sized() {
        // repr(C) 可编译 + 字段可构造 + 合理对齐
        let r = Relation {
            from_id: 1,
            to_id: 2,
            relation_type: 0,
            weight: 0.5,
            time_lag_us: 100,
            valid_from: 0,
            valid_until: i64::MAX,
        };
        assert_eq!(align_of::<Relation>(), 8); // 含 i64 → align 8
        assert_eq!(r.from_id, 1);
    }
}
