use criterion::{criterion_group, criterion_main, Criterion};
use rdp_agent::roi::RoiDetector;

fn create_test_frame(width: u32, height: u32) -> Vec<u8> {
    vec![128u8; (width * height * 4) as usize]
}

fn bench_roi_detection(c: &mut Criterion) {
    c.bench_function("roi_detection_720p", |b| {
        let width = 1280u32;
        let height = 720u32;
        let mut roi = RoiDetector::new(width, height, 32);
        let frame = create_test_frame(width, height);
        
        b.iter(|| {
            roi.detect_changes(&frame, width, height)
        })
    });
}

fn bench_roi_hash(c: &mut Criterion) {
    c.bench_function("roi_hash_720p", |b| {
        let width = 1280u32;
        let height = 720u32;
        let frame = create_test_frame(width, height);
        
        b.iter(|| {
            // 测试 hash 计算性能
            let mut hash: u64 = 0;
            for chunk in frame.chunks(4) {
                let pixel = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                hash = hash.wrapping_add(pixel as u64);
            }
            hash
        })
    });
}

criterion_group!(benches, bench_roi_detection, bench_roi_hash);
criterion_main!(benches);
