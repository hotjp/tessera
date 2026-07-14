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
    // F2 fix: use f64 for cumsum and theta to avoid catastrophic cancellation with large values
    let mut cumsum: f64 = 0.0;
    let mut theta = 0.0f64;
    for (j, &u_j) in scratch[..k].iter().enumerate() {
        cumsum += u_j as f64;
        let t = (cumsum - 1.0) / (j as f64 + 1.0);
        if (u_j as f64) - t > 0.0 {
            theta = t; // 持续覆盖，最终为 rho 处的阈值
        }
    }
    for b in buf[..k].iter_mut() {
        *b = ((*b as f64 - theta).max(0.0)) as f32;
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
    // F1 fix: sanitize non-finite values at entry
    let sanitized: Vec<f32> = v[..k].iter().map(|&x| if x.is_finite() { x } else { 0.0 }).collect();

    let mut out: Vec<f32> = sanitized.to_vec();
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
    // F1 fix: sanitize non-finite values in-place at entry
    let k_actual = k.min(buf.len()).min(K_MAX);
    for x in buf[..k_actual].iter_mut() {
        if !x.is_finite() {
            *x = 0.0;
        }
    }
    let mut scratch = [0.0f32; K_MAX];
    duchi_inplace(buf, k_actual, &mut scratch);
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

/// 单纯形约束无损编解码器（SPEC §7.1）。
///
/// 利用「Σ前 k 项 = 1」约束，每行只存前 `k-1` 个 f32（`to_le_bytes`），
/// 第 `k` 列在解码时由 `1 - Σ前 k-1` 还原 → 对满足约束的输入 100% 可逆。
pub struct SimplexCodec;

impl SimplexCodec {
    /// 编码坐标矩阵为字节流。每行存前 `k-1` 个 f32（小端）。
    ///
    /// **前置条件**：每行前 `k` 项之和 = 1（容差 1e-5），否则 panic。
    pub fn encode(m: &[[f32; K_MAX]; MAX_SLICES], slice_dims: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for (s, row) in m.iter().enumerate() {
            let k = (slice_dims.get(s).copied().unwrap_or(0) as usize).min(K_MAX);
            if k >= 1 {
                // 仅活跃切面要求 Σ前 k 项 = 1（k=0 空行无约束）
                let sum: f32 = row[..k].iter().sum();
                assert!(
                    (sum - 1.0).abs() < 1e-5,
                    "SimplexCodec::encode: row {s} sum {sum} != 1 (容差 1e-5)"
                );
            }
            // 存前 k-1 个 f32（第 k 列由约束还原）
            for v in row[..k.saturating_sub(1)].iter() {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }

    /// 解码字节流为坐标矩阵。
    ///
    /// 前 `k-1` 列从字节读回；第 `k` 列 = `1 - Σ前 k-1`；padding 列（`i >= k`）显式置 0。
    pub fn decode(data: &[u8], slice_dims: &[u8], k_max: usize) -> [[f32; K_MAX]; MAX_SLICES] {
        let mut out = [[0.0f32; K_MAX]; MAX_SLICES];
        let mut pos = 0usize;
        for (s, row) in out.iter_mut().enumerate() {
            let k = (slice_dims.get(s).copied().unwrap_or(0) as usize)
                .min(k_max)
                .min(K_MAX);
            let n_stored = k.saturating_sub(1);
            let mut sum_prev = 0.0f32;
            for slot in row[..n_stored].iter_mut() {
                let mut b = [0u8; 4];
                b.copy_from_slice(&data[pos..pos + 4]);
                let v = f32::from_le_bytes(b);
                *slot = v;
                sum_prev += v;
                pos += 4;
            }
            // 第 k 列 = 1 - Σ前 k-1
            if k >= 1 {
                row[k - 1] = 1.0 - sum_prev;
            }
            // padding 列（i >= k）显式置 0
            for slot in row[k..K_MAX].iter_mut() {
                *slot = 0.0;
            }
        }
        out
    }
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

#[cfg(test)]
mod codec {
    use super::*;

    /// 构造一行：`first` 为前 k-1 个权重，第 k 列 = 1 - Σ，其余 0。
    fn row(first: &[f32]) -> [f32; K_MAX] {
        let mut r = [0.0f32; K_MAX];
        let mut s = 0.0f32;
        for (i, &v) in first.iter().enumerate() {
            r[i] = v;
            s += v;
        }
        r[first.len()] = 1.0 - s; // 第 k 列
        r
    }

    /// 前 N 项为 `ks`，其余 0。
    fn dims(ks: &[u8]) -> [u8; MAX_SLICES] {
        let mut d = [0u8; MAX_SLICES];
        for (i, &k) in ks.iter().enumerate() {
            d[i] = k;
        }
        d
    }

    #[test]
    fn round_trip_elementwise_within_1e6() {
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = row(&[0.2, 0.3]); // k=3: [0.2,0.3,0.5]
        m[1] = row(&[0.1, 0.1, 0.1]); // k=4
        m[2] = row(&[0.5]); // k=2: [0.5,0.5]
        let dims = dims(&[3, 4, 2]);
        let enc = SimplexCodec::encode(&m, &dims);
        let dec = SimplexCodec::decode(&enc, &dims, K_MAX);
        for (s, ((d_row, m_row), &k)) in dec.iter().zip(m.iter()).zip(dims.iter()).enumerate() {
            for (i, (&d, &e)) in d_row[..k as usize]
                .iter()
                .zip(m_row[..k as usize].iter())
                .enumerate()
            {
                assert!((d - e).abs() < 1e-6, "[{s}][{i}] {d} vs {e}");
            }
        }
    }

    #[test]
    fn decode_padding_columns_are_zero() {
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = row(&[0.25, 0.25]); // k=3
        let dims = dims(&[3]);
        let enc = SimplexCodec::encode(&m, &dims);
        let dec = SimplexCodec::decode(&enc, &dims, K_MAX);
        for &v in dec[0][3..K_MAX].iter() {
            assert_eq!(v, 0.0, "padding column != 0");
        }
    }

    #[test]
    fn degenerate_zero_weight_preserved() {
        // 含 0 权重的退化维度：[0.5, 0.0, 0.5]，k=3，中间 0 不移除
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = row(&[0.5, 0.0]); // [0.5, 0.0, 0.5]
        let dims = dims(&[3]);
        let enc = SimplexCodec::encode(&m, &dims);
        let dec = SimplexCodec::decode(&enc, &dims, K_MAX);
        assert!((dec[0][0] - 0.5).abs() < 1e-6);
        assert!(dec[0][1].abs() < 1e-6, "0 权重未保留: {}", dec[0][1]);
        assert!((dec[0][2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn row_sum_not_one_panics() {
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = [0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]; // sum=1.5
        let dims = dims(&[3]);
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            SimplexCodec::encode(&m, &dims);
        }));
        assert!(res.is_err(), "expected panic on row sum != 1");
    }

    #[test]
    fn encode_size_is_minimal() {
        // 每行仅存 (k-1)*4 字节
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = row(&[0.2, 0.3]); // k=3 → 2 f32 = 8 字节
        let dims = dims(&[3]);
        let enc = SimplexCodec::encode(&m, &dims);
        assert_eq!(enc.len(), 2 * 4);
    }
}
