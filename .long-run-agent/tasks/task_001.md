# task_001

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_001.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T0] 项目脚手架 + 基线提交(回滚锚点)


## 需求 (requirements)

本仓库 main 当前无任何 commit。本任务:1) 在仓库根创建 Rust lib crate sigma4-engine(edition 2021);2) Cargo.toml 配 dev-dep criterion;3) 添加 .gitignore(/target 等);4) 将全部 docs/ 与脚手架作为 git 首个 commit 提交。此 commit 是后续所有任务独立回滚的锚点。仅脚手架,不写任何业务代码。



## 验收标准 (acceptance)


- cargo build 成功

- crate 名 sigma4-engine

- git 已有首个 commit 且含 docs

- 无非预期未跟踪文件




## 交付物 (deliverables)

- `Cargo.toml`
- `.gitignore`
- `rust-toolchain.toml`(钉 nightly,见 DECISIONS.md ADR-001)
- `src/lib.rs`
- git 首个 commit(基线锚点,含 docs/)



## 设计方案 (design)

cargo init --lib(根目录)或 cargo new --lib 后合并;Cargo.toml name=sigma4-engine edition=2021;.gitignore 含 /target;git add . && git commit -m 'chore: scaffold sigma4-engine + baseline docs (T0)'。不引入业务依赖。


## 验证证据（完成前必填）

<!-- 标记完成前，请提供以下证据： -->

- [x] **实现证明**: cargo init 等价手建——Cargo.toml(name=sigma4-engine, edition=2021, dev-dep criterion) + .gitignore + src/lib.rs(仅入口文档) + rust-toolchain.toml(钉 nightly-2026-07-11 + rust-src/rustfmt/clippy，ADR-001)。安装 rustup 后 build 通过。
- [x] **测试验证**: `cargo build` → `Finished dev profile ... in 6.14s`；crate 名 `sigma4-engine`；`git log` → 首个 commit `64d20ee` 含 docs/；`git status` 干净无未跟踪文件。
- [x] **影响范围**: 仅脚手架，无业务代码，不阻塞后续任务（task_002+ 依赖本基线 commit 作回滚锚点）。

### 测试步骤
1. `cargo build` 成功编译空 lib crate（工具链文件触发 rustup 安装 nightly-2026-07-11）
2. `git log --oneline -1` 显示 `64d20ee chore: scaffold sigma4-engine + baseline docs (task_001, T0)`
3. `git status --short` 工作树干净

### 验证结果
- `cargo build`: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 6.14s`
- LRA 质量检查: 得分 100/100（迭代 0/7，提前完成）
- 基线 commit: `64d20ee`（含 docs/ + LRA 任务库 + 脚手架；.DS_Store/.claude//target/LRA运行时 已 gitignore）