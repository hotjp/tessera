//! Tessera 数值/数学边界审计测试套件 (A1: Numeric/Math Correctness)
//!
//! 本测试套件验证 Duchi 单纯形投影、帕累托约束和 SpMV 的数值边界场景。
//! 包含 NaN/Inf、空向量、负值、大动态范围、k 越界、SIMD 一致性等场景。

use tessera::{
    constraint::{Constraint, ConstraintKind, pareto_project},
    matrix::{CascadeMatrix, spmv_csr, spmv_csr_scalar},
    simplex::{frobenius_distance, project_onto_simplex, project_onto_simplex_inplace, SimplexCodec},
};

use std::panic::catch_unwind;

/// 测试辅助：浮点近似相等
fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() < tol
}

/// 测试辅助：检查向量是否为合法单纯形（非负且和=1）
fn is_valid_simplex(v: &[f32], tol: f32) -> bool {
    if v.is_empty() {
        return true; // 空向量视为退化但合法
    }
    let sum: f32 = v.iter().sum();
    if !approx_eq(sum, 1.0, tol) {
        return false;
    }
    v.iter().all(|&x| x >= 0.0)
}

// ============================================================================
// 场景 1: NaN/Inf 输入
// ============================================================================

#[test]
fn simplex_nan_input_does_not_panic() {
    // F1 修复后：NaN 输入被 sanitize 为 0，返回有限结果
    let v = vec![f32::NAN, 1.0, 0.5];
    let p = project_onto_simplex(&v, 3);
    // 验证结果所有元素都是有限的
    assert!(p.iter().all(|&x| x.is_finite()), "结果应不含 NaN/Inf");
    // 验证是合法单纯形：非负且和≈1
    let sum: f32 = p.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
    for &x in &p {
        assert!(x >= 0.0, "元素应为非负: {}", x);
    }
}

#[test]
fn simplex_infinity_input_does_not_panic() {
    // F1 修复后：Inf 输入被 sanitize 为 0，返回有限结果
    let v = vec![f32::INFINITY, 1.0, 0.5];
    let p = project_onto_simplex(&v, 3);
    // 验证结果所有元素都是有限的
    assert!(p.iter().all(|&x| x.is_finite()), "结果应不含 NaN/Inf");
    // 验证是合法单纯形：非负且和≈1
    let sum: f32 = p.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
    for &x in &p {
        assert!(x >= 0.0, "元素应为非负: {}", x);
    }
}

#[test]
fn simplex_negative_infinity_does_not_panic() {
    // F1 修复后：-Inf 输入被 sanitize 为 0，返回有限结果
    let v = vec![f32::NEG_INFINITY, 2.0, 3.0];
    let p = project_onto_simplex(&v, 3);
    // 验证结果所有元素都是有限的
    assert!(p.iter().all(|&x| x.is_finite()), "结果应不含 NaN/Inf");
    // 验证是合法单纯形：非负且和≈1
    let sum: f32 = p.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
    for &x in &p {
        assert!(x >= 0.0, "元素应为非负: {}", x);
    }
}

#[test]
fn simplex_mixed_nan_inf_does_not_panic() {
    // F1 修复后：混合 NaN/Inf 输入被 sanitize 为 0，返回有限结果
    let v = vec![f32::NAN, f32::INFINITY, -1.0];
    let result = catch_unwind(|| project_onto_simplex(&v, 3));
    // 不 panic，返回有效结果
    assert!(result.is_ok(), "混合 NaN/Inf 不应 panic");
    let p = result.unwrap();
    // 验证结果所有元素都是有限的
    assert!(p.iter().all(|&x| x.is_finite()), "结果应不含 NaN/Inf");
    // 验证是合法单纯形：非负且和≈1
    let sum: f32 = p.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
    for &x in &p {
        assert!(x >= 0.0, "元素应为非负: {}", x);
    }
}

