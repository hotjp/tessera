//! 单纯形投影与 Frobenius 距离（SPEC §2.1 / §2.4）。
//!
//! - [`project_onto_simplex`]：Duchi et al. 2008 的 O(k log k) 欧氏投影到概率单纯形。
//! - [`frobenius_distance`]：仅对每切面前 k 项计算差平方和，padding 列显式跳过。

use crate::constants::{K_MAX, MAX_SLICES};

/// 将 `v` 的前 `k` 维投影到标准概率单纯形（`Σ=1, xᵢ≥0`）。
///
/// - 输入可为任意实数（含负数、>1），**不假设已归一化**。
/// - 输出保持 `k` 维；退化（投影为 0）的维度**不移除**，仅置 0。
/// - 复杂度 O(k log k)，由排序主导。
///
/// # NaN
/// 排序使用 `partial_cmp().unwrap_or(Equal)`，NaN 视为相等——**调用方须保证输入无 NaN**，
/// 否则结果未定义（SPEC 假设坐标为有限值）。
/// Duchi 投影核心：对 `buf[..k]` 原地投影，使用 `scratch` 作排序缓冲（长度 ≥ k）。
///
/// 先把 `buf[..k]` 复制进 `scratch` 再排序，故 `buf` 既作输入又作输出（原地安全）。
fn duchi_inplace(buf: &mut [f32], k: usize, scratch: &mut [f32]) {
    let k = k.min(buf.len()).min(scratch.len());
    if k == 0 {
        return;
    }
    scratch[..k].copy_from_slice(&buf[..k]);
    scratch[..k].sort_by(|a, b| b.partial_cmp(a).unwrap_or(core::cmp::Ordering::Equal));

    // 找 rho：最后一个满足 u[j] - (cumsum-1)/(j+1) > 0 的下标；theta 为对应阈值
    let mut cumsum = 0.0f32;
    let mut theta = 0.0f32;
    for (j, &u_j) in scratch[..k].iter().enumerate() {
        cumsum += u_j;
        let t = (cumsum - 1.0) / (j as f32 + 1.0);
        if u_j - t > 0.0 {
            theta = t; // 持续覆盖，最终为 rho 处的阈值
        }
    }
    for b in buf[..k].iter_mut() {
        *b = (*b - theta).max(0.0);
    }
}

/// 将 `v` 的前 `k` 维投影到标准概率单纯形（`Σ=1, xᵢ≥0`）。
///
/// - 输入可为任意实数（含负数、>1），**不假设已归一化**。
/// - 输出保持 `k` 维；退化（投影为 0）的维度**不移除**，仅置 0。
/// - 复杂度 O(k log k)，由排序主导。
///
/// # NaN
/// 排序使用 `partial_cmp().unwrap_or(Equal)`，NaN 视为相等——**调用方须保证输入无 NaN**，
/// 否则结果未定义（SPEC 假设坐标为有限值）。
pub fn project_onto_simplex(v: &[f32], k: usize) -> Vec<f32> {
    let k = k.min(v.len());
    let mut out: Vec<f32> = v[..k].to_vec();
    let mut scratch: Vec<f32> = vec![0.0; k];
    duchi_inplace(&mut out, k, &mut scratch);
    out
}

/// 原地投影 `buf[..k]`，**无堆分配**（栈 scratch `[f32; K_MAX]`）。
///
/// 供热路径查询（[`crate::entity::Entity::query_state`]）使用：坐标行 `k ≤ K_MAX`。
/// 若 `k > K_MAX`，截断到 `K_MAX`（debug 构建断言）。
pub fn project_onto_simplex_inplace(buf: &mut [f32], k: usize) {
    debug_assert!(k <= K_MAX, "project_onto_simplex_inplace: k ({k}) > K_MAX");
    let mut scratch = [0.0f32; K_MAX];
    duchi_inplace(buf, k.min(K_MAX), &mut scratch);
}

