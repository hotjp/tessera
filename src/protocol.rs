//! 自定义二进制帧协议（SPEC §4.2 / §5）。
//!
//! Length-Prefix Framing：`[4B len BE][version 1B][frame_type 1B][body]`。
//! `len = 2 + body.len()`（version + type + body）。`len > MAX_FRAME_LEN(65536)` 拒绝。
//! 线序大端（ADR-002）。**非** HTTP/2 / gRPC / WebSocket。

use crate::cascade::CascadeResult;
use crate::entity::DeltaEvent;

/// 协议版本。
pub const FRAME_VERSION: u8 = 1;
/// 最大帧 payload 长度（SPEC §5：超限拒绝）。
pub const MAX_FRAME_LEN: usize = 65_536;

/// 帧类型（SPEC §5）。
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameType {
    StateUpdate = 0x01,
    StateQuery = 0x02,
    StateResponse = 0x03,
    CascadeRun = 0x04,
    CascadeResult = 0x05,
    SnapshotReq = 0x06,
    SnapshotResp = 0x07,
    Heartbeat = 0xFF,
}

impl FrameType {
    /// 从单字节还原帧类型。
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0x01 => Self::StateUpdate,
            0x02 => Self::StateQuery,
            0x03 => Self::StateResponse,
            0x04 => Self::CascadeRun,
            0x05 => Self::CascadeResult,
            0x06 => Self::SnapshotReq,
            0x07 => Self::SnapshotResp,
            0xFF => Self::Heartbeat,
            _ => return None,
        })
    }
}

/// 帧解析错误。
#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    /// payload 超过 `MAX_FRAME_LEN`。
    TooLarge { len: usize },
    /// 字节不足。
    UnexpectedEof,
    /// 未知帧类型。
    UnknownFrameType(u8),
    /// 版本不匹配。
    BadVersion(u8),
}

/// 一个完整帧。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// 协议版本。
    pub version: u8,
    /// 帧类型。
    pub frame_type: FrameType,
    /// payload（线序大端编码的业务字段）。
    pub body: Vec<u8>,
}

impl Frame {
    /// 构造默认版本的帧。
    pub fn new(ft: FrameType, body: Vec<u8>) -> Self {
        Self {
            version: FRAME_VERSION,
            frame_type: ft,
            body,
        }
    }

    /// 编码为 `[4B len BE][version][frame_type][body]`。
    pub fn encode(&self) -> Vec<u8> {
        let payload_len = 2 + self.body.len();
        let mut out = Vec::with_capacity(4 + payload_len);
        out.extend_from_slice(&(payload_len as u32).to_be_bytes());
        out.push(self.version);
        out.push(self.frame_type as u8);
        out.extend_from_slice(&self.body);
        out
    }

    /// 从缓冲区解码（含 4B len 前缀）。`len > MAX_FRAME_LEN` 拒绝。
    pub fn decode(buf: &[u8]) -> Result<Self, FrameError> {
        if buf.len() < 4 {
            return Err(FrameError::UnexpectedEof);
        }
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        if len > MAX_FRAME_LEN {
            return Err(FrameError::TooLarge { len });
        }
        if buf.len() < 4 + len {
            return Err(FrameError::UnexpectedEof);
        }
        let payload = &buf[4..4 + len];
        let version = payload[0];
        if version != FRAME_VERSION {
            return Err(FrameError::BadVersion(version));
        }
        let ft = FrameType::from_u8(payload[1]).ok_or(FrameError::UnknownFrameType(payload[1]))?;
        Ok(Self {
            version,
            frame_type: ft,
            body: payload[2..].to_vec(),
        })
    }
}

// ---- Body 编解码（线序大端，ADR-002）----

