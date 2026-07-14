//! 脆性语义现实性实验 harness（**不改生产引擎**）。
//!
//! 把 `src/cascade.rs` 的 `confidence_for` / `apply_hop` / `cascade` 私有逻辑
//! vendor（复制）到此处，在其副本上分叉 5 个语义变体（V0-V4），跑 2 个场景
//! （S1 基线 8 实体 / S2 扩展 14 实体），计算指标并打印对比表。
//!
//! 校验锚点：V0 必须复现 `examples/systemic_risk_demo.rs` 的 A/B/C 真实数字。
//!
//! 生产引擎零改动；本 harness 只调用 public API：`spmv_csr`、`CascadeMatrix::from_edges`、
//! `EntityStateView`、`CascadeResult`。

use tessera::cascade::{CascadeResult, EntityStateView};
use tessera::matrix::{spmv_csr, CascadeMatrix};

// ───────────────────────── 变体参数 ─────────────────────────

/// 一个语义变体的配置。所有变体共用同一份 vendor 的 cascade 骨架，靠这三个旋钮区分。
struct Variant {
    name: &'static str,
    /// 突破放大系数 AMP：脆性实体突破时，出向信号 `× AMP`。1.0 = 关闭（= 现状）。
    amp: f32,
    /// 子阈值传导系数 SUB：脆性实体未突破时，置信度按 `raw × SUB` 计（而非 `raw × 0.5^hop`）。
    /// 0.0 = 现状（快速衰减，常被剪枝切断下游）。
    sub_coef: f32,
    /// 是否读取 resilience 向量做韧性吸收（出向 `× (1−ρ)`）。
    use_resilience: bool,
}

const V0: Variant = Variant { name: "V0 现状(对照)", amp: 1.0, sub_coef: 0.0, use_resilience: false };
const V1: Variant = Variant { name: "V1 突破放大", amp: 1.5, sub_coef: 0.0, use_resilience: false };
const V2: Variant = Variant { name: "V2 韧性吸收", amp: 1.0, sub_coef: 0.0, use_resilience: true };
const V3: Variant = Variant { name: "V3 子阈值传导", amp: 1.0, sub_coef: 0.3, use_resilience: false };
const V4: Variant = Variant { name: "V4 组合", amp: 1.5, sub_coef: 0.3, use_resilience: true };

// ───────────────────────── vendor 逻辑（变体化） ─────────────────────────

/// 变体化的 confidence_for（对照 src/cascade.rs:42-53）。
fn conf_for(raw: f32, v: &EntityStateView, hop: u32, sub_coef: f32) -> f32 {
    if v.coordinates > 0.5 {
        // 脆性实体
        if raw >= v.brittle_threshold {
            1.0 // 突破，置信度透传
        } else if sub_coef > 0.0 {
            // V3：未突破也按固定系数传导（不快速衰减到剪枝）
            raw * sub_coef
        } else {
            raw * 0.5f32.powi(hop as i32) // 现状：快速衰减
        }
    } else {
        raw * v.decay_coefficient
    }
}

/// 变体化的 apply_hop（对照 src/cascade.rs:59-89）。关键分叉点：`propagated[i]` 的赋值。
#[allow(clippy::too_many_arguments)]
fn apply_hop_v(
    signal: &[f32],
    hop: u32,
    states: &[EntityStateView],
    resilience: &[f32],
    theta: f32,
    cfg: &Variant,
    first_hop: &mut [Option<u32>],
    acc_lag: &mut [u32],
    best_conf: &mut [f32],
    propagated: &mut [f32],
) {
    for i in 0..signal.len() {
        let raw = signal[i];
        if raw == 0.0 || i >= states.len() {
            propagated[i] = 0.0;
            continue;
        }
        let c = conf_for(raw, &states[i], hop, cfg.sub_coef);
        if c >= theta {
            if first_hop[i].is_none() {
                first_hop[i] = Some(hop);
            }
            acc_lag[i] = acc_lag[i].saturating_add(states[i].time_lag_us);
            if c > best_conf[i] {
                best_conf[i] = c;
            }
            // —— 出向幅度（现状恒为 raw；变体在此分叉）——
            let mut mag = raw;
            // V1：脆性突破放大出向
            if cfg.amp > 1.0 && states[i].coordinates > 0.5 && raw >= states[i].brittle_threshold {
                mag *= cfg.amp;
            }
            // V2：韧性吸收（与 V1 可复合）
            if cfg.use_resilience && i < resilience.len() {
                mag *= 1.0 - resilience[i];
            }
            propagated[i] = mag;
        } else {
            propagated[i] = 0.0; // 剪枝
        }
    }
}

