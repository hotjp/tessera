//! Σ⁴-Engine 边界审计测试 - A4 输入健壮性
//!
//! 探测协议帧解析与 CSV 数据加载对不可信输入的处理。
//! 目标：拒绝畸形输入应返回 Err/None，不应 panic（DoS 防护）。

use sigma4_engine::cascade::CascadeResult;
use sigma4_engine::entity::DeltaEvent;
use sigma4_engine::loader;
use sigma4_engine::protocol::{
    decode_cascade_results, decode_cascade_run, decode_state_update, encode_cascade_run,
    encode_cascade_results, encode_state_update, Frame, FrameError, FrameType, MAX_FRAME_LEN,
};

// =============================================================================
// 协议层边界测试
// =============================================================================

mod protocol_boundary {
    use super::*;

    // ---- Frame::decode 边界测试 ----

    #[test]
    fn empty_buffer_returns_err() {
        let result = Frame::decode(&[]);
        assert_eq!(result, Err(FrameError::UnexpectedEof));
    }

    #[test]
    fn buffer_shorter_than_length_prefix() {
        // 只有 1 字节，需要 4 字节 len 前缀
        let result = Frame::decode(&[0x00]);
        assert_eq!(result, Err(FrameError::UnexpectedEof));
    }

    #[test]
    fn buffer_with_len_prefix_only() {
        // 只有 4 字节 len 前缀，声明需要 7 字节 payload（version + type + body）
        let buf = [0x00, 0x00, 0x00, 0x07];
        let result = Frame::decode(&buf);
        assert_eq!(result, Err(FrameError::UnexpectedEof));
    }

    #[test]
    fn truncated_body_returns_err() {
        // 声明 10 字节 payload，只提供 5 字节
        let mut buf = (10u32).to_be_bytes().to_vec();
        buf.extend_from_slice(&[1u8; 5]); // version=1, type=1, body=3 bytes
        let result = Frame::decode(&buf);
        assert_eq!(result, Err(FrameError::UnexpectedEof));
    }

    #[test]
    fn max_allowed_length_accepted() {
        // MAX_FRAME_LEN = 65536，应接受
        let len = MAX_FRAME_LEN as u32;
        let mut buf = len.to_be_bytes().to_vec();
        buf.extend_from_slice(&[1u8; MAX_FRAME_LEN]);
        let result = Frame::decode(&buf);
        assert!(result.is_ok());
    }

    #[test]
    fn max_length_plus_one_rejected() {
        // 超过 MAX_FRAME_LEN 应拒绝（DoS 防护）
        let len = (MAX_FRAME_LEN + 1) as u32;
        let mut buf = len.to_be_bytes().to_vec();
        buf.extend_from_slice(&[1u8; 4]); // version + type
        let result = Frame::decode(&buf);
        assert_eq!(result, Err(FrameError::TooLarge { len: MAX_FRAME_LEN + 1 }));
    }

    #[test]
    fn u32_max_length_rejected_without_panic() {
        // u32::MAX = 4GB，应拒绝而非尝试分配
        let buf = (u32::MAX).to_be_bytes().to_vec();
        let result = Frame::decode(&buf);
        assert_eq!(result, Err(FrameError::TooLarge { len: u32::MAX as usize }));
    }

    #[test]
    fn wrong_magic_bytes_rejected() {
        // 注：协议用 length-prefix，无固定 magic 字节
        // 但可以测试错误 version
        let mut buf = (2u32).to_be_bytes().to_vec();
        buf.extend_from_slice(&[0xFF, 0x01]); // version=255 (错误), type=1
        let result = Frame::decode(&buf);
        assert_eq!(result, Err(FrameError::BadVersion(0xFF)));
    }

    #[test]
    fn unknown_frame_type_rejected() {
        let mut buf = (2u32).to_be_bytes().to_vec();
        buf.extend_from_slice(&[1, 0xAB]); // version=1, type=0xAB (未定义)
        let result = Frame::decode(&buf);
        assert_eq!(result, Err(FrameError::UnknownFrameType(0xAB)));
    }

    #[test]
    fn frame_type_from_u8_unknown_returns_none() {
        assert_eq!(FrameType::from_u8(255), Some(FrameType::Heartbeat));
        assert_eq!(FrameType::from_u8(200), None);
        assert_eq!(FrameType::from_u8(0x00), None);
    }

    // ---- decode_state_update 边界测试 ----

