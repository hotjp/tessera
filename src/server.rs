//! TCP + TLS 1.3 服务端（SPEC §4.2 / §5）。
//!
//! tokio + rustls；ALPN=`sigma4`（拒绝非匹配连接）；每连接独立 tokio 任务；
//! Length-Prefix Framing。**严禁** HTTP/2 / gRPC / WebSocket。
//! 路径用 [`std::path::PathBuf`]（ADR-002 跨平台）。
//!
//! [`process_frame`] 为纯逻辑（不触网络），便于端到端测试；[`serve`] 为
//! tokio/rustls 网络层封装。

use crate::cascade::{cascade, EntityStateView};
use crate::entity::{Entity, EntitySnapshot};
use crate::matrix::CascadeMatrix;
use crate::protocol::{
    decode_cascade_run, decode_state_update, encode_cascade_results, Frame, FrameError, FrameType,
    MAX_FRAME_LEN,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// ALPN 协议标识。
pub const ALPN_SIGMA4: &[u8] = b"sigma4";

/// 引擎状态：实体池（StateUpdate/Query）+ 级联矩阵与状态视图（CascadeRun）。
///
/// Entity Pool 经 `Arc<Mutex<Engine>>` 跨连接共享（StateUpdate 写、Query/CascadeRun 读）。
pub struct Engine {
    /// 实体池（按 entity_id 索引）。
    pub entities: Vec<Entity>,
    /// 级联矩阵（CascadeRun）。
    pub matrix: CascadeMatrix,
    /// 每实体级联状态视图。
    pub states: Vec<EntityStateView>,
    /// 快照目录（PathBuf，跨平台）。
    pub snapshot_dir: PathBuf,
}

impl Engine {
    /// 构造引擎。
    pub fn new(
        entities: Vec<Entity>,
        matrix: CascadeMatrix,
        states: Vec<EntityStateView>,
        snapshot_dir: PathBuf,
    ) -> Self {
        Self {
            entities,
            matrix,
            states,
            snapshot_dir,
        }
    }
}

/// 处理一帧，返回响应帧（纯逻辑，不走网络 = 端到端逻辑核心）。
///
/// - StateUpdate → 解析 body → apply delta → Heartbeat(ACK)
/// - StateQuery → 查询 → StateResponse
/// - CascadeRun → 执行 cascade → CascadeResult
/// - SnapshotReq → 返回快照路径（PathBuf）
/// - Heartbeat → 回 ACK
pub fn process_frame(engine: &mut Engine, frame: &Frame) -> Frame {
    match frame.frame_type {
        FrameType::StateUpdate => {
            if let Some((id, ev)) = decode_state_update(&frame.body) {
                if (id as usize) < engine.entities.len() {
                    engine.entities[id as usize].apply_delta_singlethreaded(ev);
                }
            }
            Frame::new(FrameType::Heartbeat, Vec::new())
        }
        FrameType::StateQuery => {
            let id = frame
                .body
                .get(..4)
                .and_then(|s| s.try_into().ok())
                .map(u32::from_be_bytes)
                .unwrap_or(u32::MAX);
            let body = if (id as usize) < engine.entities.len() {
                let snap = engine.entities[id as usize].query_state(u64::MAX);
                encode_state_response(id, &snap)
            } else {
                Vec::new()
            };
            Frame::new(FrameType::StateResponse, body)
        }
        FrameType::CascadeRun => {
            let body = if let Some((theta, max_hops, initial)) = decode_cascade_run(&frame.body) {
                let n = engine.matrix.n as usize;
                let mut v = vec![0.0f32; n];
                for (id, val) in initial {
                    if (id as usize) < n {
                        v[id as usize] = val;
                    }
                }
                let results = cascade(&v, &engine.matrix, &engine.states, max_hops as u32, theta);
                encode_cascade_results(&results)
            } else {
                Vec::new()
            };
            Frame::new(FrameType::CascadeResult, body)
        }
        FrameType::SnapshotReq => {
            let path = engine.snapshot_dir.join("snapshot.bin");
            Frame::new(
                FrameType::SnapshotResp,
                path.to_string_lossy().into_owned().into_bytes(),
            )
        }
        FrameType::Heartbeat => Frame::new(FrameType::Heartbeat, Vec::new()),
        _ => Frame::new(FrameType::Heartbeat, Vec::new()),
    }
}

/// StateResponse body：`entity_id u32 | num_slices u8 | slice_dims 16B | coords 512B`。
fn encode_state_response(id: u32, snap: &EntitySnapshot) -> Vec<u8> {
    let mut b = Vec::with_capacity(4 + 1 + 16 + 512);
    b.extend_from_slice(&id.to_be_bytes());
    b.push(snap.num_slices);
    b.extend_from_slice(&snap.slice_dims);
    for row in &snap.coords {
        for &x in row {
            b.extend_from_slice(&x.to_be_bytes());
        }
    }
    b
}

/// 读取一帧（length-prefix）；`Ok(None)` 表示连接正常结束。
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Option<Frame>, FrameError> {
    let mut len_buf = [0u8; 4];
    if r.read_exact(&mut len_buf).await.is_err() {
        return Ok(None);
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(FrameError::TooLarge { len });
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)
        .await
        .map_err(|_| FrameError::UnexpectedEof)?;
    let mut full = len_buf.to_vec();
    full.extend_from_slice(&payload);
    Frame::decode(&full).map(Some)
}

/// 写入一帧。
pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, frame: &Frame) -> std::io::Result<()> {
    w.write_all(&frame.encode()).await
}