/// 变体化的 cascade（对照 src/cascade.rs:100-155）。返回 (结果, total_energy)。
fn cascade_v(
    initial: &[f32],
    matrix: &CascadeMatrix,
    states: &[EntityStateView],
    resilience: &[f32],
    max_hops: u32,
    theta: f32,
    cfg: &Variant,
) -> (Vec<CascadeResult>, f64) {
    let n = matrix.n as usize;
    let mut cur = initial.to_vec();
    cur.resize(n, 0.0);
    let mut next = vec![0.0f32; n];
    let mut raw_buf = vec![0.0f32; n];
    let mut first_hop: Vec<Option<u32>> = vec![None; n];
    let mut acc_lag = vec![0u32; n];
    let mut best_conf = vec![0.0f32; n];
    let mut energy: f64 = 0.0;

    apply_hop_v(&cur, 0, states, resilience, theta, cfg, &mut first_hop, &mut acc_lag, &mut best_conf, &mut next);
    energy += next.iter().map(|&x| x as f64).sum::<f64>();
    core::mem::swap(&mut cur, &mut next);

    for hop in 1..=max_hops {
        spmv_csr(&cur, matrix, &mut raw_buf);
        apply_hop_v(&raw_buf, hop, states, resilience, theta, cfg, &mut first_hop, &mut acc_lag, &mut best_conf, &mut next);
        energy += next.iter().map(|&x| x as f64).sum::<f64>();
        core::mem::swap(&mut cur, &mut next);
    }

    let results: Vec<CascadeResult> = (0..n)
        .filter_map(|i| {
            first_hop[i].map(|h| CascadeResult {
                entity_id: i as u32,
                confidence: best_conf[i],
                hop: h,
                lag_us: acc_lag[i],
            })
        })
        .collect();
    (results, energy)
}

// ───────────────────────── 指标 ─────────────────────────

struct Metrics {
    reach: usize,
    n: usize,
    terminal_hit: bool,
    terminal_conf: f32,
    brittle_conf: f32,
    energy: f64,
}

fn metrics(results: &[CascadeResult], n: usize, terminal_id: u32, brittle_id: u32, energy: f64) -> Metrics {
    let terminal_conf = results
        .iter()
        .find(|r| r.entity_id == terminal_id)
        .map(|r| r.confidence)
        .unwrap_or(0.0);
    let brittle_conf = results
        .iter()
        .find(|r| r.entity_id == brittle_id)
        .map(|r| r.confidence)
        .unwrap_or(0.0);
    Metrics {
        reach: results.len(),
        n,
        terminal_hit: terminal_conf > 0.0,
        terminal_conf,
        brittle_conf,
        energy,
    }
}

// ───────────────────────── S1：基线 8 实体（复现 demo） ─────────────────────────

fn flow_edge(src: u32, dst: u32, w: f32) -> (u32, u32, f32, u32) {
    (dst, src, w, 0) // 翻转：引擎语义 (from=row=dst, to=col=src)，信号 src→dst
}

fn s1_matrix() -> CascadeMatrix {
    CascadeMatrix::from_edges(
        8,
        &[
            flow_edge(0, 1, 0.60),
            flow_edge(0, 2, 0.70),
            flow_edge(0, 3, 0.80),
            flow_edge(1, 4, 0.70),
            flow_edge(2, 5, 0.60),
            flow_edge(3, 6, 0.85),
            flow_edge(6, 7, 0.70),
        ],
    )
}

fn s1_shock() -> Vec<f32> {
    let mut s = vec![0.0f32; 8];
    s[0] = 1.0;
    s
}

