//! A2 索引/拓扑边界审计测试。
//!
//! 探测编译期硬约束的边界行为：
//! - MAX_SLICES=16
//! - K_MAX=8
//! - MAX_ENTITIES=65_536
//! - DELTA_RING_CAPACITY=1024

use sigma4_engine::entity::{DeltaEvent, Entity};
use sigma4_engine::matrix::CascadeMatrix;
use sigma4_engine::cascade::cascade;
use sigma4_engine::cascade::EntityStateView;
use sigma4_engine::constraint::{Constraint, ConstraintKind, pareto_project};

// ============================================================================
// Entity::new 边界测试
// ============================================================================

#[test]
#[should_panic(expected = "num_slices")]
fn entity_new_num_slices_exceeds_max_is_rejected() {
    // FIXED: num_slices=17 > MAX_SLICES=16 被拒绝（构造期 panic）
    Entity::new(0, 0, 17);
}

#[test]
fn entity_new_id_at_max_entities_is_accepted() {
    // id=65535 = MAX_ENTITIES-1，边界值
    let e = Entity::new(65_535, 0, 4);
    assert_eq!(e.id, 65_535);
}

#[test]
#[should_panic(expected = "MAX_ENTITIES")]
fn entity_new_id_exceeds_max_entities_is_rejected() {
    // FIXED: id=65536 >= MAX_ENTITIES 被拒绝（构造期 panic）
    Entity::new(65_536, 0, 4);
}

#[test]
#[should_panic(expected = "MAX_ENTITIES")]
fn entity_new_large_id_is_rejected() {
    // FIXED: id=100000 >> MAX_ENTITIES 被拒绝（构造期 panic）
    Entity::new(100_000, 0, 4);
}

// ============================================================================
// apply_delta_singlethreaded 边界测试
// ============================================================================

#[test]
fn apply_delta_endpoint_idx_exceeds_k_max_is_ignored() {
    // endpoint_idx=10 >= K_MAX=8，应该被拒绝但实际是被静默忽略
    let mut e = Entity::new(0, 0, 4);
    e.coordinates[0][0] = 0.5;
    e.slice_dims[0] = 2;

    let delta = DeltaEvent::new(0, 1, 10, 0.1);
    e.apply_delta_singlethreaded(delta);

    let snap = e.query_state(u64::MAX);
    // 坐标未变（因为 endpoint_idx>=K_MAX 被跳过）
    assert_eq!(snap.coords[0][0], 0.5);
}

#[test]
fn apply_delta_endpoint_idx_at_k_max_is_ignored() {
    // endpoint_idx=8 = K_MAX，刚好等于上限，应该被拒绝
    let mut e = Entity::new(0, 0, 4);
    e.coordinates[0][0] = 0.5;

    let delta = DeltaEvent::new(0, 1, 8, 0.1);
    e.apply_delta_singlethreaded(delta);

    let snap = e.query_state(u64::MAX);
    assert_eq!(snap.coords[0][0], 0.5);
}

#[test]
#[should_panic(expected = "delta ring overflow")]
fn apply_delta_ring_overflow_panics() {
    // 填满 1024 个槽位后再写一个，应panic
    let mut e = Entity::new(0, 0, 4);
    for i in 0..1024u64 {
        e.apply_delta_singlethreaded(DeltaEvent::new(i, 1, 0, 0.0));
    }
    // 第 1025 个写入应触发panic
    e.apply_delta_singlethreaded(DeltaEvent::new(1024, 1, 0, 0.0));
}

// ============================================================================
// query_state 时间戳边界测试
// ============================================================================

#[test]
fn query_state_timestamp_before_all_deltas() {
    // 查询时间早于所有delta
    let mut e = Entity::new(0, 0, 4);
    e.coordinates[0][0] = 0.5;

    e.apply_delta_singlethreaded(DeltaEvent::new(100, 1, 0, 0.3));
    e.apply_delta_singlethreaded(DeltaEvent::new(200, 1, 0, 0.2));

    let snap = e.query_state(50);
    assert_eq!(snap.coords[0][0], 0.5);
}