    #[test]
    fn state_update_empty_body_returns_none() {
        assert!(decode_state_update(&[]).is_none());
    }

    #[test]
    fn state_update_truncated_body_returns_none() {
        // 需要 19 字节，逐字节缩短测试
        for len in 0..19 {
            let body = vec![0u8; len];
            let result = decode_state_update(&body);
            assert!(result.is_none(), "len={} 应返回 None", len);
        }
    }

    #[test]
    fn state_update_exact_19_bytes_accepted() {
        let body = vec![
            0, 0, 0, 1, // entity_id
            0, 0, 0, 0, 0, 0, 0, 1, // timestamp
            0, 1, // slice_mask
            0, // endpoint_idx
            0x3F, 0x80, 0x00, 0x00, // delta_value = 1.0f32
        ];
        let result = decode_state_update(&body);
        assert!(result.is_some());
    }

    #[test]
    fn state_update_u32_max_entity_id_accepted() {
        let mut body = [0u8; 19];
        body[0..4].copy_from_slice(&u32::MAX.to_be_bytes());
        let result = decode_state_update(&body);
        assert!(result.is_some());
        let (id, _) = result.unwrap();
        assert_eq!(id, u32::MAX);
    }

    #[test]
    fn state_update_nan_delta_accepted() {
        let mut body = [0u8; 19];
        body[15..19].copy_from_slice(&f32::NAN.to_be_bytes());
        let result = decode_state_update(&body);
        // 当前实现接受 NaN（可能需要上层过滤）
        assert!(result.is_some());
        let (_, ev) = result.unwrap();
        assert!(ev.delta_value.is_nan());
    }

    #[test]
    fn state_update_infinity_delta_accepted() {
        let mut body = [0u8; 19];
        body[15..19].copy_from_slice(&f32::INFINITY.to_be_bytes());
        let result = decode_state_update(&body);
        assert!(result.is_some());
        let (_, ev) = result.unwrap();
        assert!(ev.delta_value.is_infinite());
    }

    // ---- decode_cascade_run 边界测试 ----

    #[test]
    fn cascade_run_empty_body_returns_none() {
        assert!(decode_cascade_run(&[]).is_none());
    }

    #[test]
    fn cascade_run_truncated_before_count_returns_none() {
        // 最少需要 7 字节：theta(4) + max_hops(1) + count(2)
        for len in 0..7 {
            let body = vec![0u8; len];
            assert!(decode_cascade_run(&body).is_none());
        }
    }

    #[test]
    fn cascade_run_zero_max_hops_accepted() {
        let body = encode_cascade_run(0.1, 0, &[]);
        let result = decode_cascade_run(&body);
        assert!(result.is_some());
        let (_, hops, _) = result.unwrap();
        assert_eq!(hops, 0);
    }

    #[test]
    fn cascade_run_max_hops_255_accepted() {
        let body = encode_cascade_run(0.1, 255, &[]);
        let result = decode_cascade_run(&body);
        assert!(result.is_some());
        let (_, hops, _) = result.unwrap();
        assert_eq!(hops, 255);
    }

    #[test]
    fn cascade_run_negative_theta_accepted() {
        let body = encode_cascade_run(-1.0, 5, &[]);
        let result = decode_cascade_run(&body);
        assert!(result.is_some());
        let (theta, _, _) = result.unwrap();
        assert_eq!(theta, -1.0);
    }

    #[test]
    fn cascade_run_nan_theta_accepted() {
        let mut body = [0u8; 7];
        body[0..4].copy_from_slice(&f32::NAN.to_be_bytes());
        let result = decode_cascade_run(&body);
        assert!(result.is_some());
        let (theta, _, _) = result.unwrap();
        assert!(theta.is_nan());
    }

    #[test]
    fn cascade_run_truncated_initial_shock_returns_none() {
        // count=1 但没有足够的 8 字节初始数据
        let mut body = [0u8; 7];
        body[0..4].copy_from_slice(&0.1f32.to_be_bytes());
        body[4] = 5; // max_hops
        body[5..7].copy_from_slice(&1u16.to_be_bytes()); // count=1
        assert!(decode_cascade_run(&body).is_none());
    }

    #[test]
    fn cascade_run_empty_initial_accepted() {
        let body = encode_cascade_run(0.1, 5, &[]);
        let result = decode_cascade_run(&body);
        assert!(result.is_some());
        let (_, _, initial) = result.unwrap();
        assert!(initial.is_empty());
    }