/// #6 的三种画像；其余实体恒为非脆性。返回 (states, resilience全0)。
fn s1_states(profile: &str) -> (Vec<EntityStateView>, Vec<f32>) {
    const LAGS: [u32; 8] = [0, 80, 200, 150, 100, 120, 60, 90];
    let normal = |i: usize| EntityStateView {
        coordinates: 0.3,
        brittle_threshold: 0.9,
        decay_coefficient: 0.6,
        time_lag_us: LAGS[i],
    };
    let mut v: Vec<EntityStateView> = (0..8).map(normal).collect();
    v[6] = match profile {
        "brittle_easy" => EntityStateView { coordinates: 0.9, brittle_threshold: 0.40, decay_coefficient: 0.5, time_lag_us: LAGS[6] },
        "brittle_hard" => EntityStateView { coordinates: 0.9, brittle_threshold: 0.70, decay_coefficient: 0.5, time_lag_us: LAGS[6] },
        _ => normal(6), // non_brittle
    };
    (v, vec![0.0; 8]) // S1 无吸收器，resilience 全 0
}

// ───────────────────────── S2：扩展 14 实体（补充场景） ─────────────────────────
//
// 0  北极星宏观基金(冲击源1)     7  黑石旗舰ETF
// 1  银河流动性做市              8  先锋被动巨擘
// 2  鼎盛因子策略                9  对冲基金B(冲击源2)
// 3  华尔街主经纪商             10  区域银行链
// 4  中投主权基金(吸收器 ρ=0.7) 11  养老金终端A
// 5  暗物质家办A(脆性·低阈值)   12  养老金终端B
// 6  黑曜石家办B(脆性·低阈值)   13  监管熔断阀(吸收器 ρ=0.7)

fn s2_matrix() -> CascadeMatrix {
    CascadeMatrix::from_edges(
        14,
        &[
            flow_edge(0, 1, 0.60),  // 北极星→做市
            flow_edge(0, 2, 0.70),  // 北极星→因子
            flow_edge(0, 3, 0.80),  // 北极星→主经纪商
            flow_edge(1, 4, 0.65),  // 做市→主权吸收器
            flow_edge(3, 5, 0.85),  // 主经纪商→家办A(脆)
            flow_edge(3, 6, 0.80),  // 主经纪商→家办B(脆)
            flow_edge(5, 7, 0.70),  // 家办A→ETF
            flow_edge(6, 8, 0.65),  // 家办B→被动
            flow_edge(5, 6, 0.55),  // 家办A→家办B(脆间传染)
            flow_edge(6, 5, 0.55),  // 家办B→家办A(回边 → 5↔6 循环，近似反馈)
            flow_edge(7, 11, 0.75), // ETF→养老金A
            flow_edge(8, 12, 0.70), // 被动→养老金B
            flow_edge(9, 2, 0.60),  // 第二冲击源→因子
            flow_edge(4, 10, 0.40), // 主权(吸收后残余)→区域银行
            flow_edge(10, 11, 0.50),// 区域银行→养老金A
            flow_edge(6, 13, 0.50), // 家办B→监管熔断阀
        ],
    )
}

fn s2_shock() -> Vec<f32> {
    let mut s = vec![0.0f32; 14];
    s[0] = 1.0; // 主冲击
    s[9] = 0.8; // 第二冲击源
    s
}

fn s2_states() -> (Vec<EntityStateView>, Vec<f32>) {
    let normal = |lag: u32| EntityStateView {
        coordinates: 0.3, brittle_threshold: 0.9, decay_coefficient: 0.6, time_lag_us: lag,
    };
    let brittle_low = |thr: f32, lag: u32| EntityStateView {
        coordinates: 0.9, brittle_threshold: thr, decay_coefficient: 0.5, time_lag_us: lag,
    };
    let mut v = vec![normal(0); 14];
    let lags = [0, 80, 200, 150, 100, 60, 60, 100, 120, 90, 110, 90, 90, 70];
    for i in 0..14 {
        v[i] = normal(lags[i]);
    }
    // 两个脆性杠杆节点
    v[5] = brittle_low(0.40, lags[5]); // 家办A
    v[6] = brittle_low(0.40, lags[6]); // 家办B
    // 韧性吸收器（仅在 V2/V4 生效，ρ 经 resilience 向量传入）
    let mut resilience = vec![0.0f32; 14];
    resilience[4] = 0.7; // 主权基金
    resilience[13] = 0.7; // 监管熔断阀
    (v, resilience)
}

// ───────────────────────── 报告输出 ─────────────────────────

fn pct(x: usize, n: usize) -> String {
    format!("{}/{}", x, n)
}

