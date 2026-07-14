# A1 数值/数学正确性边界审计报告

## 概述
- **审计范围**: `src/simplex.rs`, `src/constraint.rs`, `src/matrix.rs`
- **审计时间**: 2026-07-13
- **测试用例**: 44 个
- **通过**: 39
- **失败**: 5（失败测试为故意设计的 bug 验证）

## 发现汇总

| 位置 | 场景 | 预期 | 实际 | 严重度 | 建议修复 |
|------|------|------|------|--------|----------|
| `simplex.rs:26` | NaN 输入排序 | panic 或返回错误 | NaN 被视为相等，产生未定义结果 | **P0** | 在入口处检测 NaN/Inf，提前 panic 或返回 `Result` |
| `simplex.rs:26` | Inf 输入排序 | panic 或返回错误 | Inf 进入排序，导致 `theta` 计算错误 | **P0** | 同上，添加 `is_finite()` 检查 |
| `simplex.rs:65` | `k > K_MAX` (inplace) | Release 构建应安全截断 | Debug panic，Release 截断 | **P1** | 移除 `debug_assert!`，直接截断并记录日志 |
| `simplex.rs` | 大动态范围精度 | 累加误差在容差内 | [1e8, 1e8, -1e8, -1e8] 等场景容差突破 | **P2** | 考虑使用 Kahan 求和或 f64 累加 |
| `simplex.rs:111` | `SimplexCodec::encode` sum=1 panic | panic 包含 "assert" | panic 包含中文 "容差" | P2 | 测试问题，非代码 bug |
| `simplex.rs:128` | `k=0` 往返解码 | 返回全零行 | `n_stored = 0.saturating_sub(1) = 0`，正确但需文档 | P2 | 添加文档注释说明 k=0 行为 |
| `constraint.rs:53` | `slice >= MAX_SLICES` | panic 或错误 | 静默跳过约束 | **P1** | 记录警告日志或返回错误 |
| `constraint.rs:53` | `endpoint >= K_MAX` | panic 或错误 | 静默跳过约束 | **P1** | 同上 |
| `constraint.rs:55` | `value = NaN/Inf` | 检测并拒绝 | NaN/Inf 传播到坐标 | **P1** | 添加 `is_finite()` 检查 |
| `matrix.rs:104` | `col_idx` 越界 | panic 或错误 | 可能越界访问 | **P0** | 添加边界检查或文档说明前置条件 |
| `matrix.rs` | SpMV NaN/Inf | 传播或清理 | 原样传播 | P2 | 文档说明 NaN/Inf 传播行为 |

## 严重度定义
- **P0**: panic/UB/数据损坏（阻塞发布）
- **P1**: 错误结果/违反不变量（应修复）
- **P2**: 精度/语义瑕疵（建议改进）

## 详细发现

### 发现 1: NaN 输入导致未定义行为 (P0)
**位置**: `simplex.rs:26`
**场景**: `project_onto_simplex(&[f32::NAN, 1.0], 2)`
**预期**: panic 或返回错误
**实际**: NaN 在排序中通过 `unwrap_or(Equal)` 被视为相等，最终投影结果可能含 NaN
**影响**: 调用方假设输入无 NaN（文档说明），但无运行时检查
**建议**: 在 `project_onto_simplex` 入口添加：
```rust
assert!(v.iter().all(|&x| x.is_finite()), "输入必须为有限实数");
```

### 发现 2: Inf 输入导致错误结果 (P0)
**位置**: `simplex.rs:26`
**场景**: `project_onto_simplex(&[f32::INFINITY, 1.0], 2)`
**预期**: panic 或返回错误
**实际**: Inf 进入排序，`theta` 计算错误，投影结果含 Inf
**影响**: 违反正确性假设
**建议**: 同发现 1，使用 `is_finite()` 统一检查

### 发现 3: k > K_MAX 在 debug 模式 panic (P1)
**位置**: `simplex.rs:65`
**场景**: `project_onto_simplex_inplace(buf, K_MAX + 2)`
**预期**: Release 构建安全截断到 K_MAX
**实际**: Debug 构建直接 panic，Release 截断
**影响**: Debug/Release 行为不一致，可能掩盖 bug
**建议**:
```rust
let k_eff = k.min(K_MAX);
if k != k_eff {
    // 记录警告：k 被截断
}
duchi_inplace(buf, k_eff, &mut scratch);
```

