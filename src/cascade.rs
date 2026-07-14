//! 级联推理引擎（SPEC §6.3 / §2.5）。
//!
//! 沿 [`CascadeMatrix`] 传播冲击信号，每跳用 SIMD SpMV；对脆性实体施加特殊阈值逻辑：
//! 未突破阈值时快速衰减，突破后置信度透传（=1.0）。

use crate::matrix::{spmv_csr, CascadeMatrix};

/// 实体在级联中的状态视图。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EntityStateView {
    /// 脆性坐标（`> 0.5` → 该实体为脆性）。
    pub coordinates: f32,
    /// 脆性突破阈值：脆性实体收到冲击 `>= brittle_threshold` 时突破（置信度透传 1.0）。
    pub brittle_threshold: f32,
    /// 非脆性实体的衰减系数（每跳 `impact * decay_coefficient`）。
    pub decay_coefficient: f32,
    /// 该实体每跳的传播时滞（微秒）。
    pub time_lag_us: u32,
}

/// 单个实体的级联结果。
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct CascadeResult {
    /// 实体 id。
    pub entity_id: u32,
    /// 命中置信度（各跳最大值）。
    pub confidence: f32,
    /// 首次命中跳数。
    pub hop: u32,
    /// 累积时滞（微秒）。
    pub lag_us: u32,
}

/// 计算单实体在某跳的置信度（含脆性分支）。
///
/// - 脆性（`coordinates > 0.5`）：
///   - `raw >= brittle_threshold` → 突破，置信度 = 1.0（透传不衰减）。
///   - 否则 → 快速衰减 `raw * 0.5^hop`。
/// - 非脆性 → `raw * decay_coefficient`。
fn confidence_for(raw: f32, view: &EntityStateView, hop: u32) -> f32 {
    if view.coordinates > 0.5 {
        // 脆性实体
        if raw >= view.brittle_threshold {
            1.0 // 突破阈值，置信度透传
        } else {
            raw * 0.5f32.powi(hop as i32) // 未突破，快速衰减
        }
    } else {
        raw * view.decay_coefficient
    }
}

/// 对一跳的原始信号计算置信度、更新追踪状态，并输出**剪枝后**的传播向量。
///
/// `confidence < theta` 的实体剪枝置 0（不记录、不继续传播）。
#[allow(clippy::too_many_arguments)]
fn apply_hop(
    signal: &[f32],
    hop: u32,
    entity_states: &[EntityStateView],
    theta: f32,
    first_hop: &mut [Option<u32>],
    acc_lag: &mut [u32],
    best_conf: &mut [f32],
    propagated: &mut [f32],
) {
    for i in 0..signal.len() {
        let raw = signal[i];
        if raw == 0.0 || i >= entity_states.len() {
            propagated[i] = 0.0;
            continue;
        }
        let conf = confidence_for(raw, &entity_states[i], hop);
        if conf >= theta {
            if first_hop[i].is_none() {
                first_hop[i] = Some(hop);
            }
            acc_lag[i] = acc_lag[i].saturating_add(entity_states[i].time_lag_us);
            if conf > best_conf[i] {
                best_conf[i] = conf;
            }
            propagated[i] = raw; // 保留传播
        } else {
            propagated[i] = 0.0; // 剪枝置 0
        }
    }
}