#[test]
fn query_state_timestamp_after_all_deltas() {
    // 查询时间晚于所有delta
    let mut e = Entity::new(0, 0, 4);
    e.coordinates[0][0] = 0.5;

    e.apply_delta_singlethreaded(DeltaEvent::new(100, 1, 0, 0.3));
    e.apply_delta_singlethreaded(DeltaEvent::new(200, 1, 0, 0.2));

    let snap = e.query_state(u64::MAX);
    // 应该回放所有delta并投影
    assert!((snap.coords[0][0] - 0.5).abs() > 1e-6);
}

#[test]
fn query_state_timestamp_at_exact_delta() {
    // 查询时间等于某个delta的时间戳
    let mut e = Entity::new(0, 0, 4);
    e.coordinates[0][0] = 0.5;

    e.apply_delta_singlethreaded(DeltaEvent::new(100, 1, 0, 0.3));
    e.apply_delta_singlethreaded(DeltaEvent::new(200, 1, 0, 0.2));

    let snap = e.query_state(100);
    // timestamp_us <= query_time_us，所以应回放第一个delta
    assert!((snap.coords[0][0] - 0.8).abs() < 1e-5);
}

#[test]
fn query_state_slice_mask_high_bit_set() {
    // slice_mask 第15位（0x8000）但 num_slices=4
    let mut e = Entity::new(0, 0, 4);
    e.coordinates[15][0] = 0.3;
    e.slice_dims[15] = 2;

    // 尝试修改未启用的切面
    e.apply_delta_singlethreaded(DeltaEvent::new(0, 0x8000, 0, 0.2));

    let snap = e.query_state(u64::MAX);
    // slice=15 < MAX_SLICES，所以会修改，但这是语义问题：切面15不在num_slices范围内
    assert_eq!(snap.coords[15][0], 0.5);
}

#[test]
fn query_state_slice_mask_multiple_bits_set() {
    // slice_mask 有多个位设置
    let mut e = Entity::new(0, 0, 4);
    e.coordinates[0][0] = 0.3;
    e.coordinates[0][1] = 0.7; // sum=1
    e.coordinates[1][0] = 0.2;
    e.coordinates[1][1] = 0.8; // sum=1
    e.slice_dims[0] = 2;
    e.slice_dims[1] = 2;

    // slice_mask=0b11（bit0和bit1都设置）
    e.apply_delta_singlethreaded(DeltaEvent::new(0, 0b11, 0, 0.1));

    let snap = e.query_state(u64::MAX);
    // trailing_zeros() 只返回最低位的索引，所以只修改切面0
    // 修改后 coords[0] = [0.4, 0.7]，投影后为 [0.35, 0.65]（theta=0.05）
    assert!((snap.coords[0][0] - 0.35).abs() < 0.01);
    // 切面1不变
    assert!((snap.coords[1][0] - 0.2).abs() < 0.01);
}

// ============================================================================
// CascadeMatrix::from_edges 边界测试
// ============================================================================

#[test]
#[should_panic(expected = "edge from=")]
fn matrix_from_edges_invalid_from_is_rejected() {
    // FIXED: from >= n 的边被拒绝（构造期 panic）
    let edges = vec![(5, 0, 0.5, 1)];
    CascadeMatrix::from_edges(3, &edges);
}

#[test]
#[should_panic(expected = "edge to=")]
fn matrix_from_edges_invalid_to_is_rejected() {
    // FIXED: to >= n 的边被拒绝（构造期 panic）
    let edges = vec![(0, 5, 0.5, 1)];
    CascadeMatrix::from_edges(3, &edges);
}

#[test]
fn matrix_from_edges_self_loop() {
    // 自环边 (0,0,w,lag)
    let edges = vec![(0, 0, 1.0, 0)];
    let m = CascadeMatrix::from_edges(2, &edges);
    assert_eq!(m.col_idx[0], 0);
    // 自环不会导致死循环（因为每跳更新整个向量）
}