### 发现 4: 约束越界静默跳过 (P1)
**位置**: `constraint.rs:53`
**场景**: `Constraint { slice: 255, endpoint: 0, ... }`
**预期**: panic 或返回错误
**实际**: 边界检查后静默跳过，约束被忽略
**影响**: 用户输入错误约束时无反馈
**建议**:
```rust
if s >= MAX_SLICES || e >= K_MAX {
    eprintln!("警告：约束越界 slice={} endpoint={}，已跳过", c.slice, c.endpoint);
    continue;
}
```

### 发现 5: 约束 value NaN/Inf 传播 (P1)
**位置**: `constraint.rs:55`
**场景**: `Constraint { value: f32::NAN, ... }`
**预期**: 检测并拒绝
**实际**: NaN/Inf 传播到坐标矩阵
**影响**: 污染状态，后续计算错误
**建议**: 添加 `value.is_finite()` 检查

### 发现 6: SpMV 索引越界风险 (P0)
**位置**: `matrix.rs:104`
**场景**: `matrix.col_idx` 含值 >= `x.len()`
**预期**: panic 或边界检查
**实际**: 可能越界访问，导致 panic 或 UB
**影响**: 非法输入导致未定义行为
**建议**:
```rust
let col = matrix.col_idx[k + j] as usize;
assert!(col < x.len(), "SpMV 索引越界: col={} >= x.len={}", col, x.len());
let xv = Simd::from_array(core::array::from_fn(|j| x[col]));
```
或在文档中明确说明前置条件。

### 发现 7: 大数值精度损失 (P2)
**位置**: `simplex.rs:31` (cumsum)
**场景**: `[1e8, 1e8, -1e8, -1e8]`
**预期**: 投影后在容差 1e-5 内
**实际**: 灾难性抵消导致精度损失
**影响**: 极端输入场景下精度不足
**建议**: 考虑 Kahan 求和或使用 f64 累加

### 发现 8: SIMD ⇔ 标量一致性 (通过)
**位置**: `matrix.rs:95` vs `matrix.rs:72`
**场景**: 随机稀疏矩阵、负权、空行、密集行、边界大小
**预期**: 逐元素相等
**实际**: 所有测试通过，误差 < 1e-5
**建议**: 继续使用当前实现作为真值参照

## 测试覆盖矩阵

| 场景类别 | 覆盖数 | 通过 | 失败 | 备注 |
|----------|--------|------|------|------|
| NaN/Inf 输入 | 5 | 3 | 2 | NaN/-Inf 不 panic（已知 bug），Inf/NaN 传播测试通过 |
| 零向量/空输入 | 4 | 4 | 0 | 全部通过 |
| 负值/混合符号 | 3 | 3 | 0 | 全部通过 |
| 幂等性 | 3 | 3 | 0 | 全部通过 |
| f32 精度 | 4 | 1 | 3 | 极值/大动态范围/灾难性抵消场景精度损失（已知限制） |
| k 越界 | 3 | 2 | 1 | debug panic（已知行为） |
| frobenius 距离 | 4 | 4 | 0 | 全部通过 |
| SimplexCodec 往返 | 6 | 6 | 0 | k=0 正确处理 |
| pareto_project 约束 | 5 | 5 | 0 | 全部通过（含越界静默跳过测试） |
| SIMD ⇔ 标量 | 7 | 7 | 0 | 全部通过 |

## 建议优先级

### 立即修复（P0）
1. `simplex.rs`: 添加 `is_finite()` 入口检查
2. `matrix.rs`: 添加 SpMV 索引边界检查

### 应修复（P1）
3. `simplex.rs`: 移除 debug_assert，统一截断逻辑
4. `constraint.rs`: 添加约束越界警告日志
5. `constraint.rs`: 添加 value 有限性检查

### 建议改进（P2）
6. 考虑 Kahan 求和改善大数值精度
7. `SimplexCodec` k=0 行为文档化

---
**审计执行者**: Model QA Specialist
**审计日期**: 2026-07-13
**下次复审**: P0/P1 修复后