/// StateUpdate body：`entity_id u32 | timestamp u64 | slice_mask u16 | endpoint u8 | delta f32`（19B）。
pub fn encode_state_update(entity_id: u32, ev: &DeltaEvent) -> Vec<u8> {
    let mut b = Vec::with_capacity(19);
    b.extend_from_slice(&entity_id.to_be_bytes());
    b.extend_from_slice(&ev.timestamp_us.to_be_bytes());
    b.extend_from_slice(&ev.slice_mask.to_be_bytes());
    b.push(ev.endpoint_idx);
    b.extend_from_slice(&ev.delta_value.to_be_bytes());
    b
}

/// 解码 StateUpdate body。
pub fn decode_state_update(body: &[u8]) -> Option<(u32, DeltaEvent)> {
    if body.len() < 19 {
        return None;
    }
    let entity_id = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    let timestamp_us = u64::from_be_bytes(body[4..12].try_into().ok()?);
    let slice_mask = u16::from_be_bytes([body[12], body[13]]);
    let endpoint_idx = body[14];
    let delta_value = f32::from_be_bytes(body[15..19].try_into().ok()?);
    Some((
        entity_id,
        DeltaEvent::new(timestamp_us, slice_mask, endpoint_idx, delta_value),
    ))
}

/// CascadeRun 请求参数：`(theta, max_hops, 初始冲击 [(entity_id, value)])`。
pub type CascadeRunRequest = (f32, u8, Vec<(u32, f32)>);

