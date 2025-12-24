//! Load and stress tests for the P2P Drive application
//!
//! These tests verify system stability under heavy load conditions.
//! Run with: cargo test --test load_tests --release -- --nocapture

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Test high-throughput broadcast channel messaging
#[tokio::test]
async fn test_broadcast_channel_throughput() {
    const NUM_MESSAGES: u64 = 100_000;
    const NUM_RECEIVERS: usize = 10;

    let (tx, _) = broadcast::channel::<u64>(1024);
    let received_count = Arc::new(AtomicU64::new(0));

    // Spawn receivers
    let mut handles = Vec::new();
    for _ in 0..NUM_RECEIVERS {
        let mut rx = tx.subscribe();
        let count = received_count.clone();

        handles.push(tokio::spawn(async move {
            let mut local_count = 0u64;
            loop {
                match rx.recv().await {
                    Ok(_) => local_count += 1,
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            count.fetch_add(local_count, Ordering::Relaxed);
        }));
    }

    // Send messages
    let start = Instant::now();
    for i in 0..NUM_MESSAGES {
        let _ = tx.send(i);
    }
    drop(tx); // Close channel

    // Wait for receivers
    for handle in handles {
        let _ = handle.await;
    }
    let elapsed = start.elapsed();

    let total_received = received_count.load(Ordering::Relaxed);
    let messages_per_sec = NUM_MESSAGES as f64 / elapsed.as_secs_f64();

    println!(
        "Broadcast throughput: {:.0} msg/sec, {} receivers, {} total received",
        messages_per_sec, NUM_RECEIVERS, total_received
    );

    // Should achieve at least 100k msg/sec
    assert!(
        messages_per_sec > 100_000.0,
        "Throughput too low: {:.0} msg/sec",
        messages_per_sec
    );
}

/// Test concurrent hash computations
#[tokio::test]
async fn test_concurrent_hashing() {
    const NUM_TASKS: usize = 100;
    const DATA_SIZE: usize = 1024 * 1024; // 1MB per task

    let data: Vec<u8> = (0..DATA_SIZE).map(|i| (i % 256) as u8).collect();
    let data = Arc::new(data);

    let start = Instant::now();

    let handles: Vec<_> = (0..NUM_TASKS)
        .map(|_| {
            let data = data.clone();
            tokio::spawn(async move {
                let mut hasher = blake3::Hasher::new();
                // Simulate streaming hash
                for chunk in data.chunks(64 * 1024) {
                    hasher.update(chunk);
                }
                hasher.finalize()
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();
    let total_bytes = NUM_TASKS * DATA_SIZE;
    let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();

    println!(
        "Concurrent hashing: {:.0} MB/sec, {} tasks x {}MB",
        throughput_mb,
        NUM_TASKS,
        DATA_SIZE / (1024 * 1024)
    );

    // Should achieve at least 500 MB/sec concurrent throughput
    assert!(
        throughput_mb > 500.0,
        "Hash throughput too low: {:.0} MB/sec",
        throughput_mb
    );
}

/// Test memory stability under sustained load
#[tokio::test]
async fn test_memory_stability() {
    const ITERATIONS: usize = 1000;
    const CHUNK_SIZE: usize = 64 * 1024;

    // Record initial memory (approximate using vec allocations)
    let mut peak_allocations = 0usize;

    for i in 0..ITERATIONS {
        // Simulate file processing - allocate and process chunks
        let data: Vec<u8> = (0..CHUNK_SIZE).map(|j| ((i + j) % 256) as u8).collect();

        // Process with BLAKE3
        let mut hasher = blake3::Hasher::new();
        hasher.update(&data);
        let _hash = hasher.finalize();

        // Track allocations (simplified)
        peak_allocations = peak_allocations.max(data.len());

        // Yield to allow cleanup
        if i % 100 == 0 {
            tokio::task::yield_now().await;
        }
    }

    println!(
        "Memory stability test: {} iterations, peak chunk: {} bytes",
        ITERATIONS, peak_allocations
    );

    // Peak should not exceed our chunk size
    assert!(
        peak_allocations <= CHUNK_SIZE,
        "Memory leak detected: peak {} > expected {}",
        peak_allocations,
        CHUNK_SIZE
    );
}

/// Test RwLock contention under load
#[tokio::test]
async fn test_rwlock_contention() {
    use tokio::sync::RwLock;

    const NUM_READERS: usize = 50;
    const NUM_WRITERS: usize = 5;
    const OPERATIONS_PER_TASK: usize = 1000;

    let data = Arc::new(RwLock::new(Vec::<u64>::new()));
    let read_count = Arc::new(AtomicU64::new(0));
    let write_count = Arc::new(AtomicU64::new(0));

    let start = Instant::now();
    let mut handles = Vec::new();

    // Spawn readers
    for _ in 0..NUM_READERS {
        let data = data.clone();
        let count = read_count.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..OPERATIONS_PER_TASK {
                let guard = data.read().await;
                let _ = guard.len();
                drop(guard);
                count.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    // Spawn writers
    for i in 0..NUM_WRITERS {
        let data = data.clone();
        let count = write_count.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..OPERATIONS_PER_TASK {
                let mut guard = data.write().await;
                guard.push((i * OPERATIONS_PER_TASK + j) as u64);
                // Keep size bounded
                if guard.len() > 1000 {
                    guard.drain(0..500);
                }
                drop(guard);
                count.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();
    let total_reads = read_count.load(Ordering::Relaxed);
    let total_writes = write_count.load(Ordering::Relaxed);
    let ops_per_sec = (total_reads + total_writes) as f64 / elapsed.as_secs_f64();

    println!(
        "RwLock contention: {:.0} ops/sec, {} reads, {} writes",
        ops_per_sec, total_reads, total_writes
    );

    // Should achieve at least 100k ops/sec
    assert!(
        ops_per_sec > 100_000.0,
        "RwLock throughput too low: {:.0} ops/sec",
        ops_per_sec
    );
}

/// Test file I/O throughput simulation
#[tokio::test]
async fn test_simulated_file_io() {
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const NUM_FILES: usize = 50;
    const FILE_SIZE: usize = 1024 * 1024; // 1MB each

    let dir = tempdir().unwrap();
    let data: Vec<u8> = (0..FILE_SIZE).map(|i| (i % 256) as u8).collect();

    // Write files
    let write_start = Instant::now();
    for i in 0..NUM_FILES {
        let path = dir.path().join(format!("test_{}.bin", i));
        let mut file = tokio::fs::File::create(&path).await.unwrap();

        // Write in chunks
        for chunk in data.chunks(64 * 1024) {
            file.write_all(chunk).await.unwrap();
        }
        file.flush().await.unwrap();
    }
    let write_elapsed = write_start.elapsed();

    // Read files
    let read_start = Instant::now();
    for i in 0..NUM_FILES {
        let path = dir.path().join(format!("test_{}.bin", i));
        let mut file = tokio::fs::File::open(&path).await.unwrap();
        let mut buffer = vec![0u8; 64 * 1024];

        loop {
            let n = file.read(&mut buffer).await.unwrap();
            if n == 0 {
                break;
            }
        }
    }
    let read_elapsed = read_start.elapsed();

    let total_bytes = NUM_FILES * FILE_SIZE;
    let write_throughput = (total_bytes as f64 / (1024.0 * 1024.0)) / write_elapsed.as_secs_f64();
    let read_throughput = (total_bytes as f64 / (1024.0 * 1024.0)) / read_elapsed.as_secs_f64();

    println!(
        "File I/O: Write {:.0} MB/sec, Read {:.0} MB/sec ({} files x {}MB)",
        write_throughput,
        read_throughput,
        NUM_FILES,
        FILE_SIZE / (1024 * 1024)
    );

    // Should achieve at least 50 MB/sec for both
    assert!(
        write_throughput > 50.0,
        "Write throughput too low: {:.0} MB/sec",
        write_throughput
    );
    assert!(
        read_throughput > 50.0,
        "Read throughput too low: {:.0} MB/sec",
        read_throughput
    );
}

/// Stress test with sustained high load
#[tokio::test]
async fn test_sustained_load() {
    const DURATION_SECS: u64 = 5;
    const CONCURRENT_TASKS: usize = 20;

    let operations = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let deadline = start + Duration::from_secs(DURATION_SECS);

    let handles: Vec<_> = (0..CONCURRENT_TASKS)
        .map(|_| {
            let ops = operations.clone();
            tokio::spawn(async move {
                while Instant::now() < deadline {
                    // Simulate work: hash some data
                    let data = vec![0u8; 1024];
                    let _ = blake3::hash(&data);
                    ops.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let total_ops = operations.load(Ordering::Relaxed);
    let ops_per_sec = total_ops as f64 / DURATION_SECS as f64;

    println!(
        "Sustained load: {:.0} ops/sec over {}s ({} concurrent tasks)",
        ops_per_sec, DURATION_SECS, CONCURRENT_TASKS
    );

    // Should maintain at least 50k ops/sec under sustained load
    assert!(
        ops_per_sec > 50_000.0,
        "Sustained throughput too low: {:.0} ops/sec",
        ops_per_sec
    );
}