    #[test]
    fn cascade_run_u32_max_entity_id_accepted() {
        let body = encode_cascade_run(0.1, 5, &[(u32::MAX, 1.0)]);
        let result = decode_cascade_run(&body);
        assert!(result.is_some());
        let (_, _, initial) = result.unwrap();
        assert_eq!(initial[0].0, u32::MAX);
    }

    // ---- decode_cascade_results 边界测试 ----

    #[test]
    fn cascade_results_empty_body_returns_none() {
        assert!(decode_cascade_results(&[]).is_none());
    }

    #[test]
    fn cascade_results_only_count_byte_returns_none() {
        // 需要 2 字节 count
        assert!(decode_cascade_results(&[1]).is_none());
    }

    #[test]
    fn cascade_results_zero_count_accepted() {
        let body = encode_cascade_results(&[]);
        let result = decode_cascade_results(&body);
        assert!(result.is_some());
        let results = result.unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn cascade_results_truncated_entry_returns_none() {
        // count=1 但没有足够的 13 字节数据
        let body = [0, 1]; // count=1
        assert!(decode_cascade_results(&body).is_none());
    }

    #[test]
    fn cascade_results_nan_confidence_accepted() {
        let mut body = [0u8; 15]; // 2 + 13
        body[0..2].copy_from_slice(&1u16.to_be_bytes());
        body[6..10].copy_from_slice(&f32::NAN.to_be_bytes());
        let result = decode_cascade_results(&body);
        assert!(result.is_some());
        let results = result.unwrap();
        assert!(results[0].confidence.is_nan());
    }

    // ---- 往返一致性边界测试 ----

    #[test]
    fn state_update_round_trip_with_boundary_values() {
        let test_cases = vec![
            (0u32, DeltaEvent::new(0, 0, 0, 0.0)),
            (u32::MAX, DeltaEvent::new(u64::MAX, u16::MAX, u8::MAX, f32::MAX)),
            (42, DeltaEvent::new(0, 0xFFFF, 7, -1.0)),
        ];

        for (id, ev) in test_cases {
            let body = encode_state_update(id, &ev);
            let (id2, ev2) = decode_state_update(&body).expect("应解码成功");
            assert_eq!(id, id2);
            // packed 字段按值拷入元组再比较（避免对 packed 字段取引用）
            let got = (ev2.timestamp_us, ev2.slice_mask, ev2.endpoint_idx);
            let want = (ev.timestamp_us, ev.slice_mask, ev.endpoint_idx);
            assert_eq!(got, want);
            // f32 比较
            assert!((ev.delta_value - ev2.delta_value).abs() < f32::EPSILON || ev.delta_value.is_nan());
        }
    }

    #[test]
    fn cascade_run_round_trip_with_boundary_values() {
        let test_cases = vec![
            (0.0, 0u8, vec![]),
            (1.0, 255, vec![(u32::MAX, f32::MAX), (0, -1.0)]),
            (-1.0, 128, vec![(0, 0.0)]),
        ];

        for (theta, hops, initial) in test_cases {
            let body = encode_cascade_run(theta, hops, &initial);
            let (theta2, hops2, initial2) = decode_cascade_run(&body).expect("应解码成功");
            assert!((theta - theta2).abs() < f32::EPSILON || theta.is_nan());
            assert_eq!(hops, hops2);
            assert_eq!(initial, initial2);
        }
    }

    #[test]
    fn cascade_results_round_trip_with_boundary_values() {
        let test_cases = vec![
            vec![],
            vec![CascadeResult {
                entity_id: u32::MAX,
                confidence: f32::MAX,
                hop: 255, // hop is encoded as u8, max 255
                lag_us: u32::MAX,
            }],
            vec![
                CascadeResult { entity_id: 0, confidence: 0.0, hop: 0, lag_us: 0 },
                CascadeResult { entity_id: 1, confidence: -1.0, hop: 255, lag_us: u32::MAX },
            ],
        ];

        for results in test_cases {
            let body = encode_cascade_results(&results);
            let results2 = decode_cascade_results(&body).expect("应解码成功");
            assert_eq!(results.len(), results2.len());
            for (a, b) in results.iter().zip(results2.iter()) {
                assert_eq!(a.entity_id, b.entity_id);
                assert!((a.confidence - b.confidence).abs() < f32::EPSILON || a.confidence.is_nan());
                assert_eq!(a.hop, b.hop);
                assert_eq!(a.lag_us, b.lag_us);
            }
        }
    }
}

// =============================================================================
// 加载层边界测试
// =============================================================================

mod loader_boundary {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_csv_returns_empty_pool() {
        let pool = loader::load_from_text("");
        assert_eq!(pool.entities.len(), 0);
        assert_eq!(pool.names.len(), 0);
    }

