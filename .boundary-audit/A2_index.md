# A2 索引/拓扑边界审计报告

## 概述

审计 Σ⁴-Engine 索引和拓扑边界，重点探测编译期硬约束的运行时行为：
- `MAX_SLICES=16`
- `K_MAX=8`
- `MAX_ENTITIES=65_536`
- `DELTA_RING_CAPACITY=1024`

## 发现汇总

| 严重度 | 数量 |
|--------|------|
| P0 (panic/UB) | 2 |
| P1 (错误结果/越界) | 7 |
| P2 (语义瑕疵/不一致) | 3 |

**总计：12 个边界问题**

---

## 详细发现

### P0 级别（panic/未定义行为）

| 位置 | 场景 | 预期 | 实际 | 建议修复 |
|------|------|------|------|----------|
| `matrix.rs:104` | `from_edges` 接受 `to >= n` 的边 | 拒绝非法边 | 被接受，`spmv_csr` 访问 `x[col]` 越界 panic | 在 `from_edges` 中验证 `to < n` |
| `cascade.rs:69-74` | `cascade` 中 `signal.len() > entity_states.len()` | panic/拒绝 | 静默跳过（检查 `i >= entity_states.len()`） | 验证 `entity_states.len() >= matrix.n` |

### P1 级别（错误结果/越界）

| 位置 | 场景 | 预期 | 实际 | 建议修复 |
|------|------|------|------|----------|
| `entity.rs:139-163` | `Entity::new(id, _, num_slices)` | `id < MAX_ENTITIES`, `num_slices <= MAX_SLICES` | 无验证，接受任意值 | 在构造函数中添加断言 |
| `entity.rs:202-207` | `query_state` 中 `slice_mask` 指向未启用切面 | 拒绝或警告 | 修改 `coordinates[slice]` 其中 `slice >= num_slices` | 验证 `slice < num_slices` |
| `entity.rs:202-207` | `slice_mask` 多位设置 | 错误/警告 | 只修改最低位切面（`trailing_zeros`） | 文档化此行为或改为错误 |
| `constraint.rs:53-56` | `Constraint.slice >= MAX_SLICES` | 拒绝 | 静默跳过 | 已有边界检查，行为正确 |
| `constraint.rs:63-66` | `Constraint.endpoint >= K_MAX` | 拒绝 | 静默跳过 | 已有边界检查，行为正确 |
| `cascade.rs:107-109` | `initial.len() < matrix.n` | panic/错误 | 填充 0 | 文档化或改为错误 |
| `matrix.rs:97-98` | `spmv_csr` 中 `y.len() != matrix.n` | panic | zip 静默截断 | 添加长度断言 |

### P2 级别（语义瑕疵）

| 位置 | 场景 | 预期 | 实际 | 建议修复 |
|------|------|------|------|----------|
| `entity.rs:169-176` | Delta ring 溢出 | panic（设计行为） | panic "delta ring overflow" | 无需修复，按设计工作 |
| `matrix.rs:26-66` | 重复边 | 权重累加或覆盖 | 保留两条边，权重分别存储 | 文档化当前行为 |
| `cascade.rs:69-74` | 自环边 `(i,i,w,lag)` | 警告/限制 | 无特殊处理，每跳更新 | 文档化 |

---

## TOP 3 最严重问题

1. **`matrix.rs:104` - `from_edges` 未验证 `to` 索引**
   - `from_edges(3, &[(0, 5, ...)])` 被接受，但 `spmv_csr` 访问 `x[5]` 会 panic
   - **影响**：构造阶段不报错，运行时 panic
   - **修复**：在 `from_edges` 第 48 行添加 `if to < n { ... }` 检查

2. **`entity.rs:139` - `Entity::new` 无参数验证**
   - `Entity::new(100000, 0, 17)` 被接受，违反 `MAX_ENTITIES` 和 `MAX_SLICES` 约束
   - **影响**：实体 ID 超出池容量，后续访问越界
   - **修复**：添加 `assert!(id < MAX_ENTITIES as u32)` 和 `assert!(num_slices <= MAX_SLICES as u8)`

3. **`cascade.rs:107` - `initial` 向量长度不匹配静默填充**
   - `initial.len() < matrix.n` 时用 0 填充而非报错
   - **影响**：用户误以为信号已完整设置，实际部分被丢弃
   - **修复**：在 `cascade` 开头添加 `assert!(initial.len() >= matrix.n)`

---

## 测试覆盖

测试文件：`tests/index_boundary.rs`
- 测试用例数：29
- 编译状态：通过
- 测试结果：29 passed

### 关键测试场景

1. `entity_new_num_slices_exceeds_max_is_accepted` - 验证 num_slices > 16 被接受
2. `entity_new_id_exceeds_max_entities_is_accepted` - 验证 id >= 65536 被接受
3. `apply_delta_ring_overflow_panics` - 验证 ring 溢出 panic（按设计）
4. `spmv_csr_invalid_col_idx_panics` - 验证无效 to 索引导致 panic
5. `matrix_from_edges_invalid_to_is_accepted` - 验证 from_edges 不检查 to
6. `cascade_initial_shorter_than_n_is_padded` - 验证 initial 向量被填充
7. `query_state_slice_mask_multiple_bits_set` - 验证多位 mask 行为

---

## 阻塞问题

无阻塞问题。所有边界问题均已探测并记录，可以继续其他审计任务。
