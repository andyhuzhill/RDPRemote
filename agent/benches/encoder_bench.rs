use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use rdp_agent::encoder::{VP9Encoder, VideoEncoder};

fn create_test_frame(width: u32, height: u32) -> Vec<u8> {
    // 创建 BGRA 测试帧
    let mut frame = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        frame.extend_from_slice(&[128, 128, 128, 255]); // BGRA
    }
    frame
}

fn bench_vp9_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("vp9_encode");
    
    for (width, height) in [(640, 480), (1280, 720), (1920, 1080)].iter() {
        let frame = create_test_frame(*width, *height);
        let mut encoder = VP9Encoder::new(*width, *height, 2000).unwrap();
        
        group.bench_with_input(
            BenchmarkId::new("resolution", format!("{}x{}", width, height)),
            &frame,
            |b, frame| {
                b.iter(|| {
                    encoder.encode(&frame, *width, *height, 0).unwrap()
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, bench_vp9_encode);
criterion_main!(benches);
