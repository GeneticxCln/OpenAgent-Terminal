// WGPU offscreen snapshot example
// Renders a solid color to an offscreen texture and writes a PNG file.
// Prints a JSON summary with output path and dimensions for CI parsing.

#[cfg(feature = "wgpu")]
use std::fs;
#[cfg(feature = "wgpu")]
use std::path::PathBuf;

#[cfg(feature = "wgpu")]
use image::{ImageBuffer, Rgba};

#[cfg(feature = "wgpu")]
async fn run_wgpu(width: u32, height: u32, out_path: &str) -> anyhow::Result<()> {
    use wgpu::util::DeviceExt;

    // Create instance and request adapter (headless)
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::all(), ..Default::default() });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| anyhow::anyhow!("No suitable WGPU adapter found"))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("OpenAgent WGPU Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        )
        .await?;

    // Texture we'll render into
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Clear the texture to a deterministic color without drawing any geometry
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clear-encoder") });
    {
        let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.4, a: 1.0 }),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }

    // Copy texture into a mappable buffer
    let bytes_per_pixel = 4u32;
    let bytes_per_row = bytes_per_pixel * width;
    // WGPU requires bytes_per_row to be a multiple of wgpu::COPY_BYTES_PER_ROW_ALIGNMENT (256)
    let padded_bytes_per_row = ((bytes_per_row + 255) / 256) * 256;
    let output_buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Read back the buffer
    let slice = output_buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();

    // Remove row padding and write PNG
    let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
    for chunk in data.chunks(padded_bytes_per_row as usize) {
        pixels.extend_from_slice(&chunk[..bytes_per_row as usize]);
    }
    drop(data);
    output_buffer.unmap();

    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, pixels)
        .ok_or_else(|| anyhow::anyhow!("failed to build image buffer"))?;

    let out_path = PathBuf::from(out_path);
    if let Some(parent) = out_path.parent() { fs::create_dir_all(parent)?; }
    img.save(&out_path)?;

    // Print JSON for CI step parsing
    println!("{{\"output\":\"{}\",\"width\":{},\"height\":{}}}", out_path.display(), width, height);

    Ok(())
}

fn main() {
    // Use fixed size expected by CI step
    let width = 256u32;
    let height = 128u32;
    let out_path = "tests/snapshot_output/wgpu_offscreen.png";

    #[cfg(feature = "wgpu")]
    {
        if let Err(e) = pollster::block_on(run_wgpu(width, height, out_path)) {
            eprintln!("WGPU snapshot failed: {}", e);
            // Still print a JSON with expected size but no file to make debugging easier
            println!("{{\"output\":\"{}\",\"width\":{},\"height\":{}}}", out_path, width, height);
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "wgpu"))]
    {
        // If WGPU feature isn't enabled, print JSON and exit 0 so CI can handle fallback
        println!("{{\"output\":\"{}\",\"width\":{},\"height\":{}}}", out_path, width, height);
    }
}

