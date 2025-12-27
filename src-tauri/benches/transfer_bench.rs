//! Benchmarks for file transfer and hashing operations
//!
//! Run with: cargo bench --bench transfer_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::io::Write;
use tempfile::NamedTempFile;

/// Generate test data of the specified size
fn generate_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Benchmark BLAKE3 streaming hash computation
fn bench_blake3_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("blake3_streaming");

    // Test different file sizes
    for size in [
        1024,              // 1 KB
        64 * 1024,         // 64 KB
        1024 * 1024,       // 1 MB
        10 * 1024 * 1024,  // 10 MB
        100 * 1024 * 1024, // 100 MB
    ] {
        let data = generate_test_data(size);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut hasher = blake3::Hasher::new();
                    // Simulate streaming with 64KB chunks
                    for chunk in data.chunks(64 * 1024) {
                        hasher.update(black_box(chunk));
                    }
                    hasher.finalize()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark BLAKE3 full-file hash (for comparison)
fn bench_blake3_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("blake3_full");

    for size in [
        1024,             // 1 KB
        64 * 1024,        // 64 KB
        1024 * 1024,      // 1 MB
        10 * 1024 * 1024, // 10 MB
    ] {
        let data = generate_test_data(size);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format_size(size)),
            &data,
            |b, data| {
                b.iter(|| blake3::hash(black_box(data)));
            },
        );
    }

    group.finish();
}

/// Benchmark file read with streaming
fn bench_file_read_streaming(c: &mut Criterion) {
    use std::io::Read;

    let mut group = c.benchmark_group("file_read_streaming");

    for size in [
        1024 * 1024,      // 1 MB
        10 * 1024 * 1024, // 10 MB
    ] {
        // Create temp file with test data
        let data = generate_test_data(size);
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();
        let path = temp_file.path().to_path_buf();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format_size(size)),
            &path,
            |b, path| {
                b.iter(|| {
                    let file = std::fs::File::open(path).unwrap();
                    let mut reader = std::io::BufReader::with_capacity(64 * 1024, file);
                    let mut buffer = vec![0u8; 64 * 1024];
                    let mut total = 0usize;

                    loop {
                        let bytes_read = reader.read(&mut buffer).unwrap();
                        if bytes_read == 0 {
                            break;
                        }
                        total += bytes_read;
                    }

                    black_box(total)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark ChaCha20-Poly1305 encryption with streaming
fn bench_encryption_streaming(c: &mut Criterion) {
    use chacha20poly1305::{
        aead::{Aead, KeyInit},
        ChaCha20Poly1305,
    };

    let mut group = c.benchmark_group("chacha20poly1305_streaming");

    // Fixed key and nonce for benchmarking
    let key = [0u8; 32];
    let cipher = ChaCha20Poly1305::new_from_slice(&key).unwrap();

    for size in [
        64 * 1024,   // 64 KB chunk (our chunk size)
        1024 * 1024, // 1 MB
    ] {
        let data = generate_test_data(size);
        let nonce = chacha20poly1305::Nonce::from_slice(&[0u8; 12]);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format_size(size)),
            &data,
            |b, data| {
                b.iter(|| cipher.encrypt(black_box(nonce), black_box(data.as_slice())));
            },
        );
    }

    group.finish();
}

/// Benchmark broadcast channel throughput
fn bench_broadcast_channel(c: &mut Criterion) {
    use tokio::sync::broadcast;

    let mut group = c.benchmark_group("broadcast_channel");

    for capacity in [256, 1024, 4096] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("cap_{}", capacity)),
            &capacity,
            |b, &capacity| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let (tx, mut rx) = broadcast::channel::<u64>(capacity);

                        // Send messages
                        for i in 0..100 {
                            let _ = tx.send(black_box(i));
                        }

                        // Receive all
                        let mut count = 0;
                        while rx.try_recv().is_ok() {
                            count += 1;
                        }
                        black_box(count)
                    })
                });
            },
        );
    }

    group.finish();
}

/// Format size in human-readable form
fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{}MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{}B", bytes)
    }
}

criterion_group!(
    benches,
    bench_blake3_streaming,
    bench_blake3_full,
    bench_file_read_streaming,
    bench_encryption_streaming,
    bench_broadcast_channel,
);

criterion_main!(benches);
