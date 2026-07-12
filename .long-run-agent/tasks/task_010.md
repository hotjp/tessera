# task_010

## ⚠️ 重要提示（Agent 必读）

**当前位置**: `.long-run-agent/tasks/task_010.md`（任务描述文件）

**工作目录**: 项目根目录（`.long-run-agent` 的同级目录）

**产出物**: 请在项目根目录或适当子目录创建交付物

**这是配置文件**，不是最终产出！

## 描述

[T9] 网络层: TCP + TLS 1.3 + 自定义二进制帧 (SPEC §4.2 §5)


## 需求 (requirements)

src/protocol.rs: Frame { version u8, frame_type u8, body }, FrameType(StateUpdate=0x01/StateQuery/StateResponse/CascadeRun/CascadeResult/SnapshotReq/SnapshotResp/Heartbeat=0xFF);Frame::encode/decode;Length-Prefix Framing [4B len BE]+payload;len>65536 拒绝。src/server.rs: tokio + rustls,TLS ALPN=[sigma4] 拒绝非匹配连接,每连接独立 tokio 任务,处理 StateUpdate(解析->apply delta)/StateQuery/CascadeRun(执行->返回)/SnapshotReq/Heartbeat。严禁 HTTP/2/gRPC/WebSocket。Entity Pool 只读共享。



## 验收标准 (acceptance)


- cargo test protocol 通过

- 帧编解码往返

- 超大帧>65536 拒绝

- StateUpdate 端到端写帧解析应用delta

- CascadeRun 端到端请求执行返回

- 不走 HTTP/2 或 gRPC




## 交付物 (deliverables)

- `src/protocol.rs`(Frame/FrameType/encode/decode + 测试)
- `src/server.rs`(tokio+rustls, length-prefix, ALPN sigma4;路径用 PathBuf)



## 设计方案 (design)

src/protocol.rs + src/server.rs。rustls 0.22 + tokio-rustls。每帧 read 4B len 再 read payload。证书用自签 rcgen 生成(仅 dev)。


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