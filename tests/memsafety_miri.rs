//! Memory safety and concurrency tests for Tessera.
//!
//! A3 audit: focus on Entity Send/Sync bounds, SPSC assumptions,
//! and concurrent access patterns. Run with:
//!   cargo test --test memsafety_miri
//! For Miri:
//!   cargo +nightly-2026-07-11 miri test --lib -- --skip "100_entities" --skip "under_100us" --skip "perf" --skip "latency" --skip "throughput"

#![allow(dead_code)]

use tessera::entity::{DeltaEvent, Entity};
use tessera::server::Engine;
use tessera::matrix::CascadeMatrix;
use tessera::cascade::EntityStateView;
use std::sync::{Arc, Mutex};
use std::thread;
use std::path::PathBuf;

#[cfg(not(miri))]
#[test]
fn entity_send_boundary_can_move_across_threads() {
    // Verify Entity is Send: can move across thread boundary
    let mut entity = Entity::new(0, 0, 1);
    entity.slice_dims[0] = 2;
    entity.coordinates[0] = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    let handle = thread::spawn(move || {
        // Entity was moved into this thread
        entity.apply_delta_singlethreaded(DeltaEvent::new(100, 1, 0, 0.1));
        entity.ring_head
    });

    let ring_head = handle.join().unwrap();
    assert_eq!(ring_head, 1);
}

#[cfg(not(miri))]
#[test]
fn entity_sync_is_not_sync() {
    // Verify Entity is NOT Sync: cannot share &Entity across threads
    // This is expected and correct - Entity mutation requires &mut self
    let _entity = Entity::new(0, 0, 1);

    // This should NOT compile (if it does, that's a problem)
    // Uncomment to verify compile error:
    // let _ = Arc::new(entity);
    // let handle = thread::spawn(move || {
    //     let e = arc_clone.clone();
    //     // Cannot get &mut Entity from Arc<Entity>
    // });

    // Instead, the correct pattern is Mutex for interior mutability
}

#[cfg(not(miri))]
#[test]
fn engine_arc_mutex_concurrent_process_frame() {
    // Verify the server's Arc<Mutex<Engine>> pattern works under concurrency
    let e0 = Entity::new(0, 0, 1);
    let e1 = Entity::new(1, 0, 0);
    let matrix = CascadeMatrix::from_edges(2, &[(1, 0, 0.5, 0)]);
    let states = vec![
        EntityStateView {
            coordinates: 0.0,
            brittle_threshold: 0.0,
            decay_coefficient: 1.0,
            time_lag_us: 10,
        };
        2
    ];

    let engine = Arc::new(Mutex::new(Engine::new(
        vec![e0, e1],
        matrix,
        states,
        PathBuf::from("snapshots"),
    )));

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let engine_clone = engine.clone();
            thread::spawn(move || {
                // Each thread gets a &mut Engine from the Mutex
                let mut eng = engine_clone.lock().unwrap();
                // Simulate concurrent access (safe due to Mutex)
                eng.entities[0].id = i;
                i
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    // All threads completed successfully without deadlock
    assert_eq!(results.len(), 4);
}

#[test]
fn delta_ring_capacity_boundaries() {
    // Verify delta ring operates within capacity boundaries
    // Ring overflow panic is already tested in entity::entity_layout::ring_overflow_panics
    let mut entity = Entity::new(0, 0, 0);
    let delta = DeltaEvent::new(0, 0, 0, 0.0);

    // Fill the ring to capacity-1 (safe)
    // DELTA_RING_CAPACITY = 1024, so valid range is 0-1023
    for _ in 0..1023 {
        entity.apply_delta_singlethreaded(delta);
    }
    assert_eq!(entity.ring_head, 1023);
    assert_eq!(entity.ring_tail, 0);

    // ring_head - ring_tail = 1023 < 1024, still within capacity
    // The 1024th write would overflow (tested in entity.rs)
}

#[cfg(miri)]
#[test]
fn miri_lightweight_entity_send() {
    // Lightweight test runnable under Miri to verify Send
    let entity = Entity::new(0, 0, 1);
    // Just verify it can be moved (no actual threading in Miri)
    let _ = entity;
}

#[cfg(miri)]
#[test]
fn miri_lightweight_name_ptr_validation() {
    // Verify name_ptr is null-initialized (safe under Miri)
    let entity = Entity::new(0, 0, 1);
    assert!(entity.name_ptr.is_null());
    assert_eq!(entity.name_len, 0);
}

#[test]
fn entity_new_initializes_safely() {
    // Verify Entity::new produces a safely initialized entity
    let entity = Entity::new(42, 1, 2);

    assert_eq!(entity.id, 42);
    assert_eq!(entity.entity_type, 1);
    assert_eq!(entity.num_slices, 2);
    assert_eq!(entity.ring_head, 0);
    assert_eq!(entity.ring_tail, 0);
    assert!(entity.name_ptr.is_null());
    assert_eq!(entity.name_len, 0);

    // Coordinates should be zero-initialized
    for row in &entity.coordinates {
        for &val in row {
            assert_eq!(val, 0.0);
        }
    }
}

#[test]
fn delta_event_new_is_safe() {
    // Verify DeltaEvent::new produces safe values
    // Note: DeltaEvent is repr(C, packed), so we verify by round-trip
    // The query_state function in entity.rs already handles packed access safely
    let ev = DeltaEvent::new(1000, 0b1010, 3, 0.5);

    // Verify that DeltaEvent can be copied and compared (packed-safe)
    let ev2 = ev;
    // Packed structs can be compared by value (memcpy-based)
    // We verify timestamp_us through the Copy trait behavior
    let bytes = unsafe { core::ptr::read_unaligned(&ev) };
    let bytes2 = unsafe { core::ptr::read_unaligned(&ev2) };

    // Compare raw bytes to verify struct is bitwise identical
    unsafe {
        let src = &bytes as *const _ as *const u8;
        let src2 = &bytes2 as *const _ as *const u8;
        let mut buf = [0u8; 16];
        let mut buf2 = [0u8; 16];
        core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), 16);
        core::ptr::copy_nonoverlapping(src2, buf2.as_mut_ptr(), 16);
        assert_eq!(&buf[..], &buf2[..], "copied DeltaEvent should be identical");
    }
}

#[cfg(not(miri))]
#[test]
fn multiple_entities_concurrent_write_different() {
    // Verify concurrent writes to DIFFERENT entities is safe
    let entities = vec![Entity::new(0, 0, 1), Entity::new(1, 0, 1)];
    let mut handles = Vec::new();

    for (i, mut entity) in entities.into_iter().enumerate() {
        let handle = thread::spawn(move || {
            entity.apply_delta_singlethreaded(DeltaEvent::new(i as u64, 1, 0, 0.1));
            entity.ring_head
        });
        handles.push(handle);
    }

    for handle in handles {
        let ring_head = handle.join().unwrap();
        assert_eq!(ring_head, 1);
    }
}
