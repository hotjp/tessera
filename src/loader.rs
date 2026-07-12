//! CSV 基线数据加载器（SPEC 数据基座）。
//!
//! 解析 `docs/global_capital_players_full_index.csv`（183 实体 × 7 字段），
//! 按分类 A-E 用**确定性先验权重表**推导各切面（权力拓扑 / 动态模式 / 认知可达 / 级联响应）
//! 的初始单纯形坐标（**非 LLM**，基于可审计公式，可回测）。中国主体独立标记 `ownership_type`。
//!
//! 数据集无引号 / 无内嵌 ASCII 逗号（字段用中文标点），故手写 `splitn(7, ',')` 解析，
//! 零外部依赖（符合系统自包含目标）。

use crate::constants::{K_MAX, MAX_SLICES};
use crate::entity::{Entity, SteadyState};
use std::path::Path;

/// 每切面有效端点数（K_MAX=8 中前 4 个有效，后 4 个为 padding）。
const FACET_ENDPOINTS: usize = 4;
/// 切面数：权力拓扑 / 动态模式 / 认知可达 / 级联响应。
pub const NUM_FACETS: u8 = 4;

/// 分类字段（如 `"A. 宏观对冲基金/多策略平台"`）→ 码 0..5（A..E）。
fn category_code(cat_field: &str) -> Option<u8> {
    let c = cat_field.trim().chars().next()?;
    Some(match c {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        'D' => 3,
        'E' => 4,
        _ => return None,
    })
}

/// 确定性先验：分类 `cat`(0..5) × 切面 `facet`(0..4) → 4 端点单纯形权重（和 = 1）。
///
/// **非数据拟合、非 LLM**：基于 `(cat, facet, endpoint)` 的可审计公式生成正权重后归一化。
/// 不同分类 / 切面产出不同分布，便于回测与审计。
fn facet_prior(cat: u8, facet: usize) -> [f32; FACET_ENDPOINTS] {
    let mut w = [0.0f32; FACET_ENDPOINTS];
    let base = cat as usize + facet;
    for (j, w_j) in w.iter_mut().enumerate() {
        *w_j = 1.0 + 0.1 * (((base + j) % 7) as f32);
    }
    let sum: f32 = w.iter().sum();
    for w_j in w.iter_mut() {
        *w_j /= sum;
    }
    w
}

/// 是否中国主体（组织名含中国主权 / 央企标记）。
fn is_chinese_entity(org: &str) -> bool {
    const MARKERS: &[&str] = &[
        "中国",
        "中投",
        "中信",
        "招商局",
        "中金",
        "央企",
        "社保基金",
        "国新",
        "兵器",
        "航天",
        "航空",
        "五矿",
        "中船",
        "中核",
        "中石油",
        "中石化",
        "国家电网",
        "敦和",
        "高瓴",
        "红杉中国",
    ];
    MARKERS.iter().any(|m| org.contains(m))
}

/// 实体池：实体 + 名字。名字由池持有，`Entity.name_ptr` 指向其中（非拥有指针）。
///
/// **不变量**：构造后不得再 `push` 到 `names`/`entities`（否则 `name_ptr` 失效）。
pub struct EntityPool {
    /// 实体列表（按加载顺序，id = 行号）。
    pub entities: Vec<Entity>,
    /// 实体显示名（与 entities 一一对应；name_ptr 指向此处）。
    pub names: Vec<String>,
}