fn run_s1() {
    println!("\n{}", "═".repeat(70));
    println!("场景 S1：基线 8 实体（复现 systemic_risk_demo.rs）");
    println!("{}", "═".repeat(70));

    let m = s1_matrix();
    let shock = s1_shock();
    // #6 的三种画像
    for (profile, label) in [
        ("brittle_easy", "A 脆性·易破(thr0.40)"),
        ("non_brittle", "B 非脆性(衰减0.6)"),
        ("brittle_hard", "C 脆性·难破(thr0.70)"),
    ] {
        let (states, res) = s1_states(profile);
        println!("\n── 方案 {label} ──  #6 在各变体下：");
        println!("  {:<22} {:>10} {:>10} {:>12} {:>12} {:>10}",
            "变体", "#6 conf", "#7 conf", "terminal#7", "reach", "energy");
        for cfg in [&V0, &V1, &V3, &V4] {
            let (r, e) = cascade_v(&shock, &m, &states, &res, 5, 0.20, cfg);
            let mm = metrics(&r, 8, 7, 6, e);
            let term = if mm.terminal_hit { format!("命中") } else { "未命中".into() };
            println!("  {:<22} {:>10.3} {:>10.3} {:>12} {:>10} {:>10.3}",
                cfg.name, mm.brittle_conf, mm.terminal_conf, term,
                pct(mm.reach, mm.n), mm.energy);
        }
    }

    println!("\n── 诊断 H1（V0 突破是否放大下游？）──");
    let (sa, ra) = s1_states("brittle_easy");
    let (sb, rb) = s1_states("non_brittle");
    let (_, ea_a) = cascade_v(&shock, &m, &sa, &ra, 5, 0.20, &V0);
    let (_, ea_b) = cascade_v(&shock, &m, &sb, &rb, 5, 0.20, &V0);
    let (ra_v1, _) = cascade_v(&shock, &m, &sa, &ra, 5, 0.20, &V1);
    let t7_v0a = metrics(&cascade_v(&shock, &m, &sa, &ra, 5, 0.20, &V0).0, 8, 7, 6, 0.0).terminal_conf;
    let t7_v0b = metrics(&cascade_v(&shock, &m, &sb, &rb, 5, 0.20, &V0).0, 8, 7, 6, 0.0).terminal_conf;
    let t7_v1a = metrics(&ra_v1, 8, 7, 6, 0.0).terminal_conf;
    println!("  V0: 方案A(#6突破,conf1.0) terminal#7 = {:.3}  vs  方案B(#6非脆,conf0.408) = {:.3}", t7_v0a, t7_v0b);
    println!("      energy A = {:.3}  vs  energy B = {:.3}   (能量近乎相同 → 突破未放大)", ea_a, ea_b);
    println!("  V1: 方案A(amp=1.5) terminal#7 = {:.3}   (> V0 的 {:.3} → 突破放大下游)", t7_v1a, t7_v0a);

    println!("\n── 诊断 H3（V3 是否消除子阈值黑洞？）──");
    let (sc, rc) = s1_states("brittle_hard");
    let t7_v0c = metrics(&cascade_v(&shock, &m, &sc, &rc, 5, 0.20, &V0).0, 8, 7, 6, 0.0);
    let t7_v3c = metrics(&cascade_v(&shock, &m, &sc, &rc, 5, 0.20, &V3).0, 8, 7, 6, 0.0);
    println!("  方案C(#6脆性·难破): V0 terminal#7 {} (conf {:.3})  →  V3 terminal#7 {} (conf {:.3})",
        if t7_v0c.terminal_hit {"命中"} else {"未命中"}, t7_v0c.terminal_conf,
        if t7_v3c.terminal_hit {"命中"} else {"未命中"}, t7_v3c.terminal_conf);
}