// ============================================================================
// 场景 2: 零向量/空输入
// ============================================================================

#[test]
fn simplex_empty_vector_returns_empty() {
    // 空向量应返回空向量（退化但合法）
    let v: Vec<f32> = vec![];
    let p = project_onto_simplex(&v, 0);
    assert_eq!(p.len(), 0);
}

#[test]
fn simplex_zero_vector_uniform_projection() {
    // 零向量 [0,0,...,0] 应投影到均匀分布 [1/k, 1/k, ..., 1/k]
    let k = 4;
    let v = vec![0.0f32; k];
    let p = project_onto_simplex(&v, k);
    assert!(is_valid_simplex(&p, 1e-5));
    for &x in &p {
        assert!(approx_eq(x, 1.0 / k as f32, 1e-5));
    }
}

#[test]
fn simplex_k_zero_returns_empty() {
    // k=0 应返回空向量
    let v = vec![1.0, 2.0, 3.0];
    let p = project_onto_simplex(&v, 0);
    assert_eq!(p.len(), 0);
}

#[test]
fn simplex_k_greater_than_len_truncates() {
    // k > v.len() 应安全截断到 v.len()
    let v = vec![1.0, 2.0];
    let p = project_onto_simplex(&v, 100);
    assert_eq!(p.len(), 2);
    assert!(is_valid_simplex(&p, 1e-5));
}

// ============================================================================
// 场景 3: 负值/混合符号
// ============================================================================

#[test]
fn simplex_all_negative_clamps_to_zero() {
    // 全负值向量应投影到某个有效单纯形（不一定均匀）
    let v = vec![-5.0, -1.0, -3.0, -7.0];
    let p = project_onto_simplex(&v, 4);
    assert!(is_valid_simplex(&p, 1e-5));
    // Duchi 投影可能将所有负值截断为 0，然后重新归一化
}

#[test]
fn simplex_mixed_sign() {
    // 混合正负值
    let v = vec![-5.0, -1.0, 3.0, 7.0];
    let p = project_onto_simplex(&v, 4);
    assert!(is_valid_simplex(&p, 1e-5));
}

#[test]
fn simplex_negative_with_zero() {
    // 含零的混合符号
    let v = vec![-3.0, 0.0, 5.0, 2.0];
    let p = project_onto_simplex(&v, 4);
    assert!(is_valid_simplex(&p, 1e-5));
}

// ============================================================================
// 场景 4: 全等值（已归一化）- 幂等性测试
// ============================================================================

#[test]
fn simplex_idempotent_uniform() {
    // 已归一化的均匀向量投影后应不变
    let v = vec![0.25; 4];
    let p1 = project_onto_simplex(&v, 4);
    let p2 = project_onto_simplex(&p1, 4);
    for (&x1, &x2) in p1.iter().zip(p2.iter()) {
        assert!(approx_eq(x1, x2, 1e-6), "幂等性失败: {} vs {}", x1, x2);
    }
}

#[test]
fn simplex_idempotent_arbitrary() {
    // 任意向量投影两次应等于投影一次
    let v = vec![1.0, 5.0, -3.0, 2.0];
    let p1 = project_onto_simplex(&v, 4);
    let p2 = project_onto_simplex(&p1, 4);
    for (&x1, &x2) in p1.iter().zip(p2.iter()) {
        assert!(approx_eq(x1, x2, 1e-6), "幂等性失败: {} vs {}", x1, x2);
    }
}

#[test]
fn simplex_already_normalized_is_fixed_point() {
    // 任何合法单纯形投影后应不变
    let v = vec![0.6, 0.3, 0.1];
    let p = project_onto_simplex(&v, 3);
    for (&orig, &proj) in v.iter().zip(p.iter()) {
        assert!(approx_eq(orig, proj, 1e-5));
    }
}

// ============================================================================
// 场景 5: f32 累计精度（大动态范围）
// ============================================================================

