# task_011

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_011.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T10] CSV 加载器: 183 实体基线数据 (docs/global_capital_players_full_index.csv)


## 需求 (requirements)

src/loader.rs: 解析 183 行 CSV(字段:组织/负责人/观点/阵地/获利/追踪渠道/分类 A-E),映射为 Entity Pool;按分类推导各切面(power/dynamics/epistemic/cascade)初始单纯形坐标(分类->确定性先验权重表,非 LLM),填 slice_dims/num_slices/steady_state;padding 列置 0;中国主体独立标记(ownership_type)。



## 验收标准 (acceptance)


- cargo test loader 通过

- 加载 entity_count==183

- 每实体每切面前k项和=1容差1e-5

- padding=0

- 分类 A-E 全覆盖




## 交付物 (deliverables)

- `src/loader.rs`(CSV→Entity Pool + 测试)
- 数据源: `docs/global_capital_players_full_index.csv`



## 设计方案 (design)

src/loader.rs。倾向零外部依赖手写 CSV 解析(字段含中文逗号,需正确处理引号),或引入 csv crate。坐标推导为确定性规则(分类->权重表),保证可审计可回测。


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