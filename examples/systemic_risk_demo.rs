//! Σ⁴-Engine 复杂示例（虚构）：系统级联风险压力测试。
//!
//! 场景：某量化巨头（北极星宏观基金）遭遇 $50B 同步赎回潮 → 冲击沿
//! 流动性做市 / 因子共振 / 主经纪商信用 三条链级联传导。
//!
//! 核心演示：**同一拓扑 + 同一冲击，仅因实体 #6（暗物质杠杆家办）的「脆性画像」不同，
//! 产出三种截然不同的系统性风险判定。** 脆性实体在此充当「临界点门控」——
//! 未突破阈值时快速衰减（甚至剪枝、切断下游传播），突破后置信度透传 1.0（标记为系统级关键节点）。

use sigma4_engine::cascade::{cascade, EntityStateView};
use sigma4_engine::matrix::CascadeMatrix;

/// 8 个虚构资本主体（原型化命名）。
const NAMES: [&str; 8] = [
    "北极星宏观基金", // 0 — 冲击源：量化巨头遭遇同步赎回潮
    "银河流动性做市", // 1 — 吸收初始抛压
    "鼎盛因子策略",   // 2 — 因子暴露共振
    "华尔街主经纪商", // 3 — prime brokerage 信用敞口
    "黑石旗舰ETF",    // 4 — ETF 申赎链条
    "先锋被动巨擘",   // 5 — 被动指数跟跌
    "暗物质杠杆家办", // 6 — 高杠杆 + 集中仓位（脆性节点）
    "全球养老金联盟", // 7 — 终端受益人
];

/// 信号流方向的边（src → dst）。
///
/// 引擎矩阵语义见 `cascade.rs:98-99`：`spmv_csr` 下信号沿 `col → row` 流动，
/// `from_edges` 中 `(from=i, to=j)` 表示 `j → i`。故此封装把直觉的 `src→dst`
/// 翻转为 `(from=dst, to=src)`，调用方仍按自然方向书写。
fn flow_edge(src: u32, dst: u32, w: f32, _lag: u32) -> (u32, u32, f32, u32) {
    (dst, src, w, _lag)
}

fn build_matrix() -> CascadeMatrix {
    let flow = [
        flow_edge(0, 1, 0.60, 80),  // 北极星 → 银河做市
        flow_edge(0, 2, 0.70, 200), // 北极星 → 鼎盛因子（共振）
        flow_edge(0, 3, 0.80, 150), // 北极星 → 主经纪商（信用）
        flow_edge(1, 4, 0.70, 100), // 做市 → ETF 申赎
        flow_edge(2, 5, 0.60, 120), // 因子 → 被动跟跌
        flow_edge(3, 6, 0.85, 60),  // 主经纪商 → 杠杆家办（强传导）
        flow_edge(6, 7, 0.70, 90),  // 家办被迫平仓 → 养老金
    ];
    CascadeMatrix::from_edges(8, &flow)
}

fn initial_shock() -> Vec<f32> {
    let mut s = vec![0.0f32; 8];
    s[0] = 1.0; // 北极星首发满强度冲击
    s
}

/// 构造实体状态视图；entity #6 的脆性画像随 `config` 变化，其余实体恒为非脆性。
fn entity_states(config: &str) -> Vec<EntityStateView> {
    const LAGS: [u32; 8] = [0, 80, 200, 150, 100, 120, 60, 90];
    let normal = |i: usize| EntityStateView {
        coordinates: 0.3,
        brittle_threshold: 0.9,
        decay_coefficient: 0.6,
        time_lag_us: LAGS[i],
    };
    let mut v: Vec<EntityStateView> = (0..8).map(normal).collect();
    v[6] = match config {
        // 脆性·低阈值：raw=0.68 ≥ 0.40 → 突破，置信度透传 1.0
        "brittle_easy" => EntityStateView {
            coordinates: 0.9,
            brittle_threshold: 0.40,
            decay_coefficient: 0.5,
            time_lag_us: LAGS[6],
        },
        // 脆性·高阈值：raw=0.68 < 0.70 → 未突破，快速衰减 0.68×0.5²=0.17 < θ=0.20 → 剪枝
        "brittle_hard" => EntityStateView {
            coordinates: 0.9,
            brittle_threshold: 0.70,
            decay_coefficient: 0.5,
            time_lag_us: LAGS[6],
        },
        // 非脆性对照：线性衰减 0.68×0.6=0.408
        _ => normal(6),
    };
    v
}

fn config_desc(c: &str) -> &'static str {
    match c {
        "brittle_easy" => "脆性 · 低阈值 0.40（易突破）",
        "brittle_hard" => "脆性 · 高阈值 0.70（难突破）",
        _ => "非脆性（线性衰减 0.6）",
    }
}

fn run(label: &str, config: &str, matrix: &CascadeMatrix) {
    let states = entity_states(config);
    let results = cascade(&initial_shock(), matrix, &states, 5, 0.20);
    let mut r = results.clone();
    r.sort_by_key(|x| (x.hop, x.entity_id));

    println!("\n——— 方案 {label}：entity#6 画像 = {}", config_desc(config));
    println!("  命中 {}/8 实体：", r.len());
    for x in &r {
        let breach = if x.confidence >= 0.999 {
            "  ★ 脆性突破 → 置信度透传 1.0"
        } else {
            ""
        };
        println!(
            "   hop{}  #{:<2}{:<16}  conf={:.3}  lag={}μs{}",
            x.hop,
            x.entity_id,
            NAMES[x.entity_id as usize],
            x.confidence,
            x.lag_us,
            breach,
        );
    }
    let hit7 = results.iter().any(|x| x.entity_id == 7);
    println!(
        "  → 终端养老金(#7) 被波及：{}",
        if hit7 { "是 — 冲击穿透至社会终端" } else { "否 — 冲击在 #6 被门控切断" },
    );
}

fn main() {
    let matrix = build_matrix();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  Σ⁴-Engine 系统级联风险压力测试（虚构场景）              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("冲击源 ：北极星宏观基金(#0) 遭遇 $50B 同步赎回，首发满强度信号");
    println!("拓扑   ：8 实体 / 7 条传导边 / 最多 5 跳 / 置信度剪枝阈值 θ = 0.20");
    println!("对照   ：同一拓扑 + 同一冲击，仅改变 #6（暗物质杠杆家办）的脆性画像");

    run("A", "brittle_easy", &matrix);
    run("B", "non_brittle", &matrix);
    run("C", "brittle_hard", &matrix);

    println!("\n┌─ 解读 ──────────────────────────────────────────────────");
    println!("│ 方案A：#6 是脆性且阈值低 → 收到 raw=0.68 即突破，conf=1.0，");
    println!("│        冲击穿透至终端养老金(#7)。#6 被标记为系统级关键节点。");
    println!("│ 方案B：#6 非脆性 → 同样 raw 下 conf 仅 0.408，仍传导但烈度低。");
    println!("│ 方案C：#6 脆性但阈值高 → 未突破，快速衰减至 0.17 < θ 被剪枝，");
    println!("│        #7 完全不被波及。脆性节点在此充当「安全阀 / 熔断」。");
    println!("│ → 同一拓扑三种风险判定：这是 μs 级「如果…会怎样」推演的价值。");
    println!("└─────────────────────────────────────────────────────────");
}