#[test]
fn simplex_very_small_values() {
    // 极小值 [1e-7, ...] 测试精度
    let v: Vec<f32> = vec![1e-7; 8];
    let p = project_onto_simplex(&v, 8);
    assert!(is_valid_simplex(&p, 1e-4), "极小值精度失效");
}

#[test]
fn simplex_very_large_values() {
    // F2 修复后：极大值使用 f64 累加，精度应在 1e-4 容差内
    let v: Vec<f32> = vec![1e8, 1e8, 1e8, 1e8];
    let p = project_onto_simplex(&v, 4);
    assert!(is_valid_simplex(&p, 1e-5), "极大值精度失效: {:?}", p);
    // 验证均匀分布
    for &x in &p {
        assert!(approx_eq(x, 0.25, 1e-5), "应为均匀分布: {}", x);
    }
}

#[test]
fn simplex_large_dynamic_range() {
    // F2 修复后：大动态范围使用 f64 累加，精度应在 1e-5 容差内
    let v = vec![1e-10, 1e10, 1.0, 0.5];
    let p = project_onto_simplex(&v, 4);
    assert!(is_valid_simplex(&p, 1e-5), "大动态范围精度失效: {:?}", p);
    // 验证和≈1
    let sum: f32 = p.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
}

#[test]
fn simplex_catastrophic_cancellation() {
    // F2 修复后：灾难性抵消使用 f64 累加，精度应在 1e-5 容差内
    let v = vec![1e8, 1e8, -1e8, -1e8];
    let p = project_onto_simplex(&v, 4);
    assert!(is_valid_simplex(&p, 1e-5), "灾难性抵消导致精度损失: {:?}", p);
    // 验证和≈1
    let sum: f32 = p.iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
}

// ============================================================================
// 场景 6: k 越界
// ============================================================================

#[test]
fn simplex_k_far_greater_than_len() {
    // k 远大于 v.len() 应安全
    let v = vec![1.0, 2.0];
    let p = project_onto_simplex(&v, 1000);
    assert_eq!(p.len(), 2);
    assert!(is_valid_simplex(&p, 1e-5));
}

#[test]
fn simplex_inplace_k_max_boundary() {
    // 测试 inplace 版本的 K_MAX 边界
    const K_MAX: usize = 8;
    let mut v = vec![1.0; K_MAX];
    project_onto_simplex_inplace(&mut v, K_MAX);
    assert!(is_valid_simplex(&v, 1e-5));
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "k")]
fn simplex_inplace_k_exceeds_k_max() {
    // k > K_MAX 在 debug 模式 panic，release 截断
    const K_MAX: usize = 8;
    let mut v = vec![1.0; K_MAX + 2];
    // Debug 模式 panic
    project_onto_simplex_inplace(&mut v, K_MAX + 2);
}

// ============================================================================
// 场景 7: frobenius_distance 边界
// ============================================================================

#[test]
fn frobenius_identical_matrices_zero_distance() {
    // 两个相同矩阵距离应为 0
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut a = [[0.0f32; K_MAX]; MAX_SLICES];
    let mut b = [[0.0f32; K_MAX]; MAX_SLICES];
    for s in 0..MAX_SLICES {
        for i in 0..K_MAX {
            a[s][i] = 0.3;
            b[s][i] = 0.3;
        }
    }
    let dims = [K_MAX as u8; MAX_SLICES];
    let d = frobenius_distance(&a, &b, &dims);
    assert!(d < 1e-6, "相同矩阵距离应为 0: {}", d);
}

#[test]
fn frobenius_negative_distance_impossible() {
    // 距离不应为负
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let a = [[0.0f32; K_MAX]; MAX_SLICES];
    let b = [[1.0f32; K_MAX]; MAX_SLICES];
    let dims = [K_MAX as u8; MAX_SLICES];
    let d = frobenius_distance(&a, &b, &dims);
    assert!(d >= 0.0, "距离不应为负: {}", d);
}

