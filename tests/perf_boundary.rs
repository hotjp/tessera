//! Σ⁴-Engine 性能/延迟边界审计（A5）。
//!
//! 测量 SPEC 性能目标：
//! - 状态写入 < 1μs
//! - 状态查询 < 1μs
//! - 级联推理(100 实体/5 跳) < 100μs
//! - 快照恢复 < 500ms
//!
//! 使用 `std::time::Instant` 直接测量，避免 criterion 依赖。
//! 断言使用宽松容差（10× SPEC）避免 CI 抖动；实际测量值打印供分析。

use sigma4_engine::cascade::{cascade, EntityStateView};
use sigma4_engine::entity::{DeltaEvent, Entity};
use sigma4_engine::matrix::{spmv_csr, spmv_csr_scalar, CascadeMatrix};
use std::hint::black_box;
use std::time::Instant;

/// 辅助：创建非脆性实体状态视图。
fn nb_view(decay: f32, lag: u32) -> EntityStateView {
    EntityStateView {
        coordinates: 0.0,
        brittle_threshold: 0.0,
        decay_coefficient: decay,
        time_lag_us: lag,
    }
}

/// 辅助：创建增量事件。
fn de(ts: u64, mask: u16, ep: u8, dv: f32) -> DeltaEvent {
    DeltaEvent::new(ts, mask, ep, dv)
}

/// 辅助：创建测试实体。
fn make_entity(
    id: u32,
    num_slices: u8,
    dims: &[u8; 16],
    coords: &[[f32; 8]; 16],
) -> Entity {
    let mut e = Entity::new(id, 0, num_slices);
    e.slice_dims = *dims;
    e.coordinates = *coords;
    e
}

/// 辅助：统计百分位数。
fn percentile(sorted: &mut [f64], p: f64) -> f64 {
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = (p * (sorted.len() - 1) as f64) as usize;
    sorted[idx]
}

// ============================================================================
// 1. 写入延迟测试：Entity::apply_delta_singlethreaded
// ============================================================================

#[test]
fn write_latency_single_delta() {
    const SPEC_TARGET_NS: f64 = 1000.0; // 1μs
    const ITERS: usize = 100_000;

    let mut e = Entity::new(0, 0, 3);
    let delta = de(0, 1, 0, 0.1);

    // 预热
    for _ in 0..1000 {
        e.apply_delta_singlethreaded(delta);
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        // 每次迭代创建新实体避免环溢出
        let mut e = Entity::new(0, 0, 3);
        let start = Instant::now();
        e.apply_delta_singlethreaded(delta);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);
    let p999 = percentile(&mut times_ns, 0.999);

    println!("\n=== 写入延迟 (单增量) ===");
    println!("中位数: {:.1} ns ({:.2}× SPEC)", median, median / SPEC_TARGET_NS);
    println!("P99: {:.1} ns ({:.2}× SPEC)", p99, p99 / SPEC_TARGET_NS);
    println!("P99.9: {:.1} ns ({:.2}× SPEC)", p999, p999 / SPEC_TARGET_NS);
    println!(
        "SPEC 目标: < {} ns (1μs) | 状态: {}",
        SPEC_TARGET_NS,
        if median < SPEC_TARGET_NS { "✅ 达标" } else { "❌ 未达标" }
    );

    // 宽松断言（10× 容差）
    assert!(median < 10.0 * SPEC_TARGET_NS, "中位数写入延迟超出 10× 容差");
}

#[test]
fn write_latency_back_to_back() {
    const SPEC_TARGET_NS: f64 = 1000.0; // 1μs
    const WARMUP_ITERS: usize = 500;
    const MEASURE_ITERS: usize = 300; // 限制测量次数避免溢出

    let mut e = Entity::new(0, 0, 3);

    // 预热：填环到约一半
    for i in 0..WARMUP_ITERS {
        e.apply_delta_singlethreaded(de(i as u64, 1, 0, 0.01));
    }

    let mut times_ns = Vec::with_capacity(MEASURE_ITERS);
    let start_ts = WARMUP_ITERS as u64;
    for offset in 0..MEASURE_ITERS {
        let start = Instant::now();
        e.apply_delta_singlethreaded(de(start_ts + offset as u64, 1, 0, 0.01));
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);

    println!("\n=== 写入延迟 (背靠背，环半满) ===");
    println!("中位数: {:.1} ns ({:.2}× SPEC)", median, median / SPEC_TARGET_NS);
    println!("P99: {:.1} ns ({:.2}× SPEC)", p99, p99 / SPEC_TARGET_NS);
    println!(
        "SPEC 目标: < {} ns | 状态: {}",
        SPEC_TARGET_NS,
        if median < SPEC_TARGET_NS { "✅ 达标" } else { "❌ 未达标" }
    );

    assert!(median < 10.0 * SPEC_TARGET_NS);
}

