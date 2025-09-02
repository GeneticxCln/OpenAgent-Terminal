// WGPU snapshot offscreen example: renders a solid color into an offscreen texture and reads back to PNG.
// Feature-gated on `wgpu`.

#![cfg(feature = "wgpu")]

use std::fs;
use std::num::NonZeroU32;

use image::{ImageBuffer, Rgba};

#[tokio::main]
async fn main() {
    let width: u32 = 256;
    let height: u32 = 128;

    // Instance and device
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        })
        .await
        .expect("no adapter");

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor { label: Some("snap-device"), required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::downlevel_webgl2_defaults() },
            None,
        )
        .await
        .expect("device");

    // Offscreen texture
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

    // Render: clear to color
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("snap-encoder") });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.3, b: 0.8, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        drop(pass);
    }

    // Buffer for readback (padded rows to 256 bytes alignment)
    let bytes_per_pixel = 4u32;
    let padded_bytes_per_row = ((width * bytes_per_pixel + 255) / 256) * 256;
    let buffer_size = (padded_bytes_per_row * height) as u64;

    let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &read_buf,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(NonZeroU32::new(padded_bytes_per_row).unwrap()),
                rows_per_image: Some(NonZeroU32::new(height).unwrap()),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );

    queue.submit([encoder.finish()]);
    device.poll(wgpu::Maintain::Wait);

    // Map and copy to tightly-packed buffer
    let slice = read_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {} ).await.unwrap();
    let data = slice.get_mapped_range();

    let mut img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
    for y in 0..height {
        let row_start = (y * padded_bytes_per_row) as usize;
        let row = &data[row_start..row_start + (width * bytes_per_pixel) as usize];
        for x in 0..width {
            let i = (x * bytes_per_pixel) as usize;
            let rgba = [row[i], row[i + 1], row[i + 2], row[i + 3]];
            img.put_pixel(x, height - 1 - y, Rgba(rgba));
        }
    }
    drop(data);
    read_buf.unmap();

    let out_dir = std::path::Path::new("tests/snapshot_output");
    let _ = fs::create_dir_all(out_dir);
    let out_path = out_dir.join("wgpu_offscreen.png");
    img.save(&out_path).expect("save png");
    println!("{{\"output\":\"{}\",\"width\":{},\"height\":{}}}", out_path.display(), width, height);
}