#[test]
fn matrix_from_edges_duplicate_edges_weights_add() {
    // 重复边：权重应该是累加还是覆盖？
    let edges = vec![
        (0, 1, 0.3, 0),
        (0, 1, 0.2, 0),
    ];
    let m = CascadeMatrix::from_edges(2, &edges);
    // 两条边都保留，权重分别存储
    assert_eq!(m.values.len(), 2);
    assert!((m.values[0] - 0.3).abs() < 1e-6);
    assert!((m.values[1] - 0.2).abs() < 1e-6);
}

#[test]
fn matrix_from_edges_zero_matrix() {
    // n=0 空矩阵
    let m = CascadeMatrix::from_edges(0, &[]);
    assert_eq!(m.n, 0);
    assert_eq!(m.row_ptr.len(), 1); // [0]
}

// ============================================================================
// spmv_csr 边界测试
// ============================================================================

#[test]
#[should_panic]
fn spmv_csr_x_length_mismatch_panics() {
    // x 长度与 matrix.n 不匹配
    let edges = vec![(0, 1, 0.5, 0)];
    let m = CascadeMatrix::from_edges(3, &edges);
    let x = vec![1.0f32]; // 太短
    let mut y = vec![0.0f32; 3];
    sigma4_engine::matrix::spmv_csr(&x, &m, &mut y);
}

#[test]
fn spmv_csr_y_shorter_than_n_is_silent() {
    // BUG: y 长度 < matrix.n 时，zip silently truncates
    let edges = vec![(0, 1, 0.5, 0)];
    let m = CascadeMatrix::from_edges(3, &edges);
    let x = vec![1.0f32, 2.0, 3.0];
    let mut y = vec![0.0f32]; // 太短
    sigma4_engine::matrix::spmv_csr(&x, &m, &mut y);
    // 只写了 y[0]，y[1], y[2] 被忽略
    assert_eq!(y.len(), 1);
    assert!((y[0] - 1.0).abs() < 1e-6); // 0.5 * 2.0 = 1.0
}

#[test]
#[should_panic]
fn spmv_csr_invalid_col_idx_panics() {
    // Defense-in-depth：手动构造非法矩阵（from_edges 已在构造期拦截，此测试验证 spmv 内部越界保护）
    // col_idx 包含 5，但 x 长度为 3 → spmv 访问 x[5] 时应 panic
    let m = CascadeMatrix {
        n: 3,
        row_ptr: vec![0, 1, 1, 1],
        col_idx: vec![5], // 非法列索引
        values: vec![0.5],
        time_lag_us: vec![0],
    };
    let x = vec![1.0f32, 2.0, 3.0];
    let mut y = vec![0.0f32; 3];
    sigma4_engine::matrix::spmv_csr(&x, &m, &mut y);
}

#[test]
fn spmv_csr_empty_matrix() {
    // n=0 空矩阵的spmv
    let m = CascadeMatrix::from_edges(0, &[]);
    let x: Vec<f32> = vec![];
    let mut y: Vec<f32> = vec![];
    sigma4_engine::matrix::spmv_csr(&x, &m, &mut y);
    assert_eq!(y.len(), 0);
}

// ============================================================================
// cascade 边界测试
// ============================================================================

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "cascade: initial.len()")]
fn cascade_initial_shorter_than_n_is_rejected_debug() {
    // FIXED: initial 长度 < matrix.n 时，debug 下 panic 暴露调用方 bug（release 模式仍容错填零）
    let edges = vec![(0, 1, 0.5, 0)];
    let m = CascadeMatrix::from_edges(3, &edges);
    let initial = vec![1.0f32]; // 太短，只有1个元素
    let states = vec![EntityStateView {
        coordinates: 0.0,
        brittle_threshold: 0.0,
        decay_coefficient: 1.0,
        time_lag_us: 0,
    }; 3];

    cascade(&initial, &m, &states, 1, 0.1);
}

