use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use image::{ImageEncoder, RgbaImage};

use super::gpu_context::GpuContext;

pub(crate) struct ScreenshotReadback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
}

pub(crate) fn prepare_readback(
    gpu: &GpuContext,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
) -> ScreenshotReadback {
    let width = gpu.config.width;
    let height = gpu.config.height;
    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
    let buffer_size = padded_bytes_per_row as u64 * height as u64;

    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &readback,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    ScreenshotReadback {
        buffer: readback,
        width,
        height,
        unpadded_bytes_per_row,
        padded_bytes_per_row,
    }
}

pub(crate) fn finalize(
    gpu: &GpuContext,
    output_path: &Path,
    readback: ScreenshotReadback,
) -> Result<()> {
    let slice = readback.buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    gpu.device.poll(wgpu::Maintain::Wait);
    rx.recv().context("waiting for screenshot map")??;

    let data = slice.get_mapped_range();
    let mut pixels = vec![0u8; (readback.width * readback.height * 4) as usize];
    for y in 0..readback.height as usize {
        let src_start = y * readback.padded_bytes_per_row as usize;
        let src_end = src_start + readback.unpadded_bytes_per_row as usize;
        let dst_start = y * readback.unpadded_bytes_per_row as usize;
        let dst_end = dst_start + readback.unpadded_bytes_per_row as usize;
        pixels[dst_start..dst_end].copy_from_slice(&data[src_start..src_end]);
    }
    drop(data);
    readback.buffer.unmap();

    if gpu.config.format == wgpu::TextureFormat::Bgra8UnormSrgb
        || gpu.config.format == wgpu::TextureFormat::Bgra8Unorm
    {
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
    }

    let image = image::RgbaImage::from_raw(readback.width, readback.height, pixels)
        .context("failed to build screenshot image buffer")?;
    write_png_if_changed(output_path, &image)?;
    Ok(())
}

fn write_png_if_changed(output_path: &Path, image: &RgbaImage) -> Result<()> {
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ColorType::Rgba8.into(),
        )
        .with_context(|| format!("encoding screenshot for {}", output_path.display()))?;

    if fs::read(output_path).is_ok_and(|existing| existing == encoded) {
        return Ok(());
    }

    fs::write(output_path, encoded)
        .with_context(|| format!("saving screenshot to {}", output_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{Rgba, RgbaImage};

    use super::write_png_if_changed;

    #[test]
    fn write_png_if_changed_skips_identical_existing_file() {
        let path = std::env::temp_dir().join(format!(
            "stitchlands-render-skip-{}-{}.png",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let image = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));

        write_png_if_changed(&path, &image).unwrap();
        let original_permissions = std::fs::metadata(&path).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions).unwrap();

        write_png_if_changed(&path, &image).unwrap();

        std::fs::set_permissions(&path, original_permissions).unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