#[test]
fn frobenius_nan_in_matrix() {
    // BUG: 矩阵含 NaN 时距离应为 NaN，但未检查
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut a = [[0.0f32; K_MAX]; MAX_SLICES];
    a[0][0] = f32::NAN;
    let b = [[0.0f32; K_MAX]; MAX_SLICES];
    let dims = [K_MAX as u8; MAX_SLICES];
    let d = frobenius_distance(&a, &b, &dims);
    // 当前行为：返回 NaN（未检查）
    assert!(d.is_nan(), "含 NaN 矩阵产生 NaN 距离");
}

#[test]
fn frobenius_padding_ignored() {
    // 验证 padding 列差异被忽略
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let k = 3;
    let mut a = [[0.0f32; K_MAX]; MAX_SLICES];
    let mut b = [[0.0f32; K_MAX]; MAX_SLICES];

    for s in 0..MAX_SLICES {
        for i in 0..k {
            a[s][i] = 0.2;
            b[s][i] = 0.2;
        }
        for i in k..K_MAX {
            a[s][i] = 0.99;
            b[s][i] = -0.5;
        }
    }
    let dims = [k as u8; MAX_SLICES];
    let d = frobenius_distance(&a, &b, &dims);
    assert!(d < 1e-6, "padding 差异被忽略: {}", d);
}

// ============================================================================
// 场景 8: SimplexCodec 往返
// ============================================================================

#[test]
fn codec_round_trip_exact() {
    // 正常输入往返应无损
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = simplex_row(&[0.2, 0.3]); // k=3: [0.2, 0.3, 0.5]
    m[1] = simplex_row(&[0.1, 0.1, 0.1]); // k=4
    let dims = slice_dims(&[3, 4]);

    let enc = SimplexCodec::encode(&m, &dims);
    let dec = SimplexCodec::decode(&enc, &dims, K_MAX);

    for (s, (&k, (d_row, m_row))) in dims.iter().zip(dec.iter().zip(m.iter())).enumerate() {
        for i in 0..k as usize {
            assert!(approx_eq(d_row[i], m_row[i], 1e-6),
                "往返失败: [{},{}] {} vs {}", s, i, d_row[i], m_row[i]);
        }
    }
}

#[test]
fn codec_k0_round_trip() {
    // k=0 时，encode 跳过该行，decode 返回全零（正确行为）
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let m = [[0.0f32; K_MAX]; MAX_SLICES];
    let dims = slice_dims(&[0]);

    let enc = SimplexCodec::encode(&m, &dims);
    assert_eq!(enc.len(), 0, "k=0 编码应为空");

    let dec = SimplexCodec::decode(&enc, &dims, K_MAX);

    // 验证 decode 返回全零行
    for &v in &dec[0] {
        assert_eq!(v, 0.0, "k=0 解码应为全零");
    }
}

#[test]
fn codec_k1_round_trip() {
    // k=1 时只有一列，编码存 0 列，解码恢复为 1.0
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0][0] = 1.0; // k=1: [1.0]
    let dims = slice_dims(&[1]);

    let enc = SimplexCodec::encode(&m, &dims);
    let dec = SimplexCodec::decode(&enc, &dims, K_MAX);

    assert!(approx_eq(dec[0][0], 1.0, 1e-6), "k=1 解码失败");
}

#[test]
fn codec_zero_weight_preservation() {
    // 含零权重的行应正确保留
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = simplex_row(&[0.5, 0.0]); // k=3: [0.5, 0.0, 0.5]
    let dims = slice_dims(&[3]);

    let enc = SimplexCodec::encode(&m, &dims);
    let dec = SimplexCodec::decode(&enc, &dims, K_MAX);

    assert!(approx_eq(dec[0][0], 0.5, 1e-6));
    assert!(dec[0][1].abs() < 1e-6, "零权重未保留: {}", dec[0][1]);
    assert!(approx_eq(dec[0][2], 0.5, 1e-6));
}

