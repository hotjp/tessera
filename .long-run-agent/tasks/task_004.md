# task_004

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_004.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T3] SimplexCodec 无损编解码 (SPEC §7.1)


## 需求 (requirements)

SimplexCodec::encode(M, slice_dims) -> Vec<u8>: 每行存前 k-1 个 f32(f32::to_le_bytes);编码前断言行和=1(容差 1e-5)否则 panic。decode(data, slice_dims, K_max) -> [[f32;K_MAX];MAX_SLICES]: 前 k-1 列从字节读回,末列=1-Σ前k-1,padding 列(i>=k)显式置 0。100% 可逆。



## 验收标准 (acceptance)


- cargo test codec 通过

- 编解码往返逐元素误差<1e-6

- 解码后 padding 列=0

- 退化维度含0权重正确编码

- 行和非1时 encode panic




## 交付物 (deliverables)

- `src/simplex.rs`(追加 SimplexCodec::encode/decode + 测试)



## 设计方案 (design)

src/simplex.rs(追加 impl)。f32 to_le_bytes/from_le_bytes。decode 末尾 for i in k..K_max 显式置 0。


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