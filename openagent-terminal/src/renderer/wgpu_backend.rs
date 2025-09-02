// WGPU Renderer Backend for OpenAgent Terminal
// This module provides GPU-accelerated rendering using WGPU

use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, info, warn};
use wgpu::{
    Backends, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits,
    PowerPreference, PresentMode, Queue, RequestAdapterOptions, SurfaceConfiguration, TextureUsages,
};
use winit::window::Window;

use super::{rects::RenderRect, shader};
use crate::display::color::Rgb;

/// Performance metrics for WGPU renderer
#[derive(Debug, Clone)]
pub struct RenderMetrics {
    pub frame_time_ms: f32,
    pub gpu_time_ms: f32,
    pub cpu_time_ms: f32,
    pub draw_calls: u32,
    pub vertex_count: u32,
    pub memory_usage_mb: f32,
}

impl Default for RenderMetrics {
    fn default() -> Self {
        Self {
            frame_time_ms: 0.0,
            gpu_time_ms: 0.0,
            cpu_time_ms: 0.0,
            draw_calls: 0,
            vertex_count: 0,
            memory_usage_mb: 0.0,
        }
    }
}

/// WGPU Renderer State
pub struct WgpuRenderer {
    instance: Instance,
    device: Arc<Device>,
    queue: Arc<Queue>,
    config: SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    // Keep a raw pointer to the winit window; we recreate the surface on demand
    window_ptr: *const winit::window::Window,

    // Render pipeline components
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,

    // Text rendering
    text_atlas: TextAtlas,
    glyph_cache: GlyphCache,

    // Performance monitoring
    metrics: RenderMetrics,
    frame_counter: u64,
    enable_performance_hud: bool,
}

/// Text atlas for glyph caching
struct TextAtlas {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    size: (u32, u32),
    used_space: Vec<AtlasRegion>,
}

/// Region within the text atlas
#[derive(Clone, Debug)]
struct AtlasRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    glyph_id: u64,
}

/// Glyph cache for text rendering
struct GlyphCache {
    entries: std::collections::HashMap<u64, CachedGlyph>,
    lru_order: std::collections::VecDeque<u64>,
    max_entries: usize,
}

#[derive(Clone)]
struct CachedGlyph {
    atlas_region: AtlasRegion,
    metrics: GlyphMetrics,
    last_used: std::time::Instant,
}

#[derive(Clone, Debug)]
struct GlyphMetrics {
    advance_x: f32,
    advance_y: f32,
    bearing_x: f32,
    bearing_y: f32,
}

impl WgpuRenderer {
    /// Create a new WGPU renderer instance
    pub async fn new(window: &Window) -> Result<Self> {
        let size = window.inner_size();

        // Create WGPU instance with all backends
        let instance = Instance::new(InstanceDescriptor { backends: Backends::all(), ..Default::default() });

        // Create surface from window (ephemeral; we don't store it to avoid lifetimes)
        let surface = unsafe { instance.create_surface(window).context("Failed to create WGPU surface")? };

        // Request adapter with high performance preference
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find suitable GPU adapter")?;

        // Log adapter info
        let adapter_info = adapter.get_info();
        info!("Using GPU: {} ({:?})", adapter_info.name, adapter_info.backend);

        // Create device and queue
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("OpenAgent Terminal Device"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                },
                None,
            )
            .await
            .context("Failed to create WGPU device")?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        surface.configure(&device, &config);

        // Create render pipeline
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terminal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/terminal.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Create buffers
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: 65536, // 64KB initial size
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            size: 32768, // 32KB initial size
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: 256, // Small uniform buffer
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group (placeholder for now)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[],
        });

        // Initialize text atlas
        let text_atlas = TextAtlas::new(&device, 2048, 2048);

        // Initialize glyph cache
        let glyph_cache = GlyphCache::new(1024);

        Ok(Self {
            instance,
            device,
            queue,
            config,
            size,
            window_ptr: window as *const _ as *const winit::window::Window,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            bind_group,
            text_atlas,
            glyph_cache,
            metrics: RenderMetrics::default(),
            frame_counter: 0,
            enable_performance_hud: false,
        })
    }

    /// Resize the renderer
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            // Reconfigure using a fresh surface
            let window_ref = unsafe { &*self.window_ptr };
            if let Ok(surface) = unsafe { self.instance.create_surface(window_ref) } {
                surface.configure(&self.device, &self.config);
            }
        }
    }

    /// Begin a new frame
    pub fn begin_frame(&mut self) -> Result<wgpu::SurfaceTexture> {
        let frame_start = std::time::Instant::now();

        let output = self
            .surface
            .get_current_texture()
            .context("Failed to acquire next swap chain texture")?;

        self.frame_counter += 1;

        Ok(output)
    }

    /// Render the current frame
    pub fn render(&mut self, output: &wgpu::SurfaceTexture) -> Result<()> {
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // Draw calls would go here
            self.metrics.draw_calls += 1;
        }

        // Submit command buffer
        self.queue.submit(std::iter::once(encoder.finish()));

        // Update metrics
        self.metrics.frame_time_ms = 16.67; // Placeholder

        Ok(())
    }

    /// Present the rendered frame
    pub fn present(&mut self, output: wgpu::SurfaceTexture) {
        output.present();
    }

    /// Toggle performance HUD
    pub fn toggle_performance_hud(&mut self) {
        self.enable_performance_hud = !self.enable_performance_hud;
        info!(
            "Performance HUD: {}",
            if self.enable_performance_hud { "enabled" } else { "disabled" }
        );
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> RenderMetrics {
        self.metrics.clone()
    }

    /// Clear the glyph cache
    pub fn clear_glyph_cache(&mut self) {
        self.glyph_cache.clear();
        self.text_atlas.clear();
    }
}

impl TextAtlas {
    fn new(device: &Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self { texture, texture_view, sampler, size: (width, height), used_space: Vec::new() }
    }

    fn clear(&mut self) {
        self.used_space.clear();
    }
}

impl GlyphCache {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            lru_order: std::collections::VecDeque::new(),
            max_entries,
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
    }
}

/// Renderer capabilities query
pub fn query_wgpu_support() -> bool {
    let instance = Instance::new(InstanceDescriptor { backends: Backends::all(), ..Default::default() });

    // Check if any adapters are available
    let adapters = instance.enumerate_adapters(Backends::all());

    if adapters.is_empty() {
        warn!("No WGPU adapters found");
        return false;
    }

    for adapter in adapters {
        let info = adapter.get_info();
        debug!("Found adapter: {} ({})", info.name, info.backend);
    }

    true
}
