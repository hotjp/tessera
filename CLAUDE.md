# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Project**: global-capital-players
**System**: Σ⁴-System / Σ⁴-Engine（Sigma-4，微秒级稀疏状态级联引擎）
**Description**: 确定性多实体级联依赖推理引擎。以「全球资本全景表」(`docs/global_capital_players_full_index.csv`，183 个资本主体：宏观基金、主权基金、家族网络、央企集团等，分 A/B/C/D/E 五类，含中国主体独立标记) 为数据基座，将实体建模为**多切面单纯形坐标**（权力拓扑 / 动态模式 / 认知可达 / 级联响应），通过**稀疏事件驱动**状态更新与 **SIMD 加速的稀疏矩阵级联传播**，在固定内存预算内完成 μs 级确定性推理。所有推理链为**确定性白盒推导**，可审计、可回测，拒绝 LLM 黑盒提取。

**当前阶段**: 设计 / 规范阶段。仓库目前**仅含架构设计文档与基线数据 CSV，尚无实现代码**。Σ⁴-Engine（Rust 核心引擎）的 PoC 实现是下一步工作。

## Architecture

技术栈与核心设计（完整细节见 [docs/](docs/)，**以最新版本为准**）：

- **热路径**：Rust（内存安全、零成本抽象、AVX-512 / SVE SIMD）
  - 内存常驻 Entity Pool（每实体 ~16KB，预分配 ≤65,536 槽位；`MAX_SLICES=16`, `K_MAX=8`）
  - 三层状态表示：基态（O(1) 内存直读）→ 增量环 Delta Ring（无锁 CAS 写入）→ SSD 顺序快照日志（Delta-of-Delta 编码，只追加）
  - CSR 稀疏矩阵 × 向量级联传播（Lazy Cascade + 子图剪枝，5 跳）
  - Duchi 单纯形投影 / 帕累托约束求解
  - 切面运行时热加载（预分配槽位，无停机扩展）
- **管理面**：Python（切面注册、配置管理、约束配置、可视化）
- **传输**：HTTP/2 自定义二进制帧（magic `0xCAFE`，状态更新 ~26 bytes，非 JSON）
- **性能目标**：状态写入 < 1μs；状态查询 < 1μs；级联推理(100 实体/5 跳) < 100μs；快照恢复 < 500ms

### 文档导航

架构历经多版本演进，**以 SIGMA4 规范与 v4 为当前权威**，早期版本仅作归档参考：

| 文档 | 说明 | 状态 |
|------|------|------|
| [SIGMA4_SPEC_v1.md](docs/SIGMA4_SPEC_v1.md) | 自包含技术规范（数学模型 / 协议 / 算法 / 部署） | ✅ 当前权威 |
| [ARCHITECTURE_v4.md](docs/ARCHITECTURE_v4.md) | v4.0 内存计算架构重写（认知修正与设计动机） | ✅ 当前 |
| [ARCHITECTURE_v3.md](docs/ARCHITECTURE_v3.md) / [v3_1](docs/ARCHITECTURE_v3_1.md) / [v2](docs/ARCHITECTURE_v2.md) | 演进中间版（数据库思路，已被 v4 否定） | 📦 归档 |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | v1 初稿（SCS-GlobalCapital，Python + PostgreSQL/Neo4j 图数据库） | 📦 归档 |
| [global_capital_players_full_index.csv](docs/global_capital_players_full_index.csv) | 基线数据：183 实体 × 7 字段 × 5 分类 | 📊 数据基座 |

## Task Management

This project uses **LRA** for task tracking.
See [lra.md](lra.md) for command reference.

## Quick Start

```bash
lra ready              # Find available work
lra show <id>          # View task details
```

<!-- BEGIN LRA CLAUDE SECTION -->

## LRA Task Management

This project uses **LRA** profile: **full**

- Detailed guide: [lra.md](lra.md)
- Use `lra` for all task management
- Run `lra ready` before starting work
- ❌ Do not use markdown TODO lists

<!-- END LRA CLAUDE SECTION -->