/// CascadeRun 请求 body：`theta f32 | max_hops u8 | count u16 | count×(entity_id u32 | value f32)`。
pub fn encode_cascade_run(theta: f32, max_hops: u8, initial: &[(u32, f32)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(4 + 1 + 2 + initial.len() * 8);
    b.extend_from_slice(&theta.to_be_bytes());
    b.push(max_hops);
    b.extend_from_slice(&(initial.len() as u16).to_be_bytes());
    for &(id, v) in initial {
        b.extend_from_slice(&id.to_be_bytes());
        b.extend_from_slice(&v.to_be_bytes());
    }
    b
}

/// 解码 CascadeRun 请求 body。
pub fn decode_cascade_run(body: &[u8]) -> Option<CascadeRunRequest> {
    if body.len() < 7 {
        return None;
    }
    let theta = f32::from_be_bytes(body[0..4].try_into().ok()?);
    let max_hops = body[4];
    let count = u16::from_be_bytes([body[5], body[6]]) as usize;
    let mut initial = Vec::with_capacity(count);
    let mut p = 7;
    for _ in 0..count {
        if p + 8 > body.len() {
            return None;
        }
        let id = u32::from_be_bytes(body[p..p + 4].try_into().ok()?);
        let v = f32::from_be_bytes(body[p + 4..p + 8].try_into().ok()?);
        initial.push((id, v));
        p += 8;
    }
    Some((theta, max_hops, initial))
}

/// CascadeResult 响应 body：`count u16 | count×(entity_id u32 | confidence f32 | hop u8 | lag u32)`。
pub fn encode_cascade_results(results: &[CascadeResult]) -> Vec<u8> {
    let mut b = Vec::with_capacity(2 + results.len() * 13);
    b.extend_from_slice(&(results.len() as u16).to_be_bytes());
    for r in results {
        b.extend_from_slice(&r.entity_id.to_be_bytes());
        b.extend_from_slice(&r.confidence.to_be_bytes());
        b.push(r.hop as u8);
        b.extend_from_slice(&r.lag_us.to_be_bytes());
    }
    b
}

/// 解码 CascadeResult 响应 body。
pub fn decode_cascade_results(body: &[u8]) -> Option<Vec<CascadeResult>> {
    if body.len() < 2 {
        return None;
    }
    let count = u16::from_be_bytes([body[0], body[1]]) as usize;
    let mut out = Vec::with_capacity(count);
    let mut p = 2;
    for _ in 0..count {
        if p + 13 > body.len() {
            return None;
        }
        let entity_id = u32::from_be_bytes(body[p..p + 4].try_into().ok()?);
        let confidence = f32::from_be_bytes(body[p + 4..p + 8].try_into().ok()?);
        let hop = body[p + 8] as u32;
        let lag_us = u32::from_be_bytes(body[p + 9..p + 13].try_into().ok()?);
        out.push(CascadeResult {
            entity_id,
            confidence,
            hop,
            lag_us,
        });
        p += 13;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_encode_decode_round_trip() {
        let f = Frame::new(FrameType::StateUpdate, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let enc = f.encode();
        let dec = Frame::decode(&enc).unwrap();
        assert_eq!(dec, f);
    }

    #[test]
    fn frame_all_types_round_trip() {
        for ft in [
            FrameType::StateUpdate,
            FrameType::StateQuery,
            FrameType::StateResponse,
            FrameType::CascadeRun,
            FrameType::CascadeResult,
            FrameType::SnapshotReq,
            FrameType::SnapshotResp,
            FrameType::Heartbeat,
        ] {
            let f = Frame::new(ft, vec![1, 2, 3]);
            assert_eq!(Frame::decode(&f.encode()).unwrap(), f);
        }
    }

    #[test]
    fn oversized_frame_rejected() {
        // 手工构造 len=70000 的帧（>65536）
        let mut buf = (70_000u32).to_be_bytes().to_vec();
        buf.extend_from_slice(&[0u8; 8]);
        assert_eq!(
            Frame::decode(&buf),
            Err(FrameError::TooLarge { len: 70_000 })
        );
    }

    #[test]
    fn truncated_frame_rejected() {
        let f = Frame::new(FrameType::Heartbeat, vec![0; 10]);
        let mut enc = f.encode();
        enc.truncate(enc.len() - 5); // 截断
        assert_eq!(Frame::decode(&enc), Err(FrameError::UnexpectedEof));
    }

    #[test]
    fn unknown_frame_type_rejected() {
        // 构造合法 len 但 frame_type 字节非法
        let payload = [FRAME_VERSION, 0xAB];
        let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
        buf.extend_from_slice(&payload);
        assert_eq!(Frame::decode(&buf), Err(FrameError::UnknownFrameType(0xAB)));
    }

    #[test]
    fn state_update_body_round_trip() {
        let ev = DeltaEvent::new(0x1122334455, 0x0102, 7, 1.5);
        let body = encode_state_update(42, &ev);
        let (id, ev2) = decode_state_update(&body).unwrap();
        assert_eq!(id, 42);
        // packed 字段按值拷入元组再比较（避免对 packed 字段取引用）
        let got = (ev2.timestamp_us, ev2.slice_mask, ev2.endpoint_idx);
        let want = (ev.timestamp_us, ev.slice_mask, ev.endpoint_idx);
        assert_eq!(got, want);
        assert!((ev2.delta_value - 1.5).abs() < 1e-6);
    }

    #[test]
    fn cascade_run_body_round_trip() {
        let initial = vec![(0u32, 1.0f32), (3, 0.5), (5, 0.25)];
        let body = encode_cascade_run(0.1, 5, &initial);
        let (theta, hops, init2) = decode_cascade_run(&body).unwrap();
        assert!((theta - 0.1).abs() < 1e-6);
        assert_eq!(hops, 5);
        assert_eq!(init2, initial);
    }

    #[test]
    fn cascade_results_body_round_trip() {
        let results = vec![
            CascadeResult {
                entity_id: 1,
                confidence: 0.8,
                hop: 2,
                lag_us: 30,
            },
            CascadeResult {
                entity_id: 9,
                confidence: 1.0,
                hop: 1,
                lag_us: 10,
            },
        ];
        let body = encode_cascade_results(&results);
        let dec = decode_cascade_results(&body).unwrap();
        assert_eq!(dec.len(), results.len());
        for (a, b) in dec.iter().zip(results.iter()) {
            assert_eq!(a.entity_id, b.entity_id);
            assert!((a.confidence - b.confidence).abs() < 1e-6);
            assert_eq!(a.hop, b.hop);
            assert_eq!(a.lag_us, b.lag_us);
        }
    }
}