#[test]
#[should_panic(expected = "sum")]
fn codec_encode_invalid_sum_panics() {
    // 行和不为 1 应 panic
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = [0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]; // sum=1.5
    let dims = slice_dims(&[3]);
    SimplexCodec::encode(&m, &dims);
}

#[test]
fn codec_padding_zeros_after_decode() {
    // 解码后 padding 列应为 0
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = simplex_row(&[0.25, 0.25]); // k=3
    let dims = slice_dims(&[3]);

    let enc = SimplexCodec::encode(&m, &dims);
    let dec = SimplexCodec::decode(&enc, &dims, K_MAX);

    for &v in dec[0][3..K_MAX].iter() {
        assert_eq!(v, 0.0, "padding 列应为 0");
    }
}

// ============================================================================
// 场景 9: pareto_project 边界
// ============================================================================

#[test]
fn pareto_empty_constraints_noop() {
    // 空约束列表应为无操作
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = [0.6, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let dims = slice_dims(&[2]);
    let before = m;

    pareto_project(&mut m, &[], &dims);

    // 应保持不变（投影一次）
    for (b, a) in before[0].iter().zip(m[0].iter()) {
        assert!(approx_eq(*b, *a, 1e-6));
    }
}

#[test]
fn pareto_nan_value_constraint() {
    // F3 修复后：约束 value 为 NaN 时跳过该约束，不传播 NaN
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let dims = slice_dims(&[2]);

    let cs = [Constraint {
        slice: 0,
        endpoint: 0,
        value: f32::NAN,
        kind: ConstraintKind::LowerBound,
    }];

    pareto_project(&mut m, &cs, &dims);

    // NaN 约束被跳过，结果应为有限且合法单纯形
    assert!(m[0].iter().all(|&x| x.is_finite()), "NaN 应被跳过，结果应有限");
    let sum: f32 = m[0][..2].iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
}

#[test]
fn pareto_inf_value_constraint_propagates() {
    // F3 修复后：约束 value 为 Inf 时跳过该约束，不传播 Inf
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let dims = slice_dims(&[2]);

    let cs = [Constraint {
        slice: 0,
        endpoint: 0,
        value: f32::INFINITY,
        kind: ConstraintKind::LowerBound,
    }];

    pareto_project(&mut m, &cs, &dims);

    // Inf 约束被跳过，结果应为有限且合法单纯形
    assert!(m[0].iter().all(|&x| x.is_finite()), "Inf 应被跳过，结果应有限");
    let sum: f32 = m[0][..2].iter().sum();
    assert!(approx_eq(sum, 1.0, 1e-5), "和应为 1: {}", sum);
}

#[test]
fn pareto_slice_out_of_bounds() {
    // BUG: slice >= MAX_SLICES 时当前实现静默跳过
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let dims = slice_dims(&[2]);

    let cs = [Constraint {
        slice: 255, // 越界
        endpoint: 0,
        value: 0.8,
        kind: ConstraintKind::LowerBound,
    }];

    // 当前行为：静默跳过（不 panic）
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        pareto_project(&mut m, &cs, &dims);
    }));
    assert!(result.is_ok(), "越界 slice 应静默跳过");
    // 验证 m[0] 未被修改
    assert!(approx_eq(m[0][0], 0.5, 1e-6), "越界约束不应影响有效数据");
}

#[test]
fn pareto_endpoint_out_of_bounds() {
    // BUG: endpoint >= K_MAX 时当前实现静默跳过
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let dims = slice_dims(&[2]);

    let cs = [Constraint {
        slice: 0,
        endpoint: 255, // 越界
        value: 0.8,
        kind: ConstraintKind::LowerBound,
    }];

    // 当前行为：静默跳过（不 panic）
    let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
        pareto_project(&mut m, &cs, &dims);
    }));
    assert!(result.is_ok(), "越界 endpoint 应静默跳过");
}

