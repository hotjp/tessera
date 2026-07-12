# Agent Guide

> 本项目使用 **LRA** (Long-Running Agent) 管理任务进度。

## 项目信息

- **项目**: global-capital-players（Σ⁴-System / Σ⁴-Engine）
- **定位**: 确定性多实体级联依赖推理引擎。以全球资本全景表（183 实体，A/B/C/D/E 五类）为基座，将实体建模为多切面单纯形坐标，通过稀疏事件驱动 + SIMD 级联传播完成 μs 级白盒推理
- **技术栈**: Rust（热路径：内存常驻 Entity Pool + CSR 稀疏矩阵级联 + Duchi 单纯形投影）+ Python（管理面：切面注册/配置/可视化）；HTTP/2 自定义二进制帧
- **当前阶段**: 设计 / 规范阶段（仓库含架构文档 + 基线 CSV，尚无实现代码）
- **权威文档**: [docs/SIGMA4_SPEC_v1.md](docs/SIGMA4_SPEC_v1.md)、[docs/ARCHITECTURE_v4.md](docs/ARCHITECTURE_v4.md)
- **任务管理**: 使用 LRA
- **Constitution**: [.long-run-agent/constitution.yaml](.long-run-agent/constitution.yaml)

## 快速开始

```bash
cat lra.md              # 查看 LRA 工具使用说明
lra ready               # 查看可认领任务
lra show <id>          # 查看任务详情
```

## 外部依赖

详见: [.long-run-agent/config.json](.long-run-agent/config.json)

## 相关文档

- [lra.md](lra.md) - LRA 详细命令 ← 工具使用说明
- [CLAUDE.md](CLAUDE.md) - Claude Code 特定优化

<!-- BEGIN LRA AGENT SECTION -->

## LRA 任务管理

本项目使用 **LRA** (Long-Running Agent) 管理任务。

- 详细说明: [lra.md](lra.md)
- ❌ 不要使用 markdown TODO 列表

<!-- END LRA AGENT SECTION -->
