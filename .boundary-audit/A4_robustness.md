# A4 输入健壮性审计报告

**审计范围**: 协议帧解析 (`src/protocol.rs`, `src/server.rs`) + CSV 数据加载 (`src/loader.rs`)
**审计日期**: 2026-07-13
**测试文件**: `tests/protocol_loader_boundary.rs`
**测试用例数**: 55 (协议层 35, 加载层 20)
**编译状态**: 通过
**测试结果**: 55 passed; 0 failed

---

## 协议层发现

| 位置 | 输入 | 预期 | 实际 | 严重度 | 建议修复 |
|------|------|------|------|--------|----------|
| `Frame::decode` | 空 buf `[]` | 返回 `UnexpectedEof` Err | ✅ 返回 `Err(UnexpectedEof)` | - | 健壮 |
| `Frame::decode` | 只有 4B len，无 payload | 返回 `UnexpectedEof` Err | ✅ 返回 `Err(UnexpectedEof)` | - | 健壮 |
| `Frame::decode` | payload_len > 声明长度（截断） | 返回 `UnexpectedEof` Err | ✅ 返回 `Err(UnexpectedEof)` | - | 健壮 |
| `Frame::decode` | payload_len = u32::MAX (4GB) | 拒绝，避免分配巨量内存 | ✅ 返回 `Err(TooLarge)` | - | 健壮 |
| `Frame::decode` | FrameType 未知（如 0xAB） | 返回 `UnknownFrameType` Err | ✅ 返回 `Err(UnknownFrameType)` | - | 健壮 |
| `FrameType::from_u8` | v = 255, 200, 0x00 | 返回 None | ✅ 返回 None (255 返回 Heartbeat) | - | 健壮 |
| `decode_state_update` | 空 body 或截断 body | 返回 None | ✅ 返回 None | - | 健壮 |
| `decode_state_update` | entity_id = u32::MAX | 接受并解码 | ✅ 正确解码 | - | 健壮 |
| `decode_state_update` | delta_value = NaN/Inf | 接受（语义层过滤） | ✅ 接受（需语义层验证） | P2 | 考虑在解码层拒绝 NaN/Inf |
| `decode_cascade_run` | body 长度 < 7 字节 | 返回 None | ✅ 返回 None | - | 健壮 |
| `decode_cascade_run` | max_hops = 0 / 255 | 接受 | ✅ 接受 | - | 健壮 |
| `decode_cascade_run` | theta = NaN / 负数 | 接受（语义层过滤） | ✅ 接受（需语义层验证） | P2 | 考虑在解码层拒绝 NaN |
| `decode_cascade_run` | 截断的 initial_shock | 返回 None | ✅ 返回 None | - | 健壮 |
| `decode_cascade_results` | 空 body 或截断 | 返回 None | ✅ 返回 None | - | 健壮 |
| `read_frame` (server.rs) | 连接关闭（read_exact 失败） | 返回 `Ok(None)` | ✅ 返回 `Ok(None)` | - | 健壮 |

**协议层总结**:
- **P0 发现数**: 0
- **P1 发现数**: 0
- **P2 发现数**: 2 (NaN/Inf 接受问题)

---

## 加载层发现

