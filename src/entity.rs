//! 数据实体内存布局（SPEC §3.1 / §3.4）与状态查询（SPEC §3.2 / §6.1）。
//!
//! 所有热路径结构为 `repr(C)`，保证跨平台（ADR-002 矩阵）确定性布局与字段偏移。
//! `Entity` 整体 64 字节对齐（缓存行友好）。
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
    /// 增量事件环形缓冲区（SPSC；溢出由 `ring_head - ring_tail >= DELTA_RING_CAPACITY` 检测）。
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

// SAFETY: `name_ptr` 指向外部（非拥有）字节串，跨线程移动 Entity 不产生数据竞争——
// 指针仅作数据值，所指数据的生命周期与线程可见性由外部（持有名字的所有者）保证。
// 故 Entity 可跨线程传递（服务端 Arc<Mutex<Engine>> 共享所需）。
unsafe impl Send for Entity {}

/// 增量事件：单点坐标更新（SPEC §3.4）。`repr(packed)` 紧凑打包到 16 字节。
///
/// 注意：packed 结构的字段访问可能非对齐；读取应按值拷贝（query_state 已如此）。
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct DeltaEvent {
    /// 事件时间戳（微秒，单调）。
    pub timestamp_us: u64,
    /// 受影响切面位掩码（bit i → slice i；定位用 `trailing_zeros()`）。
    pub slice_mask: u16,
    /// 切面内端点索引（< K_MAX）。
    pub endpoint_idx: u8,
    /// 该端点坐标的增量值。
    pub delta_value: f32,
    _pad: u8,
}

impl DeltaEvent {
    /// 构造增量事件（`_pad` 置 0）。
    pub fn new(timestamp_us: u64, slice_mask: u16, endpoint_idx: u8, delta_value: f32) -> Self {
        Self {
            timestamp_us,
            slice_mask,
            endpoint_idx,
            delta_value,
            _pad: 0,
        }
    }
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

impl SteadyState {
    /// 构造稳态属性（`_pad` 置 0）。
    pub fn new(geography: u16, industry: u16, ownership_type: u8) -> Self {
        Self {
            geography,
            industry,
            ownership_type,
            _pad: [0; 5],
        }
    }
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

/// 状态查询快照：**全栈分配（无堆）**，深拷贝坐标。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EntitySnapshot {
    /// 查询时刻回放 + Duchi 投影后的坐标。
    pub coords: [[f32; K_MAX]; MAX_SLICES],
    /// 每切面有效维度数。
    pub slice_dims: [u8; MAX_SLICES],
    /// 有效切面数。
    pub num_slices: u8,
}

impl Entity {
    /// 构造零初始化实体（坐标与增量环清零；仅设 id / entity_type / num_slices；valid_until=MAX）。
    pub fn new(id: u32, entity_type: u8, num_slices: u8) -> Self {
        Self {
            id,
            entity_type,
            flags: 0,
            num_slices,
            _pad0: 0,
            valid_from: 0,
            valid_until: i64::MAX,
            coordinates: [[0.0; K_MAX]; MAX_SLICES],
            slice_dims: [0; MAX_SLICES],
            delta_ring: [DeltaEvent::new(0, 0, 0, 0.0); DELTA_RING_CAPACITY],
            ring_head: 0,
            ring_tail: 0,
            steady_state: SteadyState {
                geography: 0,
                industry: 0,
                ownership_type: 0,
                _pad: [0; 5],
            },
            name_ptr: core::ptr::null(),
            name_len: 0,
            _pad1: [0; 6],
        }
    }

    /// 单线程（SPSC）写入一个增量事件到环尾。
    ///
    /// 写 `delta_ring[ring_head % CAP]`，`ring_head += 1`；若
    /// `ring_head - ring_tail >= CAP` 则 panic（需要快照刷盘，SPEC §3.2）。
    pub fn apply_delta_singlethreaded(&mut self, delta: DeltaEvent) {
        let cap = DELTA_RING_CAPACITY as u32;
        self.delta_ring[(self.ring_head % cap) as usize] = delta;
        self.ring_head = self.ring_head.wrapping_add(1);
        if self.ring_head.wrapping_sub(self.ring_tail) >= cap {
            panic!("delta ring overflow: head - tail >= {cap} (需要快照刷盘)");
        }
    }

