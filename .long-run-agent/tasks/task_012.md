# task_012

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_012.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T11] 跨平台 CI 矩阵 + 架构决策记录(DECISIONS.md)


## 需求 (requirements)

1) 创建 docs/DECISIONS.md,ADR 风格(背景/决策/后果)记录两条架构决策:(a) SIMD 采用 nightly std::simd(portable_simd),明确声明偏离 SPEC §6.2 与附录B决策树(规范警告勿用 std::simd),理由=4 平台单一可移植代码路径;(b) 目标平台矩阵= aarch64-macOS(开发机)/ x86_64-Linux(服务器)/ aarch64-Linux(Graviton)/ Windows。2) GitHub Actions workflow:上述 4 平台矩阵跑 cargo test,通过 rust-toolchain.toml(task_001)安装固定 nightly,验证 portable_simd 在每平台编译通过;确认代码无 x86_64 专属 intrinsic 残留。3) Windows 路径可移植性回归测试。



## 验收标准 (acceptance)


- DECISIONS.md 存在并记录两条决策

- CI 4 平台矩阵 cargo test 全绿

- portable_simd 在 4 平台均编译通过

- 代码无 x86_64 专属 intrinsic 残留




## 交付物 (deliverables)

- `docs/DECISIONS.md`(已存在,本任务验证/补全)
- `.github/workflows/ci.yml`(4 平台矩阵)



## 设计方案 (design)

.github/workflows/ci.yml 用 matrix 覆盖 ubuntu-latest(x86_64)+ aarch64、macos-latest(aarch64)、windows-latest;nightly 由 rust-toolchain.toml 统一;DECISIONS.md 置于 docs/ 与其他规范并列,供未来 AI/审阅者查证偏离依据。


## 验证证据（完成前必填）

<!-- 标记完成前，请提供以下证据： -->

- [x] **实现证明**: (1) docs/DECISIONS.md 已存在(baseline commit 64d20ee，3397B)，含 ADR-001(SIMD=nightly std::simd，明确偏离 SPEC §6.2/附录B) + ADR-002(4 平台矩阵)，本任务验证齐全。(2) 新增 .github/workflows/ci.yml：test-matrix 覆盖 ubuntu-latest(x86_64)/ubuntu-24.04-arm(aarch64 Graviton)/macos-latest(Apple Silicon)/windows-latest，rust-toolchain.toml 自动安装固定 nightly，每平台 cargo build+cargo test；lint job 跑 fmt+clippy(-D warnings)+ x86 intrinsic 守卫(grep 真实用法)。(3) 新增 Windows 路径可移植性回归测试(snapshot_path_portable_across_platforms)，test_engine 路径改相对 "snapshots"。
- [x] **测试验证**: ci.yml 经 python yaml 校验合法；本地 `cargo test` 59 passed、`cargo clippy --all-targets -- -D warnings` 无告警、`cargo fmt --check` 通过；`grep -rnE "use std::arch::x86_64|is_x86_feature_detected!\(|_mm[0-9]*_" src/` 无匹配（无 x86 intrinsic）。
- [x] **影响范围**: 新增 CI 配置与一个回归测试，不改业务代码（仅 test_engine 路径字符串调整）。实际 4 平台 CI 执行在 push 到 GitHub 后由 Actions 触发。

### 测试步骤
1. `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` → 合法
2. `cargo test` → 59 passed（含 snapshot_path_portable_across_platforms）
3. `cargo clippy --all-targets -- -D warnings` → 无告警（与 CI lint job 一致）
4. `grep -rnE "use std::arch::x86_64|is_x86_feature_detected!\(|_mm[0-9]*_" src/` → 无匹配

### 验证结果
- DECISIONS.md: ADR-001 + ADR-002 齐全（docs/ 下，3397B）
- CI 矩阵: 4 平台（x86_64 Linux / aarch64 Linux Graviton / aarch64 macOS / Windows）
- portable_simd: 本地 aarch64 编译通过（matrix "编译" step 验证各平台）
- 无 x86 intrinsic: refined grep 仅匹配真实用法，src/ 无残留
- Windows 路径回归: PathBuf::join 的 file_name 在各平台均为 "snapshot.bin"
- ⚠️ 注：GitHub Actions 实际 4 平台执行需 push 到仓库触发；本地已验证 workflow 语法、测试、lint、守卫全绿