/// 级联推理。
///
/// - `initial`：初始冲击信号（长度应 ≥ `matrix.n`；不足时尾部填 0 容错，debug 下断言）。
/// - 每跳 `spmv_csr` 传播；脆性分支见 [`confidence_for`]。
/// - `confidence < theta` 剪枝置 0。
/// - 返回所有命中实体（首次命中跳数 + 累积时滞 + 最佳置信度）。
///
/// matrix 边语义：`spmv_csr` 下 `next[i] = Σ_{row i} w·cur[col]`，即冲击沿
/// `col → row` 方向流动（`from_edges` 中 `(from=i, to=j)` 表示 `j → i`）。
pub fn cascade(
    initial: &[f32],
    matrix: &CascadeMatrix,
    entity_states: &[EntityStateView],
    max_hops: u32,
    theta: f32,
) -> Vec<CascadeResult> {
    let n = matrix.n as usize;
    let mut cur: Vec<f32> = initial.to_vec();
    cur.resize(n, 0.0);
    debug_assert!(initial.len() >= n, "cascade: initial.len()={} < matrix.n={}", initial.len(), matrix.n);
    let mut next: Vec<f32> = vec![0.0; n];
    let mut raw_buf: Vec<f32> = vec![0.0; n];
    let mut first_hop: Vec<Option<u32>> = vec![None; n];
    let mut acc_lag: Vec<u32> = vec![0; n];
    let mut best_conf: Vec<f32> = vec![0.0; n];

    // hop 0：初始信号（剪枝后作为传播源）
    apply_hop(
        &cur,
        0,
        entity_states,
        theta,
        &mut first_hop,
        &mut acc_lag,
        &mut best_conf,
        &mut next,
    );
    core::mem::swap(&mut cur, &mut next);

    for hop in 1..=max_hops {
        spmv_csr(&cur, matrix, &mut raw_buf);
        apply_hop(
            &raw_buf,
            hop,
            entity_states,
            theta,
            &mut first_hop,
            &mut acc_lag,
            &mut best_conf,
            &mut next,
        );
        core::mem::swap(&mut cur, &mut next);
    }

    (0..n)
        .filter_map(|i| {
            first_hop[i].map(|h| CascadeResult {
                entity_id: i as u32,
                confidence: best_conf[i],
                hop: h,
                lag_us: acc_lag[i],
            })
        })
        .collect()
}