    /// 查询 `query_time_us` 时刻的实体状态（深拷贝基态 + 回放 + 投影）。
    ///
    /// - 深拷贝基态坐标（**不返回引用**）。
    /// - 回放 `ring[tail..head]` 中 `timestamp_us <= query_time_us` 的 delta，
    ///   切面由 `slice_mask.trailing_zeros()` 定位。
    /// - 回放后对每切面做 Duchi 投影；无 delta 回放时返回基态（跳过投影）。
    /// - **查询函数内无堆分配**（`EntitySnapshot` 栈分配，投影用栈 scratch）。
    pub fn query_state(&self, query_time_us: u64) -> EntitySnapshot {
        let mut snap = EntitySnapshot {
            coords: self.coordinates,
            slice_dims: self.slice_dims,
            num_slices: self.num_slices,
        };

        let head = self.ring_head;
        let tail = self.ring_tail;
        let cap = DELTA_RING_CAPACITY as u32;
        let mut applied = false;

        let mut i = tail;
        while i != head {
            let ev = &self.delta_ring[(i % cap) as usize];
            // packed 字段按值拷贝读取（安全，非借用）
            if ev.timestamp_us <= query_time_us {
                let slice = ev.slice_mask.trailing_zeros() as usize;
                let ep = ev.endpoint_idx as usize;
                if slice < MAX_SLICES && ep < K_MAX {
                    snap.coords[slice][ep] += ev.delta_value;
                    applied = true;
                }
            }
            i = i.wrapping_add(1);
        }

        if applied {
            let n = (self.num_slices as usize).min(MAX_SLICES);
            for s in 0..n {
                let k = (self.slice_dims[s] as usize).min(K_MAX);
                crate::simplex::project_onto_simplex_inplace(&mut snap.coords[s], k);
            }
        }

        snap
    }
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
        assert_eq!(sz, 17_024, "Entity exact size drifted: {sz}");
    }

    #[test]
    fn entity_header_offsets_are_repr_c() {
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
        assert_eq!(align_of::<DeltaEvent>(), 1);
    }

    #[test]
    fn steady_state_layout() {
        assert_eq!(size_of::<SteadyState>(), 2 + 2 + 1 + 5);
        assert_eq!(align_of::<SteadyState>(), 2);
    }

    #[test]
    fn relation_compiles_and_sized() {
        let r = Relation {
            from_id: 1,
            to_id: 2,
            relation_type: 0,
            weight: 0.5,
            time_lag_us: 100,
            valid_from: 0,
            valid_until: i64::MAX,
        };
        assert_eq!(align_of::<Relation>(), 8);
        assert_eq!(r.from_id, 1);
    }
}

#[cfg(test)]
mod state_query {
    use super::*;
    use std::hint::black_box;
    use std::panic::AssertUnwindSafe;
    use std::time::Instant;

    fn make_entity(
        num_slices: u8,
        dims: [u8; MAX_SLICES],
        coords: [[f32; K_MAX]; MAX_SLICES],
    ) -> Entity {
        Entity {
            id: 0,
            entity_type: 0,
            flags: 0,
            num_slices,
            _pad0: 0,
            valid_from: 0,
            valid_until: i64::MAX,
            coordinates: coords,
            slice_dims: dims,
            delta_ring: [DeltaEvent {
                timestamp_us: 0,
                slice_mask: 0,
                endpoint_idx: 0,
                delta_value: 0.0,
                _pad: 0,
            }; DELTA_RING_CAPACITY],
            ring_head: 0,
            ring_tail: 0,
            steady_state: SteadyState {
                geography: 0,
                industry: 0,
                ownership_type: 0,
                _pad: [0; 5],
            },
            name_ptr: core::ptr::null(),
            name_len: 0,
            _pad1: [0; 6],
        }
    }

    fn de(ts: u64, mask: u16, ep: u8, dv: f32) -> DeltaEvent {
        DeltaEvent {
            timestamp_us: ts,
            slice_mask: mask,
            endpoint_idx: ep,
            delta_value: dv,
            _pad: 0,
        }
    }