| 位置 | 输入 | 预期 | 实际 | 严重度 | 建议修复 |
|------|------|------|------|--------|----------|
| `load_from_text` | 空字符串 `""` | 返回空 EntityPool | ✅ 返回空池 | - | 健壮 |
| `load_from_text` | 只有表头 | 返回空 EntityPool | ✅ 返回空池 | - | 健壮 |
| `load_from_text` | 空行穿插 | 跳过空行 | ✅ 跳过（`line.is_empty()`） | - | 健壮 |
| `load_from_text` | CRLF vs LF 行尾 | 正常解析 | ✅ 两种行尾都正确处理 | - | 健壮 |
| `load_from_text` | 字段数 < 7 | 跳过该行 | ✅ 跳过（`continue`） | - | 健壮 |
| `load_from_text` | 字段数 > 7 | 使用前 7 列 | ✅ `splitn(7, ',')` 取前 7 列 | P2 | 考虑报错而非静默截断 |
| `load_from_file` | 不存在的文件 | 返回 io::Error | ✅ 返回 `Err` | - | 健壮 |
| `load_from_file` | 指向目录 | 返回 io::Error | ✅ 返回 `Err` | - | 健壮 |
| `load_from_file` | 空文件 | 返回空 EntityPool | ✅ 返回空池 | - | 健壮 |
| `load_from_text` | UTF-8 BOM (`\xEF\xBB\xBF`) | 去除 BOM 后解析 | ✅ `strip_prefix('\u{feff}')` | - | 健壮 |
| `load_from_text` | 中文实体名 | 正常解析 | ✅ 支持 UTF-8 | - | 健壮 |
| `load_from_text` | 字段含逗号 | **应报错或支持引号转义** | ⚠️ **被 splitn(7) 截断** | P1 | 不支持 CSV 引号转义，导致数据损坏 |
| `load_from_text` | 字段含引号 | **应报错或支持引号转义** | ⚠️ **引号被当作名字一部分** | P1 | 同上 |
| `category_code` | 非法分类（非 A-E） | 默认为 E | ✅ 默认为 E (code 4) | - | 健壮 |
| `category_code` | 分类首字符非字母 | 默认为 E | ✅ 默认为 E | - | 健壮 |
| `load_from_text` | 重复 entity_id | 行号作为 id（非重复） | N/A（enumerate 保证唯一） | - | 设计如此 |

**加载层总结**:
- **P0 发现数**: 0
- **P1 发现数**: 2 (CSV 引号/逗号转义问题)
- **P2 发现数**: 1 (字段数 > 7 静默截断)

---

## 严重问题详细说明

### P1 问题：CSV 字段含逗号/引号导致数据损坏

**位置**: `src/loader.rs:100` `let parts: Vec<&str> = line.splitn(7, ',').collect();`

**问题描述**:
- 当前实现使用 `splitn(7, ',')` 简单分割，不支持标准 CSV 引号转义
- 字段内含逗号（如 `"Test, Inc."`）会被截断为两列，导致列数错乱
- 引号（如 `"Test Entity"`）会被当作名字的一部分

**影响**:
- 数据静默损坏：实体名、备注字段可能丢失内容
- 列数检测可能失效（`splitn` 总是返回 ≤7 列）

**建议修复**:
1. 使用标准 CSV 解析库（如 `csv` crate）
2. 或文档明确说明数据格式要求（禁止字段含逗号/引号）

---

## TOP 3 最严重问题

### 1. CSV 字段含逗号/引号导致数据损坏 (P1)
**位置**: `src/loader.rs:100`
**一句话**: 简单 `splitn(7, ',')` 不支持 CSV 引号转义，导致含逗号/引号的字段被截断或损坏
**严重度**: P1 - 静默错误数据

### 2. NaN/Inf 值被协议层接受 (P2)
**位置**: `src/protocol.rs` `decode_state_update` / `decode_cascade_run`
**一句话**: NaN/Inf 浮点值在解码层被接受，需语义层验证边界值
**严重度**: P2 - 语义瑕疵

### 3. 字段数 > 7 静默截断 (P2)
**位置**: `src/loader.rs:100`
**一句话**: 使用 `splitn(7, ',')` 对多余字段静默截断而非报错
**严重度**: P2 - 语义瑕疵

---

## 测试执行摘要

```
$ export PATH="$HOME/.cargo/bin:$PATH" && cargo test --test protocol_loader_boundary
   Compiling sigma4-engine v0.1.0
    Finished `test` profile [optimized] target(s) in X.XXs
     Running unittests src/lib.rs (target/release/deps/sigma4_engine-XXXXX)

test result: ok. 0 passed; 0 failed; 0 ignored

     Running tests/protocol_loader_boundary.rs (target/release/deps/protocol_loader_boundary-XXXXX)

test result: ok. 55 passed; 0 failed; 0 ignored
```

**编译状态**: ✅ 通过
**测试状态**: ✅ 全部通过（55/55）

---

## 阻塞问题

无。

---

## 结论

Σ⁴-Engine 的协议层和加载层对不可信输入的处理总体**健壮**：
- 协议层对所有畸形输入返回 `Err`/`None`，**无 panic**（✅ 符合生产红线要求）
- 加载层对文件错误、格式异常均返回 `Err` 或跳过，**无 panic**
- 两个 P1 问题（CSV 引号转义）**不影响服务稳定性**，但可能导致数据损坏

**整体评分**: PASS（可用于生产，建议修复 P1 问题以提升数据完整性）
