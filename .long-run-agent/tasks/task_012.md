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

- [ ] **实现证明**: 简要说明如何实现
- [ ] **测试验证**: 如何验证功能正常（测试步骤/截图/命令输出）
- [ ] **影响范围**: 是否影响其他功能

### 测试步骤
1. 
2. 
3. 

### 验证结果
<!-- 粘贴验证截图、命令输出或测试结果 -->