fn run_s2() {
    println!("\n{}", "═".repeat(70));
    println!("场景 S2：扩展 14 实体（补充场景，含吸收器 + 双脆性节点 + 回边循环）");
    println!("{}", "═".repeat(70));

    let m = s2_matrix();
    let shock = s2_shock();
    let (states, res) = s2_states();
    let terminal_ids = [11u32, 12]; // 养老金 A/B
    let brittle_ids = [5u32, 6];

    println!("\n── H4：各变体在 S2 上的系统性指标 ──");
    println!("  {:<22} {:>8} {:>10} {:>12} {:>12} {:>10}",
        "变体", "reach", "养老金", "家办A conf", "家办B conf", "energy");
    for cfg in [&V0, &V1, &V2, &V4] {
        let (r, e) = cascade_v(&shock, &m, &states, &res, 5, 0.20, cfg);
        let pension_hit = r.iter().any(|x| terminal_ids.contains(&x.entity_id));
        let a_conf = r.iter().find(|x| x.entity_id == brittle_ids[0]).map(|x| x.confidence).unwrap_or(0.0);
        let b_conf = r.iter().find(|x| x.entity_id == brittle_ids[1]).map(|x| x.confidence).unwrap_or(0.0);
        println!("  {:<22} {:>8} {:>10} {:>12.3} {:>12.3} {:>10.3}",
            cfg.name, pct(r.len(), 14),
            if pension_hit {"命中"} else {"未命中"}, a_conf, b_conf, e);
    }

    println!("\n── 诊断 H2（V2 韧性吸收器是否降低下游？）──");
    // 对照：把吸收器 ρ 置 0（关掉韧性）跑 V2
    let res_off = vec![0.0f32; 14];
    let (r_on, e_on) = cascade_v(&shock, &m, &states, &res, 5, 0.20, &V2);
    let (r_off, e_off) = cascade_v(&shock, &m, &states, &res_off, 5, 0.20, &V2);
    let pension_on = r_on.iter().any(|x| terminal_ids.contains(&x.entity_id));
    let pension_off = r_off.iter().any(|x| terminal_ids.contains(&x.entity_id));
    println!("  吸收器 ρ=0.7：reach {}  energy {:.3}  养老金命中 {}",
        pct(r_on.len(), 14), e_on, if pension_on {"是"} else {"否"});
    println!("  吸收器 ρ=0.0：reach {}  energy {:.3}  养老金命中 {}",
        pct(r_off.len(), 14), e_off, if pension_off {"是"} else {"否"});
    println!("  → 韧性吸收器降低 energy {:.3}→{:.3}（差 {:.3}）",
        e_off, e_on, e_off - e_on);
}

// ───────────────────────── S3：舆情研判 13 实体（跨域验证） ─────────────────────────
//
// 与 S2（金融）**同构**、领域不同：证明引擎核心是领域无关的级联动力学。
// 0 匿名爆料源(冲击1)      7 平台算法推荐位
// 1 吃瓜大V                 8 利益相关方(当事人)
// 2 行业自媒体              9 第二爆料源(冲击2)
// 3 主流财经媒体           10 行业协会/监管(吸收器 ρ=0.7)
// 4 官方辟谣节点(吸收器)   11 大众舆论终端A
// 5 极化社区A(脆·低阈值)   12 大众舆论终端B
// 6 极化社区B(脆·低阈值)

fn s3_matrix() -> CascadeMatrix {
    CascadeMatrix::from_edges(
        13,
        &[
            flow_edge(0, 1, 0.60), // 爆料→大V
            flow_edge(0, 2, 0.70), // 爆料→自媒体
            flow_edge(0, 3, 0.80), // 爆料→主流媒体
            flow_edge(3, 4, 0.60), // 媒体→辟谣（把吸收器放上活路径，否则无入边失效）
            flow_edge(1, 5, 0.75), // 大V→极化社区A
            flow_edge(3, 6, 0.70), // 媒体→极化社区B
            flow_edge(5, 6, 0.55), // 社区互传染
            flow_edge(6, 5, 0.55), // 回边循环（近似舆情回声室反馈）
            flow_edge(5, 7, 0.70), // 社区→平台推荐
            flow_edge(7, 11, 0.75),// 平台→大众A
            flow_edge(6, 12, 0.70),// 社区→大众B
            flow_edge(9, 2, 0.60), // 第二爆料→自媒体
            flow_edge(4, 7, 0.50), // 辟谣→平台（吸收）
            flow_edge(8, 3, 0.50), // 当事人→媒体
            flow_edge(6, 10, 0.50),// 社区→监管
        ],
    )
}

fn s3_shock() -> Vec<f32> {
    let mut s = vec![0.0f32; 13];
    s[0] = 1.0; // 主爆料
    s[9] = 0.7; // 第二爆料
    s
}

