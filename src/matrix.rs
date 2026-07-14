//! CSR 稀疏级联矩阵与标量 SpMV（SPEC §6.2 scalar）。
//!
//! 本标量实现是 SIMD 路径（task_008，nightly `std::simd`，ADR-001）的
//! **真值参照**与最终 fallback。

use std::simd::prelude::SimdFloat;
use std::simd::Simd;

/// CSR 格式的稀疏级联矩阵（n×n）。
#[repr(C)]
pub struct CascadeMatrix {
    /// 方阵维度 n。
    pub n: u32,
    /// 行指针（长度 n+1）：`row_ptr[i]..row_ptr[i+1]` 为第 i 行非零元在
    /// `col_idx` / `values` / `time_lag_us` 中的下标区间。
    pub row_ptr: Vec<u32>,
    /// 列索引（与 values 一一对应）。
    pub col_idx: Vec<u32>,
    /// 非零权重值。
    pub values: Vec<f32>,
    /// 每条边的传播时滞（微秒），与 values 一一对应。
    pub time_lag_us: Vec<u32>,
}

impl CascadeMatrix {
    /// 从边列表 `(from, to, weight, time_lag_us)` 构建 CSR（行 = from）。
    ///
    /// **Panics**：若 `from >= n` 或 `to >= n`，构造期 panic（构造期 fail-fast 优于运行时越界）。
    pub fn from_edges(n: u32, edges: &[(u32, u32, f32, u32)]) -> Self {
        let n_us = n as usize;
        // 1. 计数每行非零元
        let mut row_ptr = vec![0u32; n_us + 1];
        for &(from, to, _, _) in edges {
            assert!(from < n, "from_edges: edge from={} out of range for n={}", from, n);
            assert!(to < n, "from_edges: edge to={} out of range for n={}", to, n);
            row_ptr[from as usize + 1] += 1;
        }
        // 2. 前缀和 → row_ptr
        for i in 0..n_us {
            row_ptr[i + 1] += row_ptr[i];
        }
        // 3. 填充 col_idx / values / time_lag_us
        let nnz = row_ptr[n_us] as usize;
        let mut col_idx = vec![0u32; nnz];
        let mut values = vec![0.0f32; nnz];
        let mut time_lag_us = vec![0u32; nnz];
        let mut cursor = row_ptr.clone();
        for &(from, to, w, lag) in edges {
            let f = from as usize;
            if f < n_us {
                let pos = cursor[f] as usize;
                col_idx[pos] = to;
                values[pos] = w;
                time_lag_us[pos] = lag;
                cursor[f] += 1;
            }
        }
        CascadeMatrix {
            n,
            row_ptr,
            col_idx,
            values,
            time_lag_us,
        }
    }
}

/// 标量 CSR SpMV：`y[i] = Σ_k values[k] * x[col_idx[k]]`，
/// `k ∈ row_ptr[i]..row_ptr[i+1]`。
///
/// SIMD 路径（task_008）的真值参照与 fallback（ADR-001）。
pub fn spmv_csr_scalar(x: &[f32], matrix: &CascadeMatrix, y: &mut [f32]) {
    // 逐行点积：y[i] = Σ values[k]·x[col_idx[k]], k ∈ row_ptr[i]..row_ptr[i+1]
    for (y_i, window) in y.iter_mut().zip(matrix.row_ptr.windows(2)) {
        let start = window[0] as usize;
        let end = window[1] as usize;
        let mut acc = 0.0f32;
        for (&col, &v) in matrix.col_idx[start..end]
            .iter()
            .zip(matrix.values[start..end].iter())
        {
            acc += v * x[col as usize];
        }
        *y_i = acc;
    }
}

/// SIMD CSR SpMV（nightly `std::simd` 可移植 SIMD，ADR-001）。
///
/// LANES=8：`values` 连续装入、`x` 按 `col_idx` 聚集（先入缓冲）、向量乘 + 水平归约；
/// 不足一块的尾部用标量收尾。`std::simd` 在各平台自动降级
/// （aarch64 → NEON/SVE，x86_64 → AVX2/AVX-512），**无需运行时硬件检测**
/// （ADR-001 取代 SPEC §6.2 的 `is_x86_feature_detected!` 分发）。
/// 标量 fallback / 真值参照见 [`spmv_csr_scalar`]。
pub fn spmv_csr(x: &[f32], matrix: &CascadeMatrix, y: &mut [f32]) {
    const LANES: usize = 8;
    for (y_i, window) in y.iter_mut().zip(matrix.row_ptr.windows(2)) {
        let start = window[0] as usize;
        let end = window[1] as usize;
        let mut acc = 0.0f32;
        let mut k = start;
        while k + LANES <= end {
            let xv: Simd<f32, LANES> =
                Simd::from_array(core::array::from_fn(|j| x[matrix.col_idx[k + j] as usize]));
            let vals: Simd<f32, LANES> = Simd::from_slice(&matrix.values[k..k + LANES]);
            acc += (vals * xv).reduce_sum();
            k += LANES;
        }
        // 标量尾部（不足一块）
        while k < end {
            acc += matrix.values[k] * x[matrix.col_idx[k] as usize];
            k += 1;
        }
        *y_i = acc;
    }
}

