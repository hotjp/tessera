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

- [ ] **实现证明**: 简要说明如何实现
- [ ] **测试验证**: 如何验证功能正常（测试步骤/截图/命令输出）
- [ ] **影响范围**: 是否影响其他功能

### 测试步骤
1. 
2. 
3. 

### 验证结果
<!-- 粘贴验证截图、命令输出或测试结果 -->