/// 计算两个坐标矩阵的 Frobenius 距离，仅统计每切面前 `slice_dims[s]` 项。
///
/// - `a` / `b`：`[切面][端点]` 坐标矩阵（`K_MAX` 列）。
/// - `slice_dims`：每切面有效维度数；列 `i >= slice_dims[s]`（padding）**显式跳过**，
///   故 padding 取值不同不影响距离（SPEC §2.4）。
pub fn frobenius_distance(
    a: &[[f32; K_MAX]; MAX_SLICES],
    b: &[[f32; K_MAX]; MAX_SLICES],
    slice_dims: &[u8],
) -> f32 {
    let mut sum_sq = 0.0f32;
    for (s, &dim) in slice_dims.iter().enumerate() {
        if s >= MAX_SLICES {
            break;
        }
        let k = (dim as usize).min(K_MAX);
        for i in 0..k {
            let d = a[s][i] - b[s][i];
            sum_sq += d * d;
        }
        // padding 列（i >= k）显式跳过
    }
    sum_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn simplex_uniform_distribution() {
        let v = vec![1.0f32, 1.0, 1.0];
        let p = project_onto_simplex(&v, 3);
        assert_eq!(p.len(), 3);
        let sum: f32 = p.iter().sum();
        assert!(approx_eq(sum, 1.0, 1e-5), "sum={sum}");
        for &x in &p {
            assert!(x >= 0.0);
            assert!(approx_eq(x, 1.0 / 3.0, 1e-5), "x={x}");
        }
    }

    #[test]
    fn simplex_accepts_arbitrary_reals_including_negatives() {
        // 含负数 / >1，投影后仍合法
        let v = vec![-1.0f32, 5.0, -3.0, 2.0];
        let p = project_onto_simplex(&v, 4);
        assert_eq!(p.len(), 4);
        let sum: f32 = p.iter().sum();
        assert!(approx_eq(sum, 1.0, 1e-5), "sum={sum}");
        for &x in &p {
            assert!(x >= 0.0, "negative component {x}");
        }
    }

    #[test]
    fn simplex_values_above_one() {
        let v = vec![3.0f32, 3.0];
        let p = project_onto_simplex(&v, 2);
        let sum: f32 = p.iter().sum();
        assert!(approx_eq(sum, 1.0, 1e-5));
        for &x in &p {
            assert!(x >= 0.0);
        }
    }

    #[test]
    fn simplex_already_normalized_is_fixed_point() {
        let v = vec![0.25f32, 0.25, 0.25, 0.25];
        let p = project_onto_simplex(&v, 4);
        for &x in &p {
            assert!(approx_eq(x, 0.25, 1e-5), "x={x}");
        }
    }

    #[test]
    fn simplex_degenerate_keeps_dims() {
        // 退化：投影集中到一个维度，其余为 0 但维度保留
        let v = vec![5.0f32, 0.0, 0.0];
        let p = project_onto_simplex(&v, 3);
        assert_eq!(p.len(), 3); // 保持 k 维，不移除
        let sum: f32 = p.iter().sum();
        assert!(approx_eq(sum, 1.0, 1e-5));
        assert!(approx_eq(p[0], 1.0, 1e-5));
        assert!(p[1].abs() < 1e-6);
        assert!(p[2].abs() < 1e-6);
    }

    #[test]
    fn simplex_uses_first_k_of_longer_input() {
        // v 长度 > k：仅投影前 k
        let v = vec![1.0f32, 1.0, 1.0, 99.0, 99.0];
        let p = project_onto_simplex(&v, 3);
        assert_eq!(p.len(), 3);
        let sum: f32 = p.iter().sum();
        assert!(approx_eq(sum, 1.0, 1e-5));
    }

    #[test]
    fn frobenius_identical_is_zero() {
        let mut a = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut b = [[0.0f32; K_MAX]; MAX_SLICES];
        for s in 0..MAX_SLICES {
            for i in 0..K_MAX {
                a[s][i] = 0.3;
                b[s][i] = 0.3;
            }
        }
        let dims = [K_MAX as u8; MAX_SLICES];
        assert!(frobenius_distance(&a, &b, &dims) < 1e-6);
    }

    #[test]
    fn frobenius_padding_difference_ignored() {
        // 前 k 列相同，padding 列(i>=k)取不同值 → 距离仍 ~0
        let k = 3usize;
        let mut a = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut b = [[0.0f32; K_MAX]; MAX_SLICES];
        for s in 0..MAX_SLICES {
            for i in 0..k {
                a[s][i] = 0.2;
                b[s][i] = 0.2;
            }
            for i in k..K_MAX {
                a[s][i] = 0.99; // padding 垃圾
                b[s][i] = -0.5; // 不同垃圾
            }
        }
        let dims = [k as u8; MAX_SLICES];
        let d = frobenius_distance(&a, &b, &dims);
        assert!(d < 1e-6, "padding 差异未被忽略: d={d}");
    }

    #[test]
    fn frobenius_known_single_point_difference() {
        let mut a = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut b = [[0.0f32; K_MAX]; MAX_SLICES];
        a[0][0] = 1.0;
        b[0][0] = 0.0;
        let dims = [K_MAX as u8; MAX_SLICES];
        let d = frobenius_distance(&a, &b, &dims);
        assert!(approx_eq(d, 1.0, 1e-5), "d={d}");
    }
}