#[cfg(test)]
mod spmv {
    use super::*;

    /// 稠密参考实现（逐元素对比用）。
    fn dense_spmv(n: usize, edges: &[(u32, u32, f32, u32)], x: &[f32]) -> Vec<f32> {
        let mut d = vec![vec![0.0f32; n]; n];
        for &(f, t, w, _) in edges {
            d[f as usize][t as usize] = w;
        }
        (0..n)
            .map(|i| (0..n).map(|j| d[i][j] * x[j]).sum())
            .collect()
    }

    #[test]
    fn scalar_matches_dense_elementwise() {
        let edges = vec![
            (0, 1, 0.5, 10),
            (0, 2, 0.3, 20),
            (1, 2, 0.8, 5),
            (2, 0, 0.1, 0),
        ];
        let m = CascadeMatrix::from_edges(3, &edges);
        let x = vec![1.0f32, 2.0, 3.0];
        let mut y = vec![0.0f32; 3];
        spmv_csr_scalar(&x, &m, &mut y);
        let expected = dense_spmv(3, &edges, &x);
        // 预期 [1.9, 2.4, 0.1]
        for i in 0..3 {
            assert!(
                (y[i] - expected[i]).abs() < 1e-5,
                "[{i}] {} vs {}",
                y[i],
                expected[i]
            );
        }
    }

    #[test]
    fn empty_row_yields_zero() {
        // 行 0 无非零元 → y[0]=0
        let edges = vec![(1, 2, 0.5, 0)];
        let m = CascadeMatrix::from_edges(3, &edges);
        let x = vec![1.0f32, 1.0, 1.0];
        let mut y = vec![0.0f32; 3];
        spmv_csr_scalar(&x, &m, &mut y);
        assert_eq!(y[0], 0.0, "空行应为 0");
        assert!((y[1] - 0.5).abs() < 1e-6);
        assert_eq!(y[2], 0.0);
    }

    #[test]
    fn single_element_row_correct() {
        let edges = vec![(0, 1, 0.7, 0)];
        let m = CascadeMatrix::from_edges(2, &edges);
        let x = vec![0.0f32, 4.0];
        let mut y = vec![0.0f32; 2];
        spmv_csr_scalar(&x, &m, &mut y);
        assert!((y[0] - 2.8).abs() < 1e-6, "0.7*4.0 = 2.8, got {}", y[0]); // 0.7 * 4.0
        assert_eq!(y[1], 0.0);
    }

    #[test]
    fn valid_edges_all_accepted() {
        // 两条合法边都被保留（非法边已在 from_edges 构造期被拦截）
        let edges = vec![(0, 1, 0.5, 0), (1, 0, 0.25, 0)];
        let m = CascadeMatrix::from_edges(2, &edges);
        assert_eq!(m.col_idx.len(), 2); // 两条合法边都保留
        let x = vec![1.0f32, 2.0];
        let mut y = vec![0.0f32; 2];
        spmv_csr_scalar(&x, &m, &mut y);
        assert!((y[0] - 1.0).abs() < 1e-6); // 0.5*2.0
        assert!((y[1] - 0.25).abs() < 1e-6); // 0.25*1.0
    }

    #[test]
    fn matrix_fields_consistent() {
        let edges = vec![(0, 1, 0.5, 10), (1, 0, 0.25, 20)];
        let m = CascadeMatrix::from_edges(2, &edges);
        assert_eq!(m.n, 2);
        assert_eq!(m.row_ptr, vec![0, 1, 2]);
        assert_eq!(m.col_idx, vec![1, 0]);
        assert_eq!(m.values, vec![0.5, 0.25]);
        assert_eq!(m.time_lag_us, vec![10, 20]);
    }

    #[test]
    fn simd_matches_scalar_all_paths() {
        // row0: 17 nz（2 块 + 1 尾）；row1: 8 nz（1 块无尾）；row2: 3 nz（仅尾）
        let n = 18u32;
        let mut edges = Vec::new();
        for j in 0..17u32 {
            edges.push((0, j, (j as f32) * 0.05 + 0.01, j));
        }
        for j in 0..8u32 {
            edges.push((1, j, (j as f32) * 0.1 + 0.02, j));
        }
        for j in 0..3u32 {
            edges.push((2, j, (j as f32) * 0.2 + 0.03, j));
        }
        let m = CascadeMatrix::from_edges(n, &edges);
        let x: Vec<f32> = (0..n).map(|i| (i as f32) * 0.3).collect();
        let mut ys = vec![0.0f32; n as usize];
        let mut yv = vec![0.0f32; n as usize];
        spmv_csr_scalar(&x, &m, &mut ys);
        spmv_csr(&x, &m, &mut yv);
        for (i, (&s, &v)) in ys.iter().zip(yv.iter()).enumerate() {
            assert!((v - s).abs() < 1e-5, "[{i}] simd {v} vs scalar {s}");
        }
    }
}