    #[test]
    fn no_delta_returns_base_state() {
        let mut coords = [[0.0f32; K_MAX]; MAX_SLICES];
        coords[0] = [0.2, 0.3, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut dims = [0u8; MAX_SLICES];
        dims[0] = 3;
        let e = make_entity(1, dims, coords);
        let snap = e.query_state(u64::MAX);
        assert!((snap.coords[0][0] - 0.2).abs() < 1e-6);
        assert!((snap.coords[0][1] - 0.3).abs() < 1e-6);
        assert!((snap.coords[0][2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn no_delta_query_under_500ns() {
        let mut coords = [[0.0f32; K_MAX]; MAX_SLICES];
        coords[0] = [0.3, 0.3, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut dims = [0u8; MAX_SLICES];
        dims[0] = 3;
        let e = make_entity(1, dims, coords);
        for _ in 0..1000 {
            let _ = black_box(e.query_state(0));
        }
        let iters = 50_000u64;
        let start = Instant::now();
        for _ in 0..iters {
            let s = black_box(e.query_state(black_box(0)));
            black_box(s);
        }
        let ns = start.elapsed().as_nanos() as f64 / iters as f64;
        assert!(ns < 500.0, "no-delta query {ns:.1}ns >= 500ns");
    }

    #[test]
    fn early_query_returns_base() {
        let mut coords = [[0.0f32; K_MAX]; MAX_SLICES];
        coords[0] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut dims = [0u8; MAX_SLICES];
        dims[0] = 3;
        let mut e = make_entity(1, dims, coords);
        e.apply_delta_singlethreaded(de(100, 1, 0, 5.0));
        let snap = e.query_state(50); // 50 < 100 → 不回放
        assert!((snap.coords[0][0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn late_query_replays_all_and_projects() {
        let coords = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut dims = [0u8; MAX_SLICES];
        dims[0] = 2;
        let mut e = make_entity(1, dims, coords);
        e.apply_delta_singlethreaded(de(10, 1, 0, 0.3));
        e.apply_delta_singlethreaded(de(20, 1, 1, 0.3));
        let snap = e.query_state(u64::MAX);
        // 回放后 [0.3,0.3] → 投影 → [0.5,0.5]
        let sum: f32 = snap.coords[0][..2].iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "sum={sum}");
        assert!((snap.coords[0][0] - 0.5).abs() < 1e-5);
        assert!((snap.coords[0][1] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn slice_mask_trailing_zeros_locates_slice() {
        // slice_mask = 1<<2 → 切面 2
        let mut coords = [[0.0f32; K_MAX]; MAX_SLICES];
        coords[2] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut dims = [0u8; MAX_SLICES];
        dims[2] = 2;
        let mut e = make_entity(3, dims, coords);
        e.apply_delta_singlethreaded(de(5, 1 << 2, 0, 0.5));
        let snap = e.query_state(u64::MAX);
        // 切面 2: [1.0,0.5] → 投影 → [0.75,0.25]
        assert!(
            (snap.coords[2][0] - 0.75).abs() < 1e-5,
            "got {}",
            snap.coords[2][0]
        );
        assert!(
            (snap.coords[2][1] - 0.25).abs() < 1e-5,
            "got {}",
            snap.coords[2][1]
        );
    }

    #[test]
    fn replay_100_deltas_correct_and_under_2us() {
        let mut coords = [[0.0f32; K_MAX]; MAX_SLICES];
        coords[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut dims = [0u8; MAX_SLICES];
        dims[0] = 2;
        let mut e = make_entity(1, dims, coords);
        // 100 delta：50 个 +0.01、50 个 -0.01 → 净 0
        for i in 0..100u64 {
            let dv = if i < 50 { 0.01 } else { -0.01 };
            e.apply_delta_singlethreaded(de(i, 1, 0, dv));
        }
        let snap = e.query_state(u64::MAX);
        assert!(
            (snap.coords[0][0] - 0.5).abs() < 1e-5,
            "got {}",
            snap.coords[0][0]
        );
        assert!((snap.coords[0][1] - 0.5).abs() < 1e-5);

        for _ in 0..1000 {
            let _ = black_box(e.query_state(u64::MAX));
        }
        let iters = 20_000u64;
        let start = Instant::now();
        for _ in 0..iters {
            let s = black_box(e.query_state(black_box(u64::MAX)));
            black_box(s);
        }
        let ns = start.elapsed().as_nanos() as f64 / iters as f64;
        assert!(ns < 2000.0, "100-delta query {ns:.1}ns >= 2000ns");
    }

    #[test]
    fn ring_overflow_panics() {
        let coords = [[0.0f32; K_MAX]; MAX_SLICES];
        let dims = [0u8; MAX_SLICES];
        let mut e = make_entity(0, dims, coords);
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            for i in 0..DELTA_RING_CAPACITY {
                e.apply_delta_singlethreaded(de(i as u64, 1, 0, 0.0));
            }
        }));
        assert!(result.is_err(), "expected panic on ring overflow");
    }
}