fn s3_states() -> (Vec<EntityStateView>, Vec<f32>) {
    let normal = |lag: u32| EntityStateView {
        coordinates: 0.3, brittle_threshold: 0.9, decay_coefficient: 0.6, time_lag_us: lag,
    };
    let brittle_low = |lag: u32| EntityStateView {
        coordinates: 0.9, brittle_threshold: 0.40, decay_coefficient: 0.5, time_lag_us: lag,
    };
    let lags = [0, 80, 150, 200, 100, 60, 60, 90, 110, 90, 100, 90, 90];
    let mut v: Vec<EntityStateView> = lags.iter().map(|&l| normal(l)).collect();
    v[5] = brittle_low(lags[5]); // 极化社区A
    v[6] = brittle_low(lags[6]); // 极化社区B
    let mut resilience = vec![0.0f32; 13];
    resilience[4] = 0.7; // 官方辟谣
    resilience[10] = 0.7; // 行业监管
    (v, resilience)
}

fn run_s3() {
    println!("\n{}", "═".repeat(70));
    println!("场景 S3：舆情研判 13 实体（跨域验证 —— 与 S2 同构，领域不同）");
    println!("{}", "═".repeat(70));

    let m = s3_matrix();
    let shock = s3_shock();
    let (states, res) = s3_states();
    let terminals = [11u32, 12];
    let brittle = [5u32, 6];

    println!("\n── 各变体在 S3 上的指标（对照 S2 金融的同构表）──");
    println!("  {:<22} {:>8} {:>10} {:>12} {:>12} {:>10}",
        "变体", "reach", "大众", "社区A conf", "社区B conf", "energy");
    for cfg in [&V0, &V1, &V2, &V4] {
        let (r, e) = cascade_v(&shock, &m, &states, &res, 5, 0.20, cfg);
        let mass = r.iter().any(|x| terminals.contains(&x.entity_id));
        let a = r.iter().find(|x| x.entity_id == brittle[0]).map(|x| x.confidence).unwrap_or(0.0);
        let b = r.iter().find(|x| x.entity_id == brittle[1]).map(|x| x.confidence).unwrap_or(0.0);
        println!("  {:<22} {:>8} {:>10} {:>12.3} {:>12.3} {:>10.3}",
            cfg.name, pct(r.len(), 13),
            if mass {"命中"} else {"未命中"}, a, b, e);
    }

    // 跨域对照：同构拓扑在金融/舆情两域的突破放大比
    let s2m = s2_matrix();
    let (s2st, s2re) = s2_states();
    let s2shock = s2_shock();
    let (_, e0_s2) = cascade_v(&s2shock, &s2m, &s2st, &s2re, 5, 0.20, &V0);
    let (_, e1_s2) = cascade_v(&s2shock, &s2m, &s2st, &s2re, 5, 0.20, &V1);
    let (_, e0_s3) = cascade_v(&shock, &m, &states, &res, 5, 0.20, &V0);
    let (_, e1_s3) = cascade_v(&shock, &m, &states, &res, 5, 0.20, &V1);
    println!("\n── 跨域对照（V1 energy / V0 energy = 突破放大比）──");
    println!("  S2 金融(赎回潮):  V0={:.3}  V1={:.3}  比={:.2}×", e0_s2, e1_s2, e1_s2 / e0_s2);
    println!("  S3 舆情(爆料级联): V0={:.3}  V1={:.3}  比={:.2}×", e0_s3, e1_s3, e1_s3 / e0_s3);
    println!("  → 两域同构：脆性节点(杠杆家办 / 极化社区)被点燃后均酿成传染螺旋");
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   Tessera 脆性语义现实性实验（5 变体 × 3 场景）        ║");
    println!("║   不改生产引擎；vendor src/cascade.rs 私有逻辑分叉变体   ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("θ = 0.20，max_hops = 5（与 systemic_risk_demo.rs 一致）");

    run_s1();
    run_s2();
    run_s3();

    println!("\n{}", "─".repeat(70));
    println!("指标说明：");
    println!("  reach    = 命中实体数/总数");
    println!("  conf     = 命中置信度（脆性突破→1.0）");
    println!("  energy   = Σ_各跳 Σ_i 出向信号幅度（系统性应力积分，新指标）");
    println!("  terminal = 终端养老金是否被波及");
}