    #[test]
    fn csv_with_only_header_returns_empty_pool() {
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 0);
    }

    #[test]
    fn csv_with_only_header_and_empty_lines_returns_empty_pool() {
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\n\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 0);
    }

    #[test]
    fn csv_with_empty_line_in_middle_skips_gracefully() {
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test Entity,TE,Industry,Region,Note,Type,A. 宏观对冲基金\n\
                   \n\
                   Another Entity,AE,Industry2,Region2,Note2,Type2,B\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 2);
    }

    #[test]
    fn csv_with_crlf_line_endings_accepted() {
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\r\n\
                   Test Entity,TE,Industry,Region,Note,Type,A. 宏观对冲基金\r\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 1);
    }

    #[test]
    fn csv_with_fewer_columns_than_expected_skips_line() {
        // 少一列（只有 6 列）
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test Entity,TE,Industry,Region,Note,Type\n"; // 缺少分类列
        let pool = loader::load_from_text(csv);
        // 当前行被跳过
        assert_eq!(pool.entities.len(), 0);
    }

    #[test]
    fn csv_with_more_columns_than_expected_skips_line() {
        // 多一列（8 列）应被跳过（避免静默数据错位）
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test Entity,TE,Industry,Region,Note,Type,A. 宏观对冲基金,Extra Column\n";
        let pool = loader::load_from_text(csv);
        // 该行应被跳过（列数不匹配，splitn(7) 后第 7 列含逗号导致错位）
        assert_eq!(pool.entities.len(), 0);
    }

    #[test]
    fn csv_with_duplicate_entity_ids_overwrites() {
        // 相同的 entity_id（行号）会被覆盖吗？
        // 实际上由于用 enumerate()，后一行会有不同的 id
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Entity One,E1,Ind,Reg,Note,T,A\n\
                   Entity Two,E2,Ind,Reg,Note,T,A\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 2);
        assert_ne!(pool.entities[0].id, pool.entities[1].id);
    }

    #[test]
    fn csv_with_invalid_category_defaults_to_e() {
        // 未知分类 → E (code 4)
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test Entity,TE,Industry,Region,Note,Type,X. 未知分类\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 1);
        assert_eq!(pool.entities[0].entity_type, 4); // E
    }

    #[test]
    fn csv_with_all_valid_categories() {
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   E1,TE,Ind,Reg,Note,T,A. 宏观对冲基金\n\
                   E2,TE2,Ind,Reg,Note,T,B. 主权财富基金\n\
                   E3,TE3,Ind,Reg,Note,T,C. 家族办公室\n\
                   E4,TE4,Ind,Reg,Note,T,D. 央企集团\n\
                   E5,TE5,Ind,Reg,Note,T,E. 其他\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 5);
        assert_eq!(pool.entities[0].entity_type, 0); // A
        assert_eq!(pool.entities[1].entity_type, 1); // B
        assert_eq!(pool.entities[2].entity_type, 2); // C
        assert_eq!(pool.entities[3].entity_type, 3); // D
        assert_eq!(pool.entities[4].entity_type, 4); // E
    }

    #[test]
    fn csv_with_utf8_bom_accepted() {
        let mut csv = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        csv.extend_from_slice("组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test Entity,TE,Industry,Region,Note,Type,A. 宏观对冲基金\n".as_bytes());
        let pool = loader::load_from_text(&String::from_utf8_lossy(&csv));
        assert_eq!(pool.entities.len(), 1);
    }

    #[test]
    fn csv_with_chinese_entity_names_accepted() {
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   中国中信集团,中信,金融,北京,Note,T,A. 宏观对冲基金\n\
                   BlackRock,BR,Finance,US,Note,T,B. 主权财富基金\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 2);
        assert_eq!(pool.names[0], "中国中信集团");
    }

    #[test]
    fn csv_with_comma_in_field_is_skipped_with_warning() {
        // 字段含逗号会被检测并跳过（不静默损坏）
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test, Inc.,TI,Industry,Region,Note,Type,A. 宏观对冲基金\n";
        let pool = loader::load_from_text(csv);
        // 该行应被跳过（字段内含逗号导致 splitn(7) 后第 7 列仍含逗号）
        assert_eq!(pool.entities.len(), 0);
    }

    #[test]
    fn csv_with_quote_in_field_is_skipped_with_warning() {
        // 字段含引号会被检测并跳过（不静默损坏）
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   \"Test Entity\",TE,Industry,Region,Note,Type,A. 宏观对冲基金\n";
        let pool = loader::load_from_text(csv);
        // 该行应被跳过（含引号字符）
        assert_eq!(pool.entities.len(), 0);
    }

    #[test]
    fn load_from_nonexistent_file_returns_err() {
        let result = loader::load_from_file(PathBuf::from("/nonexistent/file.csv").as_path());
        assert!(result.is_err());
    }

    #[test]
    fn load_from_directory_returns_err() {
        let result = loader::load_from_file(PathBuf::from("/tmp").as_path());
        assert!(result.is_err());
    }

    #[test]
    fn load_from_empty_file_returns_empty_pool() {
        // 测试空文件行为
        let csv = "";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 0);
    }

    #[test]
    fn csv_with_invalid_numeric_field_accepted() {
        // 数值字段（类型编号）不是数字，当前实现不影响解析
        // 因为类型编号不被使用
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test Entity,TE,Industry,Region,Note,NotANumber,A. 宏观对冲基金\n";
        let pool = loader::load_from_text(csv);
        // 仍会解析成功
        assert_eq!(pool.entities.len(), 1);
    }

    #[test]
    fn chinese_entity_detection_works() {
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   中国中信集团,中信,金融,北京,Note,T,A. 宏观对冲基金\n\
                   BlackRock,BR,Finance,US,Note,T,B. 主权财富基金\n\
                   中投公司,CIC,Finance,Beijing,Note,T,A. 宏观对冲基金\n";
        let pool = loader::load_from_text(csv);
        assert_eq!(pool.entities.len(), 3);
        assert_eq!(pool.entities[0].steady_state.ownership_type, 1); // 中国主体
        assert_eq!(pool.entities[1].steady_state.ownership_type, 0); // 非中国
        assert_eq!(pool.entities[2].steady_state.ownership_type, 1); // 中国主体
    }

    #[test]
    fn entity_id_does_not_overflow_u32() {
        // 创建一个很长的 CSV 测试 u32 溢出
        // 由于用 enumerate()，第一行表头被跳过，数据行从 i=1 开始
        // 需要超过 2^32 行才会溢出，不现实
        // 但可以验证 id 随行数增长
        let mut csv = String::from("组织名称,简称,行业,地区,备注,类型编号,分类\n");
        for i in 0..100 {
            csv.push_str(&format!("E{i},E{i},Ind,Reg,Note,T,A\n"));
        }
        let pool = loader::load_from_text(&csv);
        assert_eq!(pool.entities.len(), 100);
        // 第一行数据（表头后第一行）的 id 是 1（enumerate 从 0 开始，跳过 i=0 表头）
        for (i, e) in pool.entities.iter().enumerate() {
            assert_eq!(e.id, (i + 1) as u32);
        }
    }

    #[test]
    fn category_code_handles_non_ascii_first_char() {
        // 分类字段首字符不是 A-E
        let csv = "组织名称,简称,行业,地区,备注,类型编号,分类\n\
                   Test Entity,TE,Industry,Region,Note,T,1. 其他\n\
                   Test2,TE2,Industry,Region,Note,T,. 分类\n\
                   Test3,TE3,Industry,Region,Note,T, 宏观\n";
        let pool = loader::load_from_text(csv);
        // 未知分类都应默认为 E
        assert_eq!(pool.entities.len(), 3);
        for e in &pool.entities {
            assert_eq!(e.entity_type, 4); // E
        }
    }

    #[test]
    fn long_entity_name_accepted() {
        let long_name = "A".repeat(1000);
        let csv = format!("组织名称,简称,行业,地区,备注,类型编号,分类\n\
                          {},Long,Industry,Region,Note,T,A\n", long_name);
        let pool = loader::load_from_text(&csv);
        assert_eq!(pool.entities.len(), 1);
        assert_eq!(pool.names[0].len(), 1000);
    }
}