#[cfg(test)]
mod cascade_tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    /// 非脆性 view。
    fn nb_view(decay: f32, lag: u32) -> EntityStateView {
        EntityStateView {
            coordinates: 0.0,
            brittle_threshold: 0.0,
            decay_coefficient: decay,
            time_lag_us: lag,
        }
    }

    /// 脆性 view。
    fn brit_view(brittle_threshold: f32, lag: u32) -> EntityStateView {
        EntityStateView {
            coordinates: 0.7,
            brittle_threshold,
            decay_coefficient: 0.9,
            time_lag_us: lag,
        }
    }

    #[test]
    fn star_leaves_receive_decayed_signal() {
        // 中心 0 → 叶 1/2/3。spmv 下 from=叶,to=中心 表示 中心→叶。
        let edges = vec![(1, 0, 0.5, 0), (2, 0, 0.5, 0), (3, 0, 0.5, 0)];
        let m = CascadeMatrix::from_edges(4, &edges);
        let initial = vec![1.0f32, 0.0, 0.0, 0.0];
        let states = vec![nb_view(1.0, 10); 4];
        let res = cascade(&initial, &m, &states, 2, 0.1);
        let by_id: std::collections::HashMap<u32, CascadeResult> =
            res.into_iter().map(|r| (r.entity_id, r)).collect();
        // 中心 hop0 conf=1.0
        let center = by_id.get(&0).unwrap();
        assert_eq!(center.hop, 0);
        assert!((center.confidence - 1.0).abs() < 1e-5);
        // 叶子 hop1 conf = w*decay = 0.5*1.0 = 0.5
        for id in 1..=3 {
            let leaf = by_id.get(&id).unwrap();
            assert_eq!(leaf.hop, 1, "leaf {id} hop");
            assert!(
                (leaf.confidence - 0.5).abs() < 1e-5,
                "leaf {id} conf {} (期望 0.5)",
                leaf.confidence
            );
            assert_eq!(leaf.lag_us, 10);
        }
    }

    #[test]
    fn brittle_breakthrough_confidence_is_one() {
        // 脆性叶：brittle_threshold=0.3，raw=0.5 >= 0.3 → 突破 conf=1.0
        let edges = vec![(1, 0, 0.5, 0)];
        let m = CascadeMatrix::from_edges(2, &edges);
        let initial = vec![1.0f32, 0.0];
        let states = vec![nb_view(1.0, 0), brit_view(0.3, 0)];
        let res = cascade(&initial, &m, &states, 1, 0.1);
        let leaf = res.iter().find(|r| r.entity_id == 1).unwrap();
        assert!(
            (leaf.confidence - 1.0).abs() < 1e-5,
            "脆性突破应 conf=1.0, got {}",
            leaf.confidence
        );
    }

    #[test]
    fn brittle_unbroken_decays_faster_than_nonbrittle() {
        // 两个叶：raw 相同=0.5。脆性(thr=0.8 未突破) vs 非脆性(decay=0.9)
        let edges = vec![(1, 0, 0.5, 0), (2, 0, 0.5, 0)];
        let m = CascadeMatrix::from_edges(3, &edges);
        let initial = vec![1.0f32, 0.0, 0.0];
        let states = vec![
            nb_view(1.0, 0),
            brit_view(0.8, 0), // 脆性未突破：0.5*0.5^1 = 0.25
            nb_view(0.9, 0),   // 非脆性：0.5*0.9 = 0.45
        ];
        let res = cascade(&initial, &m, &states, 1, 0.1);
        let brittle = res.iter().find(|r| r.entity_id == 1).unwrap();
        let nonbrit = res.iter().find(|r| r.entity_id == 2).unwrap();
        assert!(
            (brittle.confidence - 0.25).abs() < 1e-5,
            "got {}",
            brittle.confidence
        );
        assert!(
            (nonbrit.confidence - 0.45).abs() < 1e-5,
            "got {}",
            nonbrit.confidence
        );
        assert!(brittle.confidence < nonbrit.confidence, "脆性应衰减更快");
    }

    #[test]
    fn lag_accumulates_over_hops() {
        // 2-环 0↔1，w=1.0，decay=1.0：信号反复传播，lag 逐跳累积
        let edges = vec![(0, 1, 1.0, 0), (1, 0, 1.0, 0)];
        let m = CascadeMatrix::from_edges(2, &edges);
        let initial = vec![1.0f32, 0.0];
        let states = vec![
            EntityStateView {
                coordinates: 0.0,
                brittle_threshold: 0.0,
                decay_coefficient: 1.0,
                time_lag_us: 10,
            },
            EntityStateView {
                coordinates: 0.0,
                brittle_threshold: 0.0,
                decay_coefficient: 1.0,
                time_lag_us: 20,
            },
        ];
        let res = cascade(&initial, &m, &states, 3, 0.1);
        let by_id: std::collections::HashMap<u32, CascadeResult> =
            res.into_iter().map(|r| (r.entity_id, r)).collect();
        let e0 = by_id.get(&0).unwrap();
        let e1 = by_id.get(&1).unwrap();
        assert_eq!(e0.hop, 0);
        assert_eq!(e1.hop, 1);
        // e0 命中 hop0+hop2 → lag=2*10=20；e1 命中 hop1+hop3 → lag=2*20=40
        assert_eq!(e0.lag_us, 20, "e0 lag 应累积为 20");
        assert_eq!(e1.lag_us, 40, "e1 lag 应累积为 40");
    }

    #[test]
    fn sub_theta_signal_pruned() {
        // w=0.05，非脆性 decay=1.0 → conf=0.05 < theta=0.1 → 剪枝，叶不出现在结果
        let edges = vec![(1, 0, 0.05, 0)];
        let m = CascadeMatrix::from_edges(2, &edges);
        let initial = vec![1.0f32, 0.0];
        let states = vec![nb_view(1.0, 0); 2];
        let res = cascade(&initial, &m, &states, 1, 0.1);
        assert!(
            res.iter().all(|r| r.entity_id != 1),
            "置信度<theta 应被剪枝"
        );
        assert!(res.iter().any(|r| r.entity_id == 0)); // 源保留
    }

    #[test]
    fn cascade_100_entities_5_hops_under_100us() {
        // 100 实体稀疏图，5 跳，release 模式 <100μs（dev 模式给 2x 余量）
        let n = 100u32;
        let mut edges = Vec::new();
        // 每实体连到后 3 个（环状），from=接收方
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
            let _ = black_box(cascade(&initial, &m, &states, 5, 0.1));
        }
        let iters = 5_000u64;
        let start = Instant::now();
        for _ in 0..iters {
            let r = black_box(cascade(
                black_box(&initial),
                black_box(&m),
                black_box(&states),
                5,
                0.1,
            ));
            black_box(r);
        }
        let ns = start.elapsed().as_nanos() as f64 / iters as f64;
        // dev 模式用宽松上界避免 CI 抖动；release 应远低于 100μs
        assert!(ns < 100_000.0, "100实体5跳级联 {ns:.0}ns >= 100μs");
    }
}