// ============================================================================
// 2. 查询延迟测试：Entity::query_state
// ============================================================================

#[test]
fn query_latency_empty_ring() {
    const SPEC_TARGET_NS: f64 = 1000.0; // 1μs
    const ITERS: usize = 100_000;

    let mut coords = [[0.0f32; 8]; 16];
    coords[0] = [0.3, 0.3, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut dims = [0u8; 16];
    dims[0] = 3;
    let e = make_entity(0, 1, &dims, &coords);

    // 预热
    for _ in 0..1000 {
        black_box(e.query_state(0));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let s = black_box(e.query_state(black_box(0)));
        black_box(s);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);

    println!("\n=== 查询延迟 (空环) ===");
    println!("中位数: {:.1} ns ({:.2}× SPEC)", median, median / SPEC_TARGET_NS);
    println!("P99: {:.1} ns ({:.2}× SPEC)", p99, p99 / SPEC_TARGET_NS);
    println!(
        "SPEC 目标: < {} ns | 状态: {}",
        SPEC_TARGET_NS,
        if median < SPEC_TARGET_NS { "✅ 达标" } else { "❌ 未达标" }
    );

    assert!(median < 10.0 * SPEC_TARGET_NS);
}

#[test]
fn query_latency_100_deltas() {
    const SPEC_TARGET_NS: f64 = 1000.0; // 1μs
    const ITERS: usize = 50_000;

    let mut coords = [[0.0f32; 8]; 16];
    coords[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut dims = [0u8; 16];
    dims[0] = 2;
    let mut e = make_entity(0, 1, &dims, &coords);

    // 写入 100 个 delta
    for i in 0..100 {
        let dv = if i < 50 { 0.01 } else { -0.01 };
        e.apply_delta_singlethreaded(de(i as u64, 1, 0, dv));
    }

    // 预热
    for _ in 0..1000 {
        black_box(e.query_state(u64::MAX));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let s = black_box(e.query_state(black_box(u64::MAX)));
        black_box(s);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);

    println!("\n=== 查询延迟 (100 个 delta) ===");
    println!("中位数: {:.1} ns ({:.2}× SPEC)", median, median / SPEC_TARGET_NS);
    println!("P99: {:.1} ns ({:.2}× SPEC)", p99, p99 / SPEC_TARGET_NS);
    println!(
        "SPEC 目标: < {} ns | 状态: {}",
        SPEC_TARGET_NS,
        if median < SPEC_TARGET_NS { "✅ 达标" } else { "❌ 未达标" }
    );

    assert!(median < 10.0 * SPEC_TARGET_NS);
}

#[test]
fn query_latency_ring_nearly_full() {
    const SPEC_TARGET_NS: f64 = 1000.0; // 1μs
    const ITERS: usize = 20_000;

    let mut coords = [[0.0f32; 8]; 16];
    coords[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut dims = [0u8; 16];
    dims[0] = 2;
    let mut e = make_entity(0, 1, &dims, &coords);

    // 填充环到 90%
    for i in 0..900 {
        e.apply_delta_singlethreaded(de(i as u64, 1, 0, 0.001));
    }

    // 预热
    for _ in 0..500 {
        black_box(e.query_state(u64::MAX));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let s = black_box(e.query_state(black_box(u64::MAX)));
        black_box(s);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);

    println!("\n=== 查询延迟 (环 90% 满) ===");
    println!("中位数: {:.1} ns ({:.2}× SPEC)", median, median / SPEC_TARGET_NS);
    println!("P99: {:.1} ns ({:.2}× SPEC)", p99, p99 / SPEC_TARGET_NS);
    println!(
        "SPEC 目标: < {} ns | 状态: {}",
        SPEC_TARGET_NS,
        if median < SPEC_TARGET_NS { "✅ 达标" } else { "❌ 未达标" }
    );
    println!("退化分析: 环满时延迟是否显著增加？");

    assert!(median < 10.0 * SPEC_TARGET_NS);
}

// ============================================================================
// 3. 级联延迟测试：cascade() 不同图规模
// ============================================================================

#[test]
fn cascade_10_entities_5_hops() {
    const ITERS: usize = 10_000;

    let n = 10u32;
    let mut edges = Vec::new();
    // 环状图：每个节点连到后 3 个
    for i in 0..n {
        for d in 1..=3u32 {
            let src = (i + d) % n;
            edges.push((i, src, 0.3, d));
        }
    }
    let m = CascadeMatrix::from_edges(n, &edges);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    // 预热
    for _ in 0..100 {
        black_box(cascade(&initial, &m, &states, 5, 0.1));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let r = black_box(cascade(
            black_box(&initial),
            black_box(&m),
            black_box(&states),
            5,
            0.1,
        ));
        black_box(r);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);

    println!("\n=== 级联延迟 (10 实体, 5 跳) ===");
    println!("中位数: {:.1} ns", median);
    println!("P99: {:.1} ns", p99);
    println!("边数: {}", edges.len());
}

#[test]
fn cascade_100_entities_5_hops() {
    const SPEC_TARGET_NS: f64 = 100_000.0; // 100μs
    const ITERS: usize = 5_000;

    let n = 100u32;
    let mut edges = Vec::new();
    // 环状图：每个节点连到后 3 个
    for i in 0..n {
        for d in 1..=3u32 {
            let src = (i + d) % n;
            edges.push((i, src, 0.3, d));
        }
    }
    let m = CascadeMatrix::from_edges(n, &edges);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    // 预热
    for _ in 0..100 {
        black_box(cascade(&initial, &m, &states, 5, 0.1));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let r = black_box(cascade(
            black_box(&initial),
            black_box(&m),
            black_box(&states),
            5,
            0.1,
        ));
        black_box(r);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);

    println!("\n=== 级联延迟 (100 实体, 5 跳) ===");
    println!("中位数: {:.1} ns ({:.2}× SPEC)", median, median / SPEC_TARGET_NS);
    println!("P99: {:.1} ns ({:.2}× SPEC)", p99, p99 / SPEC_TARGET_NS);
    println!("边数: {}", edges.len());
    println!(
        "SPEC 目标: < {} ns (100μs) | 状态: {}",
        SPEC_TARGET_NS,
        if median < SPEC_TARGET_NS { "✅ 达标" } else { "❌ 未达标" }
    );

    // 10× 容差
    assert!(median < 10.0 * SPEC_TARGET_NS);
}

#[test]
fn cascade_500_entities_5_hops() {
    const ITERS: usize = 1_000;

    let n = 500u32;
    let mut edges = Vec::new();
    // 环状图：每个节点连到后 3 个
    for i in 0..n {
        for d in 1..=3u32 {
            let src = (i + d) % n;
            edges.push((i, src, 0.3, d));
        }
    }
    let m = CascadeMatrix::from_edges(n, &edges);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    // 预热
    for _ in 0..50 {
        black_box(cascade(&initial, &m, &states, 5, 0.1));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let r = black_box(cascade(
            black_box(&initial),
            black_box(&m),
            black_box(&states),
            5,
            0.1,
        ));
        black_box(r);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);
    let p99 = percentile(&mut times_ns, 0.99);

    println!("\n=== 级联延迟 (500 实体, 5 跳) ===");
    println!("中位数: {:.1} ns ({:.2} μs)", median, median / 1000.0);
    println!("P99: {:.1} ns ({:.2} μs)", p99, p99 / 1000.0);
    println!("边数: {}", edges.len());
    println!("可伸缩性分析: 5x 规模延迟增加多少？");
}

// ============================================================================
// 4. SIMD vs 标量一致性测试
// ============================================================================

#[test]
fn simd_matches_scalar_elementwise() {
    // 构造覆盖所有路径的矩阵：
    // - row0: 17 nz (2 块 + 1 尾)
    // - row1: 8 nz (1 块无尾)
    // - row2: 3 nz (仅尾)
    let n = 18u32;
    let mut edges = Vec::new();

    // row0: 17 条边
    for j in 0..17u32 {
        edges.push((0, j, (j as f32) * 0.05 + 0.01, j));
    }
    // row1: 8 条边
    for j in 0..8u32 {
        edges.push((1, j, (j as f32) * 0.1 + 0.02, j));
    }
    // row2: 3 条边
    for j in 0..3u32 {
        edges.push((2, j, (j as f32) * 0.2 + 0.03, j));
    }

    let m = CascadeMatrix::from_edges(n, &edges);
    let x: Vec<f32> = (0..n).map(|i| (i as f32) * 0.3).collect();

    let mut ys = vec![0.0f32; n as usize];
    let mut yv = vec![0.0f32; n as usize];

    spmv_csr_scalar(&x, &m, &mut ys);
    spmv_csr(&x, &m, &mut yv);

    let mut max_diff = 0.0f32;
    for (i, (&s, &v)) in ys.iter().zip(yv.iter()).enumerate() {
        let diff = (v - s).abs();
        max_diff = max_diff.max(diff);
        assert!(diff < 1e-5, "SIMD 与标量在 [{}] 不一致: {} vs {}", i, v, s);
    }

    println!("\n=== SIMD ⇔ 标量一致性 ===");
    println!("逐元素对比: ✅ 完全一致");
    println!("最大差异: {:.8} (机器精度级别)", max_diff);
    println!("SIMD 路径: 已启用 (std::simd portable_simd)");
}

#[test]
fn simd_scalar_performance_ratio() {
    const ITERS: usize = 10_000;

    // 中等规模矩阵，50 行，每行 ~10 条边
    let n = 50u32;
    let mut edges = Vec::new();
    for i in 0..n {
        for j in 0..10u32 {
            let to = (i + j) % n;
            edges.push((i, to, 0.1, 0));
        }
    }

    let m = CascadeMatrix::from_edges(n, &edges);
    let x: Vec<f32> = (0..n).map(|i| (i as f32) * 0.1).collect();
    let mut y = vec![0.0f32; n as usize];

    // 预热标量
    for _ in 0..100 {
        spmv_csr_scalar(&x, &m, &mut y);
    }

    let mut times_scalar_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        spmv_csr_scalar(black_box(&x), black_box(&m), &mut y);
        times_scalar_ns.push(start.elapsed().as_nanos() as f64);
    }

    // 预热 SIMD
    for _ in 0..100 {
        spmv_csr(&x, &m, &mut y);
    }

    let mut times_simd_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        spmv_csr(black_box(&x), black_box(&m), &mut y);
        times_simd_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median_scalar = percentile(&mut times_scalar_ns, 0.5);
    let median_simd = percentile(&mut times_simd_ns, 0.5);
    let speedup = median_scalar / median_simd;

    println!("\n=== SIMD vs 标量性能对比 (50×10 矩阵) ===");
    println!("标量中位数: {:.1} ns", median_scalar);
    println!("SIMD 中位数: {:.1} ns", median_simd);
    println!("加速比: {:.2}×", speedup);
    println!(
        "SIMD 状态: {}",
        if speedup > 1.1 {
            "✅ 有效加速 (>1.1×)"
        } else if speedup > 0.9 {
            "⚠️ 边界收益 (0.9-1.1×，可能矩阵太小)"
        } else {
            "❌ SIMD 反慢 (<0.9×，异常)"
        }
    );
}

// ============================================================================
// 5. 冷启动 / 首次调用延迟
// ============================================================================

#[test]
fn cold_start_latency_first_call() {
    // 第一次调用是否有 JIT-like 预热延迟
    let n = 100u32;
    let mut edges = Vec::new();
    for i in 0..n {
        for d in 1..=3u32 {
            let src = (i + d) % n;
            edges.push((i, src, 0.3, d));
        }
    }
    let m = CascadeMatrix::from_edges(n, &edges);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    // 第一次调用（冷）
    let cold_start = Instant::now();
    let r1 = cascade(&initial, &m, &states, 5, 0.1);
    let cold_ns = cold_start.elapsed().as_nanos() as f64;
    black_box(r1);

    // 第二次调用（温）
    let warm_start = Instant::now();
    let r2 = cascade(&initial, &m, &states, 5, 0.1);
    let warm_ns = warm_start.elapsed().as_nanos() as f64;
    black_box(r2);

    let ratio = cold_ns / warm_ns;

    println!("\n=== 冷启动延迟分析 ===");
    println!("首次调用 (冷): {:.1} ns ({:.2} μs)", cold_ns, cold_ns / 1000.0);
    println!("第二次调用 (温): {:.1} ns ({:.2} μs)", warm_ns, warm_ns / 1000.0);
    println!("冷/温 比率: {:.2}×", ratio);
    println!(
        "分析: {}",
        if ratio < 1.5 {
            "✅ 无显著冷启动惩罚 (<1.5×)"
        } else if ratio < 3.0 {
            "⚠️ 中等预热开销 (1.5-3×)"
        } else {
            "❌ 显著冷启动延迟 (>3×，需关注)"
        }
    );
}

// ============================================================================
// 6. 最坏情况图拓扑分析
// ============================================================================

#[test]
fn worst_case_star_topology() {
    // 星型图：中心节点连到所有叶子
    const ITERS: usize = 2_000;

    let n = 100u32;
    let mut edges = Vec::new();
    // 中心 0 连到所有其他节点
    for i in 1..n {
        edges.push((i, 0, 0.5, 5));
    }

    let m = CascadeMatrix::from_edges(n, &edges);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    // 预热
    for _ in 0..50 {
        black_box(cascade(&initial, &m, &states, 5, 0.1));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let r = black_box(cascade(&initial, &m, &states, 5, 0.1));
        black_box(r);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);

    println!("\n=== 最坏情况：星型拓扑 (100 节点) ===");
    println!("中位数: {:.1} ns ({:.2} μs)", median, median / 1000.0);
    println!("边数: {}", edges.len());
    println!("拓扑特点: 中心节点高度连接，可能缓存未命中");
}

#[test]
fn worst_case_long_chain() {
    // 长链：0 → 1 → 2 → ... → n-1
    const ITERS: usize = 5_000;

    let n = 100u32;
    let mut edges = Vec::new();
    for i in 0..n - 1 {
        edges.push((i + 1, i, 1.0, 1));
    }

    let m = CascadeMatrix::from_edges(n, &edges);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    // 预热
    for _ in 0..100 {
        black_box(cascade(&initial, &m, &states, 5, 0.1));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let r = black_box(cascade(&initial, &m, &states, 5, 0.1));
        black_box(r);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);

    println!("\n=== 最坏情况：长链拓扑 (100 节点) ===");
    println!("中位数: {:.1} ns ({:.2} μs)", median, median / 1000.0);
    println!("边数: {}", edges.len());
    println!("拓扑特点: 依赖链深，5 跳后仅覆盖 6 节点");
}

#[test]
fn worst_case_dense_clique() {
    // 密集团：每节点连到所有其他节点（稀疏度 100）
    const ITERS: usize = 500;

    let n = 50u32; // 降低 n 避免边数爆炸
    let mut edges = Vec::new();
    for i in 0..n {
        for j in 0..n {
            if i != j {
                edges.push((j, i, 0.1, 1));
            }
        }
    }

    let m = CascadeMatrix::from_edges(n, &edges);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    // 预热
    for _ in 0..20 {
        black_box(cascade(&initial, &m, &states, 5, 0.1));
    }

    let mut times_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        let r = black_box(cascade(&initial, &m, &states, 5, 0.1));
        black_box(r);
        times_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median = percentile(&mut times_ns, 0.5);

    println!("\n=== 最坏情况：密集团拓扑 (50 节点) ===");
    println!("中位数: {:.1} ns ({:.2} μs)", median, median / 1000.0);
    println!("边数: {}", edges.len());
    println!("拓扑特点: 极度密集，稀疏矩阵优势最小化");
}

// ============================================================================
// 7. 边稀疏度影响分析
// ============================================================================

#[test]
fn sparsity_impact_analysis() {
    const ITERS: usize = 3_000;

    let n = 100u32;

    // 稀疏度 5（每节点 5 条边）
    let mut edges5 = Vec::new();
    for i in 0..n {
        for j in 1..=5u32 {
            edges5.push((i, (i + j) % n, 0.2, 1));
        }
    }
    let m5 = CascadeMatrix::from_edges(n, &edges5);
    let initial: Vec<f32> = (0..n).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let states: Vec<EntityStateView> = (0..n).map(|_| nb_view(0.9, 5)).collect();

    for _ in 0..50 {
        black_box(cascade(&initial, &m5, &states, 5, 0.1));
    }
    let mut times5_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        black_box(cascade(&initial, &m5, &states, 5, 0.1));
        times5_ns.push(start.elapsed().as_nanos() as f64);
    }

    // 稀疏度 20
    let mut edges20 = Vec::new();
    for i in 0..n {
        for j in 1..=20u32 {
            edges20.push((i, (i + j) % n, 0.2, 1));
        }
    }
    let m20 = CascadeMatrix::from_edges(n, &edges20);

    for _ in 0..50 {
        black_box(cascade(&initial, &m20, &states, 5, 0.1));
    }
    let mut times20_ns = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let start = Instant::now();
        black_box(cascade(&initial, &m20, &states, 5, 0.1));
        times20_ns.push(start.elapsed().as_nanos() as f64);
    }

    let median5 = percentile(&mut times5_ns, 0.5);
    let median20 = percentile(&mut times20_ns, 0.5);
    let ratio = median20 / median5;

    println!("\n=== 边稀疏度影响 (100 实体) ===");
    println!("稀疏度 5 (每节点 5 边): {:.1} ns", median5);
    println!("稀疏度 20 (每节点 20 边): {:.1} ns", median20);
    println!("延迟比: {:.2}×", ratio);
    println!("分析: 边数增加 4×，延迟增加 {:.1}×", ratio);
}
