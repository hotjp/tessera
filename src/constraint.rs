//! 帕累托约束投影（SPEC §2.5）。
//!
//! 在 Duchi 单纯形投影之外，支持对坐标施加盒约束（lower/upper）与线性等式约束。
//! 按固定顺序 lower → upper → linear 应用，随后重新执行单纯形投影
//! （约束可能破坏 `Σ=1`，故**必须重投影**）。

use crate::constants::{K_MAX, MAX_SLICES};
use crate::simplex::project_onto_simplex_inplace;

/// 约束类型。
pub enum ConstraintKind {
    /// 下界：`coords[slice][endpoint] >= value`。
    LowerBound,
    /// 上界：`coords[slice][endpoint] <= value`。
    UpperBound,
    /// 线性等式：`Σ_i coefficients[i] · coords[slice][i] == target`。
    ///
    /// 作用于单个切面的整行；`Constraint.endpoint` 字段不使用。
    Linear {
        /// 各端点的系数。
        coefficients: [f32; K_MAX],
        /// 目标点积。
        target: f32,
    },
}

/// 单条约束。
#[repr(C)]
pub struct Constraint {
    /// 作用切面索引。
    pub slice: u8,
    /// Lower/Upper 约束的端点索引（Linear 不使用）。
    pub endpoint: u8,
    /// Lower/Upper 的界值（Linear 不使用，target 在 [`ConstraintKind::Linear`] 内）。
    pub value: f32,
    /// 约束种类。
    pub kind: ConstraintKind,
}

/// 按固定顺序 lower → upper → linear 应用约束（**不重投影**）。
///
/// Linear 约束按正交投影到超平面 `Σ coeff·coord = target` 实现。
fn apply_constraints(
    raw: &mut [[f32; K_MAX]; MAX_SLICES],
    constraints: &[Constraint],
    slice_dims: &[u8],
) {
    // 1. 下界：coord = max(coord, value)
    for c in constraints
        .iter()
        .filter(|c| matches!(c.kind, ConstraintKind::LowerBound))
    {
        // F3 fix: check constraint value is finite before applying
        if !c.value.is_finite() {
            continue;
        }
        let (s, e) = (c.slice as usize, c.endpoint as usize);
        if s < MAX_SLICES && e < K_MAX {
            raw[s][e] = raw[s][e].max(c.value);
        }
    }
    // 2. 上界：coord = min(coord, value)
    for c in constraints
        .iter()
        .filter(|c| matches!(c.kind, ConstraintKind::UpperBound))
    {
        // F3 fix: check constraint value is finite before applying
        if !c.value.is_finite() {
            continue;
        }
        let (s, e) = (c.slice as usize, c.endpoint as usize);
        if s < MAX_SLICES && e < K_MAX {
            raw[s][e] = raw[s][e].min(c.value);
        }
    }
    // 3. 线性等式：正交投影到超平面 Σ coeff·coord = target
    for c in constraints
        .iter()
        .filter(|c| matches!(c.kind, ConstraintKind::Linear { .. }))
    {
        let s = c.slice as usize;
        if s >= MAX_SLICES {
            continue;
        }
        if let ConstraintKind::Linear {
            coefficients,
            target,
        } = &c.kind
        {
            let k = (slice_dims.get(s).copied().unwrap_or(0) as usize).min(K_MAX);
            let mut dot = 0.0f32;
            let mut norm_sq = 0.0f32;
            for (&coord, &coeff) in raw[s][..k].iter().zip(coefficients[..k].iter()) {
                dot += coeff * coord;
                norm_sq += coeff * coeff;
            }
            if norm_sq > 0.0 {
                let step = (*target - dot) / norm_sq;
                for (coord, &coeff) in raw[s][..k].iter_mut().zip(coefficients[..k].iter()) {
                    *coord += step * coeff;
                }
            }
        }
    }
}

/// 帕累托约束投影：固定顺序应用约束后，对每行重新执行 Duchi 单纯形投影。
///
/// 约束可能使行和偏离 1，故**必须重投影**以恢复 `Σ=1`。
pub fn pareto_project(
    raw: &mut [[f32; K_MAX]; MAX_SLICES],
    constraints: &[Constraint],
    slice_dims: &[u8],
) {
    apply_constraints(raw, constraints, slice_dims);
    // 重投影每行（约束后行和可能 != 1）
    for (row, &k) in raw.iter_mut().zip(slice_dims.iter()) {
        project_onto_simplex_inplace(row, k as usize);
    }
}

#[cfg(test)]
mod pareto {
    use super::*;

    fn dims(ks: &[u8]) -> [u8; MAX_SLICES] {
        let mut d = [0u8; MAX_SLICES];
        for (i, &k) in ks.iter().enumerate() {
            d[i] = k;
        }
        d
    }