/// 从 CSV 文本加载实体池。
pub fn load_from_text(csv: &str) -> EntityPool {
    let csv = csv.strip_prefix('\u{feff}').unwrap_or(csv); // 去 BOM
    let mut entities = Vec::new();
    let mut names = Vec::new();

    for (i, line) in csv.lines().enumerate() {
        if i == 0 {
            continue; // 跳过表头
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(7, ',').collect();
        if parts.len() < 7 {
            continue;
        }
        let org = parts[0].trim().to_string();
        let cat = category_code(parts[6].trim()).unwrap_or(4); // 未知 → E

        let mut e = Entity::new(i as u32, cat, NUM_FACETS);
        let mut dims = [0u8; MAX_SLICES];
        for (f, (row, dim)) in e
            .coordinates
            .iter_mut()
            .zip(dims.iter_mut())
            .enumerate()
            .take(NUM_FACETS as usize)
        {
            let w = facet_prior(cat, f);
            row[..FACET_ENDPOINTS].copy_from_slice(&w);
            row[FACET_ENDPOINTS..K_MAX].fill(0.0); // padding 显式置 0
            *dim = FACET_ENDPOINTS as u8;
        }
        e.slice_dims = dims;
        e.steady_state = SteadyState::new(0, 0, if is_chinese_entity(&org) { 1 } else { 0 });

        entities.push(e);
        names.push(org);
    }

    // 设置 name_ptr 指向池持有的名字字节（非拥有指针，见结构体不变量）
    for (e, name) in entities.iter_mut().zip(names.iter()) {
        e.name_ptr = name.as_ptr();
        e.name_len = name.len() as u16;
    }

    EntityPool { entities, names }
}

/// 从文件路径加载（跨平台 [`Path`]，ADR-002）。
pub fn load_from_file(path: &Path) -> std::io::Result<EntityPool> {
    let csv = std::fs::read_to_string(path)?;
    Ok(load_from_text(&csv))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const CSV_PATH: &str = "docs/global_capital_players_full_index.csv";

    fn load() -> EntityPool {
        load_from_file(&PathBuf::from(CSV_PATH)).expect("无法读取基线 CSV")
    }

    #[test]
    fn loads_exactly_183_entities() {
        let pool = load();
        assert_eq!(pool.entities.len(), 183);
        assert_eq!(pool.names.len(), 183);
    }

    #[test]
    fn every_facet_sums_to_one() {
        let pool = load();
        for e in &pool.entities {
            let n = e.num_slices as usize;
            for f in 0..n {
                let k = e.slice_dims[f] as usize;
                let sum: f32 = e.coordinates[f][..k].iter().sum();
                assert!(
                    (sum - 1.0).abs() < 1e-5,
                    "实体 {} 切面 {} 行和 {} != 1",
                    e.id,
                    f,
                    sum
                );
            }
        }
    }

    #[test]
    fn padding_columns_are_zero() {
        let pool = load();
        for e in &pool.entities {
            let n = e.num_slices as usize;
            for f in 0..n {
                for p in FACET_ENDPOINTS..K_MAX {
                    assert_eq!(e.coordinates[f][p], 0.0, "padding [{f}][{p}] != 0");
                }
            }
        }
    }

    #[test]
    fn all_weights_non_negative() {
        let pool = load();
        for e in &pool.entities {
            for row in &e.coordinates {
                for &x in row {
                    assert!(x >= 0.0, "负权重 {}", x);
                }
            }
        }
    }

    #[test]
    fn categories_a_to_e_all_present() {
        let pool = load();
        let mut seen = [false; 5]; // A..E
        for e in &pool.entities {
            assert!(e.entity_type < 5, "非法分类码 {}", e.entity_type);
            seen[e.entity_type as usize] = true;
        }
        for (i, s) in seen.iter().enumerate() {
            assert!(s, "分类 {} 未出现", char::from(b'A' + i as u8));
        }
    }

    #[test]
    fn chinese_entities_marked() {
        let pool = load();
        let cn = pool
            .entities
            .iter()
            .filter(|e| e.steady_state.ownership_type == 1)
            .count();
        assert!(cn > 0, "应检测到中国主体（ownership_type=1）");
        // 中国主体名字应含标记词
        for (e, name) in pool.entities.iter().zip(pool.names.iter()) {
            if e.steady_state.ownership_type == 1 {
                assert!(is_chinese_entity(name), "误标中国主体: {name}");
            }
        }
    }

    #[test]
    fn distinct_categories_have_distinct_profiles() {
        // 确定性先验应随分类变化（非全相同）
        let a = facet_prior(0, 0);
        let b = facet_prior(3, 0);
        assert!(a != b, "A 与 D 分类同切面应有不同先验");
    }

    #[test]
    fn name_ptr_points_into_pool_names() {
        let pool = load();
        // 第一个实体的 name_ptr 应指向 names[0] 的字节
        let name_bytes = pool.names[0].as_bytes();
        assert!(!name_bytes.is_empty());
        assert_eq!(pool.entities[0].name_len as usize, name_bytes.len());
        // 读取 name_ptr 指向的字节验证一致（unsafe：pool 持有 names，指针有效）
        unsafe {
            let ptr = pool.entities[0].name_ptr;
            let got = std::slice::from_raw_parts(ptr, pool.entities[0].name_len as usize);
            assert_eq!(got, name_bytes);
        }
    }
}