#[test]
fn cascade_initial_valid_entity_id_out_of_range() {
    // BUG: initial 可以包含超出 matrix.n 的 entity id
    // 在 resize 之后这些值会被截断
    let edges = vec![(0, 1, 0.5, 0)];
    let m = CascadeMatrix::from_edges(2, &edges);
    let initial = vec![1.0f32, 2.0, 3.0, 4.0]; // 长度4 > n=2
    let states = vec![EntityStateView {
        coordinates: 0.0,
        brittle_threshold: 0.0,
        decay_coefficient: 1.0,
        time_lag_us: 0,
    }; 2];

    let res = cascade(&initial, &m, &states, 1, 0.1);
    // initial[2], initial[3] 被丢弃
    assert_eq!(res.len(), 2);
}

#[test]
fn cascade_empty_matrix() {
    // n=0 空矩阵的cascade
    let m = CascadeMatrix::from_edges(0, &[]);
    let initial: Vec<f32> = vec![];
    let states: Vec<EntityStateView> = vec![];

    let res = cascade(&initial, &m, &states, 1, 0.1);
    assert_eq!(res.len(), 0);
}

#[test]
fn cascade_entity_states_shorter_than_signal() {
    // entity_states 比 signal 短
    let edges = vec![(0, 1, 0.5, 0)];
    let m = CascadeMatrix::from_edges(3, &edges);
    let initial = vec![1.0f32, 0.0, 0.0];
    let states = vec![EntityStateView {
        coordinates: 0.0,
        brittle_threshold: 0.0,
        decay_coefficient: 1.0,
        time_lag_us: 0,
    }; 1]; // 只有1个state

    let res = cascade(&initial, &m, &states, 1, 0.1);
    // i >= entity_states.len() 时 propagated[i]=0
    assert!(!res.iter().any(|r| r.entity_id > 0));
}

// ============================================================================
// Constraint 边界测试
// ============================================================================

#[test]
fn constraint_slice_exceeds_max_is_ignored() {
    // slice=17 >= MAX_SLICES=16
    let mut coords = [[0.0f32; 8]; 16];
    coords[0][0] = 0.5;
    let slice_dims = [0u8; 16];

    let c = Constraint {
        slice: 17,
        endpoint: 0,
        value: 0.8,
        kind: ConstraintKind::UpperBound,
    };

    pareto_project(&mut coords, &[c], &slice_dims);
    // 被忽略，coords不变（投影后可能略有变化）
    assert!((coords[0][0] - 0.5).abs() < 0.01);
}

#[test]
fn constraint_endpoint_exceeds_k_max_is_ignored() {
    // endpoint=10 >= K_MAX=8
    let mut coords = [[0.0f32; 8]; 16];
    coords[0][0] = 0.5;
    let slice_dims = [0u8; 16];

    let c = Constraint {
        slice: 0,
        endpoint: 10,
        value: 0.8,
        kind: ConstraintKind::UpperBound,
    };

    pareto_project(&mut coords, &[c], &slice_dims);
    // 被忽略，coords不变
    assert!((coords[0][0] - 0.5).abs() < 0.01);
}

#[test]
fn constraint_slice_at_max_is_accepted() {
    // slice=15 = MAX_SLICES-1，边界值
    let mut coords = [[0.0f32; 8]; 16];
    coords[15][0] = 0.5;
    coords[15][1] = 0.5;
    let slice_dims = [0u8; 16];

    let c = Constraint {
        slice: 15,
        endpoint: 0,
        value: 0.3,
        kind: ConstraintKind::UpperBound,
    };

    pareto_project(&mut coords, &[c], &slice_dims);
    // 0.5 被 clamp 到 0.3，然后投影重分配
    assert!(coords[15][0] <= 0.3 + 0.01);
}

#[test]
fn constraint_endpoint_at_k_max_is_ignored() {
    // endpoint=8 = K_MAX，刚好等于上限
    let mut coords = [[0.0f32; 8]; 16];
    coords[0][7] = 0.9; // 最后一个有效索引
    let slice_dims = [0u8; 16];

    let c = Constraint {
        slice: 0,
        endpoint: 8,
        value: 0.5,
        kind: ConstraintKind::UpperBound,
    };

    pareto_project(&mut coords, &[c], &slice_dims);
    // endpoint >= K_MAX 被忽略，投影后保持原值
    assert!((coords[0][7] - 0.9).abs() < 0.01);
}
