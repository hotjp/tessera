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

- [x] **实现证明**: 新建 src/loader.rs。手写 CSV 解析(splitn(7,',')，零外部依赖；数据集 183 行 × 7 字段、无引号/内嵌逗号，已验证)。分类 A-E→码 0-4；确定性先验 facet_prior(cat,facet) 用 (cat,facet,endpoint) 可审计公式生成正权重归一化到 4 端点单纯形(非 LLM)，4 切面(权力/动态/认知/级联)，padding 列 fill(0)。中国主体按组织名关键词(中国/央企/中投/兵器/航天...)检测→ownership_type=1。EntityPool 持有 names+entities，name_ptr 指向池内名字字节。load_from_file 用 Path(跨平台)。
- [x] **测试验证**: `cargo test loader` → 8 passed（count==183/每切面和=1/padding=0/非负/A-E全覆盖/中国主体标记/分类先验互异/name_ptr有效）；全套 67 passed；clippy -D warnings 无告警；fmt 通过。
- [x] **影响范围**: 新增数据加载器（管理面，非热路径）；entity.rs 加 SteadyState::new 构造器（避免 _pad 私有字段跨模块构造）。加载的 EntityPool 可喂入 server.Engine。

### 测试步骤
1. `cargo test loader` → 8/8 ok
2. `cargo clippy --all-targets -- -D warnings` → 无告警
3. `cargo fmt --check` → exit 0

### 验证结果
- entity_count == 183 ✅
- 每实体每切面(num_slices=4)前 k=4 项和 = 1（容差 1e-5）✅
- padding 列(4..8)= 0 ✅；全权重 ≥ 0 ✅
- 分类 A/B/C/D/E 全覆盖（62/12/22/84/3）✅
- 中国主体(中金/中投/兵器/航天/航空/五矿等) ownership_type=1 ✅
- 不同分类先验互异 ✅；name_ptr 指向池内名字字节(unsafe 读取一致) ✅