/// 处理单个连接：循环读帧 → [`process_frame`] → 写响应。
async fn handle_connection<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    engine: Arc<Mutex<Engine>>,
) {
    let (mut r, mut w) = tokio::io::split(stream);
    while let Ok(Some(frame)) = read_frame(&mut r).await {
        let resp = {
            let mut eng = engine.lock().expect("engine mutex poisoned");
            process_frame(&mut eng, &frame)
        };
        if write_frame(&mut w, &resp).await.is_err() {
            break;
        }
    }
}

/// TLS 服务端主循环：accept → ALPN 校验（拒绝非 `sigma4`）→ 每连接 spawn 任务。
///
/// `acceptor` 由调用方提供（含证书 + ALPN=`sigma4`）。Entity Pool 经
/// `Arc<Mutex<Engine>>` 只读/写共享。
pub async fn serve(
    listener: TcpListener,
    engine: Arc<Mutex<Engine>>,
    acceptor: TlsAcceptor,
) -> std::io::Result<()> {
    loop {
        let (tcp, _addr) = listener.accept().await?;
        let engine = engine.clone();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            let tls = match acceptor.accept(tcp).await {
                Ok(t) => t,
                Err(_) => return,
            };
            // ALPN 校验：拒绝非 sigma4 连接
            if tls.get_ref().1.alpn_protocol() != Some(ALPN_SIGMA4) {
                return;
            }
            handle_connection(tls, engine).await;
        });
    }
}

#[cfg(test)]
mod server_tests {
    use super::*;
    use crate::protocol::{encode_cascade_run, encode_state_update, FrameType};

    /// 构造测试引擎：2 实体，matrix 中心0→叶1，非脆性。
    fn test_engine() -> Engine {
        let mut e0 = Entity::new(0, 0, 1);
        e0.slice_dims[0] = 2;
        e0.coordinates[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let e1 = Entity::new(1, 0, 0);
        // matrix：spmv 下 from=叶,to=中心 表示 中心→叶（同 task_009）
        let matrix = CascadeMatrix::from_edges(2, &[(1, 0, 0.5, 0)]);
        let states = vec![
            EntityStateView {
                coordinates: 0.0,
                brittle_threshold: 0.0,
                decay_coefficient: 1.0,
                time_lag_us: 10,
            },
            EntityStateView {
                coordinates: 0.0,
                brittle_threshold: 0.0,
                decay_coefficient: 1.0,
                time_lag_us: 10,
            },
        ];
        Engine::new(vec![e0, e1], matrix, states, PathBuf::from("/tmp/sigma4"))
    }

    #[test]
    fn state_update_end_to_end_applies_delta() {
        let mut eng = test_engine();
        // 构造 StateUpdate 帧：实体 0，delta (ts=100, mask=1, ep=0, +0.0)
        let ev = crate::entity::DeltaEvent::new(100, 1, 0, 0.0);
        let body = encode_state_update(0, &ev);
        let frame = Frame::new(FrameType::StateUpdate, body);
        let resp = process_frame(&mut eng, &frame);
        assert_eq!(resp.frame_type, FrameType::Heartbeat); // ACK
                                                           // 验证 delta 已入环（ring_head 推进到 1）
        assert_eq!(eng.entities[0].ring_head, 1);
    }

    #[test]
    fn cascade_run_end_to_end_returns_results() {
        let mut eng = test_engine();
        // 初始冲击在实体 0
        let req = encode_cascade_run(0.1, 2, &[(0, 1.0)]);
        let frame = Frame::new(FrameType::CascadeRun, req);
        let resp = process_frame(&mut eng, &frame);
        assert_eq!(resp.frame_type, FrameType::CascadeResult);
        let results = crate::protocol::decode_cascade_results(&resp.body).unwrap();
        // 实体 0 hop0 conf=1.0；实体 1 hop1 conf=0.5
        let r1 = results.iter().find(|r| r.entity_id == 1).unwrap();
        assert_eq!(r1.hop, 1);
        assert!((r1.confidence - 0.5).abs() < 1e-5, "got {}", r1.confidence);
    }

    #[test]
    fn snapshot_req_uses_pathbuf() {
        let mut eng = test_engine();
        let resp = process_frame(&mut eng, &Frame::new(FrameType::SnapshotReq, Vec::new()));
        assert_eq!(resp.frame_type, FrameType::SnapshotResp);
        let path = String::from_utf8(resp.body).unwrap();
        assert!(path.contains("snapshot.bin"), "path={path}");
    }

    #[test]
    fn heartbeat_acked() {
        let mut eng = test_engine();
        let resp = process_frame(&mut eng, &Frame::new(FrameType::Heartbeat, Vec::new()));
        assert_eq!(resp.frame_type, FrameType::Heartbeat);
    }

    #[tokio::test]
    async fn read_write_frame_round_trip_in_memory() {
        // 不走真实 socket：用 duplex 验证 read_frame/write_frame 往返
        let f = Frame::new(FrameType::StateQuery, vec![0xAA, 0xBB]);
        let (mut client, mut srv) = tokio::io::duplex(1024);
        write_frame(&mut client, &f).await.unwrap();
        let f2 = read_frame(&mut srv).await.unwrap().unwrap();
        assert_eq!(f2, f);
    }
}