    #[test]
    fn lower_bound_clamps_up() {
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = [0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let dims = dims(&[3]);
        let cs = [Constraint {
            slice: 0,
            endpoint: 0,
            value: 0.5,
            kind: ConstraintKind::LowerBound,
        }];
        apply_constraints(&mut m, &cs, &dims);
        assert!((m[0][0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn upper_bound_clamps_down() {
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = [0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let dims = dims(&[3]);
        let cs = [Constraint {
            slice: 0,
            endpoint: 0,
            value: 0.3,
            kind: ConstraintKind::UpperBound,
        }];
        apply_constraints(&mut m, &cs, &dims);
        assert!((m[0][0] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn fixed_order_lower_before_upper() {
        // 同一端点 lower=0.5、upper=0.3：lower 先 clamp 到 0.5，upper 后 clamp 到 0.3（upper 胜出）
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = [0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let dims = dims(&[3]);
        let cs = [
            Constraint {
                slice: 0,
                endpoint: 0,
                value: 0.5,
                kind: ConstraintKind::LowerBound,
            },
            Constraint {
                slice: 0,
                endpoint: 0,
                value: 0.3,
                kind: ConstraintKind::UpperBound,
            },
        ];
        apply_constraints(&mut m, &cs, &dims);
        assert!(
            (m[0][0] - 0.3).abs() < 1e-6,
            "upper 后应用应胜出: {}",
            m[0][0]
        );
    }

    #[test]
    fn linear_constraint_hits_target() {
        // Σ coeff·coord = target：[0.5,0.5]·[1,1] = 1.0 → 投影到 =0.8
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let dims = dims(&[2]);
        let mut coeff = [0.0f32; K_MAX];
        coeff[0] = 1.0;
        coeff[1] = 1.0;
        let cs = [Constraint {
            slice: 0,
            endpoint: 0,
            value: 0.0,
            kind: ConstraintKind::Linear {
                coefficients: coeff,
                target: 0.8,
            },
        }];
        apply_constraints(&mut m, &cs, &dims);
        let dot = m[0][0] + m[0][1];
        assert!((dot - 0.8).abs() < 1e-6, "dot={dot}");
    }

    #[test]
    fn pareto_reprojects_each_row_to_sum_one() {
        // upper 约束破坏行和 → pareto_project 必重投影使 Σ=1
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // sum=1
        m[1] = [0.3, 0.3, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0]; // sum=0.9
        let dims = dims(&[2, 3]);
        let cs = [Constraint {
            slice: 0,
            endpoint: 0,
            value: 0.2,
            kind: ConstraintKind::UpperBound,
        }];
        pareto_project(&mut m, &cs, &dims);
        let s0: f32 = m[0][..2].iter().sum();
        let s1: f32 = m[1][..3].iter().sum();
        assert!((s0 - 1.0).abs() < 1e-5, "row0 sum={s0}");
        assert!((s1 - 1.0).abs() < 1e-5, "row1 sum={s1}");
        for (row, &k) in m.iter().zip(dims.iter()).take(2) {
            for &x in row[..k as usize].iter() {
                assert!(x >= 0.0);
            }
        }
    }

    #[test]
    fn compatible_bounds_have_zero_violation() {
        // 单纯形坐标天然 ∈ [0,1]，故 lower=0 / upper=1 约束 violation=0
        let mut m = [[0.0f32; K_MAX]; MAX_SLICES];
        m[0] = [0.5, 0.3, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0];
        let dims = dims(&[3]);
        let cs = [
            Constraint {
                slice: 0,
                endpoint: 0,
                value: 0.0,
                kind: ConstraintKind::LowerBound,
            },
            Constraint {
                slice: 0,
                endpoint: 1,
                value: 0.0,
                kind: ConstraintKind::LowerBound,
            },
            Constraint {
                slice: 0,
                endpoint: 2,
                value: 0.0,
                kind: ConstraintKind::LowerBound,
            },
            Constraint {
                slice: 0,
                endpoint: 0,
                value: 1.0,
                kind: ConstraintKind::UpperBound,
            },
            Constraint {
                slice: 0,
                endpoint: 1,
                value: 1.0,
                kind: ConstraintKind::UpperBound,
            },
            Constraint {
                slice: 0,
                endpoint: 2,
                value: 1.0,
                kind: ConstraintKind::UpperBound,
            },
        ];
        pareto_project(&mut m, &cs, &dims);
        let mut viol = 0.0f32;
        for &x in m[0][..3].iter() {
            viol += (0.0 - x).max(0.0) + (x - 1.0).max(0.0);
        }
        assert!(viol < 1e-6, "violation={viol}");
        let s0: f32 = m[0][..3].iter().sum();
        assert!((s0 - 1.0).abs() < 1e-5);
    }
}