#[test]
fn pareto_linear_constraint_zero_norm() {
    // 线性约束系数全为零（norm_sq=0）应被跳过
    const MAX_SLICES: usize = 16;
    const K_MAX: usize = 8;
    let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
    m[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let dims = slice_dims(&[2]);

    let coeff = [0.0f32; K_MAX];
    // 全零系数
    let cs = [Constraint {
        slice: 0,
        endpoint: 0,
        value: 0.0,
        kind: ConstraintKind::Linear {
            coefficients: coeff,
            target: 0.8,
        },
    }];

    pareto_project(&mut m, &cs, &dims);
    // 应跳过该约束，不影响结果
}

// ============================================================================
// 场景 10: SIMD ⇔ 标量一致性
// ============================================================================

#[test]
fn spmv_simd_vs_scalar_random() {
    // 固定随机种子测试 SpMV SIMD 与标量一致性
    let edges = fixed_random_edges();
    let n = 10u32;
    let m = CascadeMatrix::from_edges(n, &edges);
    let x: Vec<f32> = (0..n).map(|i| (i as f32) * 0.3 + 0.1).collect();

    let mut y_scalar = vec![0.0f32; n as usize];
    let mut y_simd = vec![0.0f32; n as usize];

    spmv_csr_scalar(&x, &m, &mut y_scalar);
    spmv_csr(&x, &m, &mut y_simd);

    for (i, (&s, &v)) in y_scalar.iter().zip(y_simd.iter()).enumerate() {
        assert!(approx_eq(s, v, 1e-5), "[{}] SIMD {} vs 标量 {}", i, v, s);
    }
}

#[test]
fn spmv_simd_vs_scalar_negative_weights() {
    // 含负权边的测试
    let edges = [(0, 1, -0.5, 0), (1, 2, 0.8, 0), (0, 2, -0.3, 0)];
    let m = CascadeMatrix::from_edges(3, &edges);
    let x = vec![1.0f32, 2.0, 3.0];

    let mut y_scalar = vec![0.0f32; 3];
    let mut y_simd = vec![0.0f32; 3];

    spmv_csr_scalar(&x, &m, &mut y_scalar);
    spmv_csr(&x, &m, &mut y_simd);

    for (i, (&s, &v)) in y_scalar.iter().zip(y_simd.iter()).enumerate() {
        assert!(approx_eq(s, v, 1e-5), "[{}] 负权 SIMD {} vs 标量 {}", i, v, s);
    }
}

#[test]
fn spmv_simd_vs_scalar_empty_rows() {
    // 含空行的矩阵
    let edges = [(1, 0, 0.5, 0), (2, 1, 0.3, 0)];
    let m = CascadeMatrix::from_edges(4, &edges);
    let x = vec![1.0f32; 4];

    let mut y_scalar = vec![0.0f32; 4];
    let mut y_simd = vec![0.0f32; 4];

    spmv_csr_scalar(&x, &m, &mut y_scalar);
    spmv_csr(&x, &m, &mut y_simd);

    for (i, (&s, &v)) in y_scalar.iter().zip(y_simd.iter()).enumerate() {
        assert!(approx_eq(s, v, 1e-5), "[{}] 空行 SIMD {} vs 标量 {}", i, v, s);
    }
}

#[test]
fn spmv_simd_vs_scalar_dense_row() {
    // 密集行测试（接近 K_MAX）
    let n = 10u32;
    let mut edges = Vec::new();
    for j in 0..8 {
        edges.push((0, j, 0.1, 0));
    }
    let m = CascadeMatrix::from_edges(n, &edges);
    let x: Vec<f32> = (0..n).map(|_| 0.5).collect();

    let mut y_scalar = vec![0.0f32; n as usize];
    let mut y_simd = vec![0.0f32; n as usize];

    spmv_csr_scalar(&x, &m, &mut y_scalar);
    spmv_csr(&x, &m, &mut y_simd);

    for (i, (&s, &v)) in y_scalar.iter().zip(y_simd.iter()).enumerate() {
        assert!(approx_eq(s, v, 1e-5), "[{}] 密集 SIMD {} vs 标量 {}", i, v, s);
    }
}

#[test]
fn spmv_simd_vs_scalar_boundary_sizes() {
    // 测试边界大小：0, 1, 7, 8, 9 个非零元（LANES=8）
    for nnz in [0, 1, 7, 8, 9] {
        let n = 10u32;
        let mut edges = Vec::new();
        for j in 0..nnz {
            edges.push((0, j, 0.1 * (j + 1) as f32, j));
        }
        let m = CascadeMatrix::from_edges(n, &edges);
        let x: Vec<f32> = (0..n).map(|_| 1.0).collect();

        let mut y_scalar = vec![0.0f32; n as usize];
        let mut y_simd = vec![0.0f32; n as usize];

        spmv_csr_scalar(&x, &m, &mut y_scalar);
        spmv_csr(&x, &m, &mut y_simd);

        for (i, (&s, &v)) in y_scalar.iter().zip(y_simd.iter()).enumerate() {
            assert!(approx_eq(s, v, 1e-5),
                "nnz={} [{}] SIMD {} vs 标量 {}", nnz, i, v, s);
        }
    }
}

#[test]
fn spmv_nan_in_matrix() {
    // BUG: 矩阵含 NaN 时应传播 NaN，而非产生错误结果
    let edges = [(0, 1, f32::NAN, 0)];
    let m = CascadeMatrix::from_edges(2, &edges);
    let x = vec![1.0f32, 2.0];

    let mut y = vec![0.0f32; 2];
    spmv_csr_scalar(&x, &m, &mut y);

    // NaN 传播到 y[0]
    assert!(y[0].is_nan(), "NaN 权重应传播 NaN: {}", y[0]);
}

#[test]
fn spmv_inf_in_matrix() {
    // BUG: 矩阵含 Inf 时应传播 Inf
    let edges = [(0, 1, f32::INFINITY, 0)];
    let m = CascadeMatrix::from_edges(2, &edges);
    let x = vec![1.0f32, 2.0];

    let mut y = vec![0.0f32; 2];
    spmv_csr_scalar(&x, &m, &mut y);

    // Inf 传播到 y[0]
    assert!(y[0].is_infinite(), "Inf 权重应传播 Inf: {}", y[0]);
}

// ============================================================================
// 测试辅助函数
// ============================================================================

/// 构造单纯形行：前 k-1 个权重，第 k 列 = 1 - sum
fn simplex_row(first: &[f32]) -> [f32; 8] {
    const K_MAX: usize = 8;
    let mut r = [0.0f32; K_MAX];
    let mut s = 0.0f32;
    for (i, &v) in first.iter().enumerate() {
        r[i] = v;
        s += v;
    }
    r[first.len()] = 1.0 - s;
    r
}

/// 构造 slice_dims 数组
fn slice_dims(ks: &[u8]) -> [u8; 16] {
    const MAX_SLICES: usize = 16;
    let mut d = [0u8; MAX_SLICES];
    for (i, &k) in ks.iter().enumerate() {
        d[i] = k;
    }
    d
}

/// 固定"随机"边列表（不含真正随机）
fn fixed_random_edges() -> Vec<(u32, u32, f32, u32)> {
    vec![
        (0, 1, 0.5, 10),
        (0, 3, 0.3, 20),
        (1, 2, 0.8, 5),
        (2, 0, -0.2, 0),
        (3, 4, 0.6, 15),
        (4, 1, 0.1, 8),
        (5, 6, 0.4, 12),
        (6, 7, 0.9, 3),
        (7, 8, 0.2, 7),
        (8, 9, 0.7, 18),
    ]
}
