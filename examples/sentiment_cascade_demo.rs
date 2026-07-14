//! Σ⁴-Engine 舆情研判示例（虚构）：负面爆料在 KOL/社区网络的级联传导。
//!
//! 对标 `systemic_risk_demo.rs`（金融版）—— 同构的「同一拓扑 + 同一冲击，
//! 仅改变关键节点画像 → 三种判定」对比，但领域换成舆情研判，展示引擎的领域无关性。
//!
//! 场景：某上市消费品牌被匿名爆料产品质量问题 → 爆料首发 → 主流媒体跟进 →
//! 极化消费者社群(#6)是否被「点燃」决定品牌声誉终端(#7)是否受创。
//!
//! 核心演示（与金融版同构）：脆性社群充当**引爆门控**——
//! 未达点燃阈值时快速衰减（甚至剪枝、切断向大众的传导），突破后置信度透传 1.0
//! （被标记为舆情风暴的引爆节点）。

use sigma4_engine::cascade::{cascade, EntityStateView};
use sigma4_engine::matrix::CascadeMatrix;

/// 8 个虚构舆情主体（原型化命名）。
const NAMES: [&str; 8] = [
    "匿名爆料源",       // 0 — 冲击源：负面质量爆料首发
    "头部吃瓜大V",       // 1 — 转发放大（广传播）
    "行业垂直自媒体",    // 2 — 深度跟进
    "主流财经媒体",      // 3 — 强传导、高公信力放大
    "热搜话题榜",        // 4 — 议程聚合
    "泛大众营销号",      // 5 — 蹭流量跟风
    "极化消费者社群",    // 6 — 高立场极化（脆性节点）
    "品牌声誉/大众舆论", // 7 — 终端：品牌形象与社会口碑
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
        flow_edge(0, 1, 0.55, 80),  // 爆料 → 头部大V
        flow_edge(0, 2, 0.65, 200), // 爆料 → 垂直自媒体
        flow_edge(0, 3, 0.80, 150), // 爆料 → 主流媒体（强传导）
        flow_edge(1, 4, 0.65, 100), // 大V → 热搜
        flow_edge(2, 5, 0.55, 120), // 自媒体 → 营销号
        flow_edge(3, 6, 0.85, 60),  // 主流媒体 → 极化社群（强点燃）
        flow_edge(6, 7, 0.75, 90),  // 社群引爆 → 品牌声誉终端
    ];
    CascadeMatrix::from_edges(8, &flow)
}

fn initial_shock() -> Vec<f32> {
    let mut s = vec![0.0f32; 8];
    s[0] = 1.0; // 匿名爆料首发满强度
    s
}

/// 构造实体状态视图；entity #6 的画像随 `config` 变化，其余实体恒为非脆性。
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
        // 极化·低阈值：raw=0.68 ≥ 0.40 → 点燃，置信度透传 1.0
        "brittle_easy" => EntityStateView {
            coordinates: 0.9,
            brittle_threshold: 0.40,
            decay_coefficient: 0.5,
            time_lag_us: LAGS[6],
        },
        // 极化·高阈值：raw=0.68 < 0.70 → 未点燃，快速衰减 0.68×0.5²=0.17 < θ=0.20 → 剪枝
        "brittle_hard" => EntityStateView {
            coordinates: 0.9,
            brittle_threshold: 0.70,
            decay_coefficient: 0.5,
            time_lag_us: LAGS[6],
        },
        // 普通社群（非极化）：线性衰减 0.68×0.6=0.408
        _ => normal(6),
    };
    v
}

fn config_desc(c: &str) -> &'static str {
    match c {
        "brittle_easy" => "极化 · 低阈值 0.40（易点燃）",
        "brittle_hard" => "极化 · 高阈值 0.70（难点燃）",
        _ => "普通社群（线性衰减 0.6）",
    }
}

fn run(label: &str, config: &str, matrix: &CascadeMatrix) {
    let states = entity_states(config);
    let results = cascade(&initial_shock(), matrix, &states, 5, 0.20);
    let mut r = results.clone();
    r.sort_by_key(|x| (x.hop, x.entity_id));

    println!("\n——— 方案 {label}：#6 画像 = {}", config_desc(config));
    println!("  命中 {}/8 节点：", r.len());
    for x in &r {
        let breach = if x.confidence >= 0.999 {
            "  ★ 脆性点燃 → 置信度透传 1.0"
        } else {
            ""
        };
        println!(
            "   hop{}  #{:<2}{:<18}  conf={:.3}  lag={}μs{}",
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
        "  → 品牌声誉终端(#7) 受创：{}",
        if hit7 { "是 — 舆情穿透至大众口碑" } else { "否 — 传导在 #6 被门控切断" },
    );
}

fn main() {
    let matrix = build_matrix();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  Σ⁴-Engine 舆情研判级联压力测试（虚构场景）             ║");
    println!("║  对标 systemic_risk_demo.rs（金融版）—— 同构、领域不同  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("冲击源 ：匿名爆料源(#0) 发布品牌质量负面帖，首发满强度信号");
    println!("拓扑   ：8 节点 / 7 条传导边 / 最多 5 跳 / 置信度剪枝阈值 θ = 0.20");
    println!("对照   ：同一拓扑 + 同一冲击，仅改变 #6（极化消费者社群）的画像");

    run("A", "brittle_easy", &matrix);
    run("B", "normal", &matrix);
    run("C", "brittle_hard", &matrix);

    println!("\n┌─ 解读 ──────────────────────────────────────────────────");
    println!("│ 方案A：#6 极化且低阈值 → 收到 raw=0.68 即点燃，conf=1.0，");
    println!("│        舆情穿透至品牌声誉终端(#7)。#6 被标记为风暴引爆节点。");
    println!("│ 方案B：#6 普通社群 → 同样 raw 下 conf 仅 0.408，仍传导但烈度低。");
    println!("│ 方案C：#6 极化但高阈值 → 未点燃，快速衰减至 0.17 < θ 被剪枝，");
    println!("│        #7 完全不受创。极化社群在此充当「未引爆的潜在雷管」。");
    println!("│ → 同一爆料三种舆情判定：这正是 μs 级「如果…会怎样」推演在");
    println!("│   舆情研判中的价值——识别哪个社群是引爆点、哪里该投入辟谣。");
    println!("└─────────────────────────────────────────────────────────");
    println!("\n注：本 demo 用生产引擎 cascade()，与金融版 systemic_risk_demo.rs");
    println!("    共用同一推理内核；两域差异仅在实体/边/参数的数据层解释。");
}
