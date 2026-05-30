# VP9 编码器性能优化设计

## 当前状态分析

### 1. BGRA 到 I420 转换 (`bgra_to_i420` 函数)
**位置**: `agent/src/encoder/vp9.rs` 第 376-416 行

**当前实现**:
```rust
fn bgra_to_i420(bgra: &[u8], width: u32, height: u32, i420: &mut [u8]) -> Result<()> {
    // ...
    for y in 0..height_usize {
        for x in 0..width_usize {
            let bgra_idx = (y * width_usize + x) * 4;
            let y_idx = y * width_usize + x;

            let b = bgra[bgra_idx] as f32;
            let g = bgra[bgra_idx + 1] as f32;
            let r = bgra[bgra_idx + 2] as f32;

            // Convert to YUV (BT.601)
            let y_val = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
            let u_val = (0.492 * (b - y_val as f32) + 128.0) as u8;
            let v_val = (0.877 * (r - y_val as f32) + 128.0) as u8;

            y_plane[y_idx] = y_val;

            // Chroma subsampling (4:2:0)
            if x % 2 == 0 && y % 2 == 0 {
                let uv_idx = (y / 2) * (width_usize / 2) + x / 2;
                u_plane[uv_idx] = u_val;
                v_plane[uv_idx] = v_val;
            }
        }
    }
    Ok(())
}
```

**性能瓶颈**:
1. **浮点运算**: 每个像素进行 3 次浮点乘法、3 次浮点加法
2. **逐像素处理**: 无法利用 SIMD 指令
3. **条件分支**: `if x % 2 == 0 && y % 2 == 0` 在每个像素上执行

### 2. 缓冲区复用
**当前状态**: ✅ 已实现
- `VP9Encoder` 结构体已有 `i420_buffer: Vec<u8>` 字段 (第 21 行)
- `encode` 函数复用 `self.i420_buffer` (第 248 行)
- `set_resolution` 函数在分辨率变化时重新分配缓冲区 (第 216 行)

## 优化方案

### 方案 A: 查找表优化 (推荐 - 简单有效)

**优点**:
- 实现简单，无需 unsafe
- 消除浮点运算
- 性能提升约 2-3x

**实现**:
```rust
static Y_R: [u8; 256] = [...]; // 预计算 Y = (0.299 * R) as u8
static Y_G: [u8; 256] = [...]; // 预计算 Y = (0.587 * G) as u8
static Y_B: [u8; 256] = [...]; // 预计算 Y = (0.114 * B) as u8

fn bgra_to_i420_optimized(bgra: &[u8], width: u32, height: u32, i420: &mut [u8]) -> Result<()> {
    // ...
    for y in 0..height_usize {
        for x in 0..width_usize {
            let bgra_idx = (y * width_usize + x) * 4;
            
            let b = bgra[bgra_idx];
            let g = bgra[bgra_idx + 1];
            let r = bgra[bgra_idx + 2];

            // 使用查找表替代浮点运算
            let y_val = Y_R[r as usize] + Y_G[g as usize] + Y_B[b as usize];
            
            // U = 0.492 * (B - Y) + 128 = 0.492 * B - 0.492 * Y + 128
            // 可以进一步优化为查找表
            let u_val = ((0.492 * (b as i32 - y_val as i32) + 128.0) as u8);
            let v_val = ((0.877 * (r as i32 - y_val as i32) + 128.0) as u8);

            y_plane[y_idx] = y_val;

            if x % 2 == 0 && y % 2 == 0 {
                let uv_idx = (y / 2) * (width_usize / 2) + x / 2;
                u_plane[uv_idx] = u_val;
                v_plane[uv_idx] = v_val;
            }
        }
    }
    Ok(())
}
```

### 方案 B: SIMD 优化 (进阶)

**优点**:
- 性能提升约 4-8x
- 一次处理 4/8/16 个像素

**实现**: 使用 `std::arch::x86_64` 或 `core::arch::aarch64`

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

fn process_pixels_simd(bgra: &[u8], y_plane: &mut [u8], ...) {
    // 使用 SSE/AVX 指令一次处理多个像素
}
```

### 方案 C: 整数运算优化 (折中方案)

**优点**:
- 无需查找表
- 无需 SIMD
- 性能提升约 1.5-2x

**实现**:
```rust
fn bgra_to_i420_integer(bgra: &[u8], width: u32, height: u32, i420: &mut [u8]) -> Result<()> {
    // ...
    for y in 0..height_usize {
        for x in 0..width_usize {
            let bgra_idx = (y * width_usize + x) * 4;
            
            let b = bgra[bgra_idx] as i32;
            let g = bgra[bgra_idx + 1] as i32;
            let r = bgra[bgra_idx + 2] as i32;

            // 使用整数运算 (乘以 1000 避免浮点)
            let y_val = ((299 * r + 587 * g + 114 * b) / 1000) as u8;
            let u_val = ((492 * (b - y_val as i32) + 128000) / 1000) as u8;
            let v_val = ((877 * (r - y_val as i32) + 128000) / 1000) as u8;

            y_plane[y_idx] = y_val;

            if x % 2 == 0 && y % 2 == 0 {
                let uv_idx = (y / 2) * (width_usize / 2) + x / 2;
                u_plane[uv_idx] = u_val;
                v_plane[uv_idx] = v_val;
            }
        }
    }
    Ok(())
}
```

## 推荐实施顺序

1. **阶段 1**: 整数运算优化 (方案 C)
   - 风险最低
   - 无需查找表维护
   - 立即见效

2. **阶段 2**: 查找表优化 (方案 A)
   - 在阶段 1 基础上进一步优化
   - 预计算查找表

3. **阶段 3**: SIMD 优化 (方案 B)
   - 在性能关键场景下使用
   - 需要条件编译支持不同架构

## 验证计划

1. `cargo check -p rdp-agent` - 编译验证
2. `cargo test -p rdp-agent` - 单元测试验证
3. `cargo bench --bench encoder_bench` - 性能基准测试
