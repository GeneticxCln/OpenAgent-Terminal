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
use crate::display::content::RenderableCell;
use openagent_terminal_core::index::Point;

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

/// Terminal cell data for rendering
#[derive(Clone, Debug)]
pub struct TerminalCell {
    pub character: char,
    pub foreground: [f32; 4],
    pub background: [f32; 4],
    pub glyph_coords: [f32; 4], // UV coordinates in atlas
}

impl TerminalCell {
    pub fn from_renderable_cell(cell: &RenderableCell, glyph_coords: [f32; 4]) -> Self {
        let fg = cell.fg.0;
        let bg = cell.bg.0;
        
        Self {
            character: cell.c,
            foreground: [
                fg[0] as f32 / 255.0,
                fg[1] as f32 / 255.0, 
                fg[2] as f32 / 255.0,
                1.0,
            ],
            background: [
                bg[0] as f32 / 255.0,
                bg[1] as f32 / 255.0,
                bg[2] as f32 / 255.0, 
                1.0,
            ],
            glyph_coords,
        }
    }
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
        
        // Create surface on demand since we don't store it
        let window_ref = unsafe { &*self.window_ptr };
        let surface = unsafe { self.instance.create_surface(window_ref) }
            .context("Failed to create surface for frame")?;
        surface.configure(&self.device, &self.config);

        let output = surface
            .get_current_texture()
            .context("Failed to acquire next swap chain texture")?;

        self.frame_counter += 1;
        self.metrics.cpu_time_ms = frame_start.elapsed().as_millis() as f32;

        Ok(output)
    }

    /// Render terminal content with the specified terminal state
    pub fn render_terminal_content(
        &mut self, 
        output: &wgpu::SurfaceTexture,
        terminal_cells: &[TerminalCell],
        cursor_pos: (u32, u32),
        viewport_size: (u32, u32)
    ) -> Result<()> {
        let render_start = std::time::Instant::now();
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Terminal Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Terminal Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Update cell data and cursor uniforms
            self.update_terminal_uniforms(terminal_cells, cursor_pos, viewport_size)?;

            // Render terminal cells
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // Draw instanced quads for terminal cells
            let cell_count = (viewport_size.0 * viewport_size.1) as u32;
            render_pass.draw_indexed(0..6, 0, 0..cell_count);
            self.metrics.draw_calls += 1;
            self.metrics.vertex_count = cell_count * 6;

            // Render performance HUD if enabled
            if self.enable_performance_hud {
                self.render_performance_hud(&mut render_pass)?;
            }
        }

        // Submit command buffer
        self.queue.submit(std::iter::once(encoder.finish()));

        // Update metrics
        self.metrics.gpu_time_ms = render_start.elapsed().as_millis() as f32;
        self.metrics.frame_time_ms = self.metrics.cpu_time_ms + self.metrics.gpu_time_ms;

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
    
    /// Update terminal uniforms with cell data and cursor position
    fn update_terminal_uniforms(
        &mut self,
        terminal_cells: &[TerminalCell],
        cursor_pos: (u32, u32),
        viewport_size: (u32, u32)
    ) -> Result<()> {
        // Update cursor uniform buffer
        let cursor_data = [
            cursor_pos.0 as f32,
            cursor_pos.1 as f32,
            1.0, // cursor width
            1.0, // cursor height
            1.0, 1.0, 1.0, 1.0, // cursor color (white)
            (self.frame_counter as f32 * 0.05).sin().abs(), // blink phase
            0.0, 0.0, 0.0, // padding
        ];
        
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&cursor_data));
        
        // TODO: Update cell data storage buffer
        // For now, we'll use the vertex buffer to store cell vertices
        
        Ok(())
    }
    
    /// Render performance HUD overlay
    fn render_performance_hud(&mut self, render_pass: &mut wgpu::RenderPass) -> Result<()> {
        // Simple HUD background quad
        let hud_vertices = [
            -0.9f32, 0.9,   // top-left
            -0.5, 0.9,      // top-right
            -0.5, 0.6,      // bottom-right
            -0.9, 0.6,      // bottom-left
        ];
        
        // Write HUD vertices to buffer (offset from main content)
        self.queue.write_buffer(&self.vertex_buffer, 4096, bytemuck::cast_slice(&hud_vertices));
        
        // Set HUD vertex buffer and draw
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(4096..));
        render_pass.draw(0..6, 0..1); // Draw HUD quad
        
        self.metrics.draw_calls += 1;
        
        debug!("Performance HUD - Frame time: {:.2}ms, GPU time: {:.2}ms, Draw calls: {}",
               self.metrics.frame_time_ms,
               self.metrics.gpu_time_ms,
               self.metrics.draw_calls);
        
        Ok(())
    }
    
    /// Cache a glyph in the text atlas
    pub fn cache_glyph(&mut self, glyph_id: u64, glyph_data: &[u8], size: (u32, u32)) -> Result<[f32; 4]> {
        // Find space in atlas
        let (x, y) = self.find_atlas_space(size.0, size.1)
            .ok_or_else(|| anyhow::anyhow!("No space in text atlas"))?;
        
        // Upload glyph data to atlas
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.text_atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            glyph_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size.0 * 4),
                rows_per_image: Some(size.1),
            },
            wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        );
        
        // Calculate UV coordinates
        let atlas_size = self.text_atlas.size;
        let uv_coords = [
            x as f32 / atlas_size.0 as f32,
            y as f32 / atlas_size.1 as f32,
            size.0 as f32 / atlas_size.0 as f32,
            size.1 as f32 / atlas_size.1 as f32,
        ];
        
        // Cache glyph metadata
        let glyph = CachedGlyph {
            atlas_region: AtlasRegion { x, y, width: size.0, height: size.1, glyph_id },
            metrics: GlyphMetrics {
                advance_x: size.0 as f32,
                advance_y: 0.0,
                bearing_x: 0.0,
                bearing_y: size.1 as f32,
            },
            last_used: std::time::Instant::now(),
        };
        
        self.glyph_cache.entries.insert(glyph_id, glyph);
        
        Ok(uv_coords)
    }
    
    /// Stage a UI sprite for rendering
    pub fn stage_ui_sprite(&mut self, sprite: crate::renderer::ui::UiSprite) -> Result<()> {
        // For now, we'll add sprites to a pending list and render them in the next frame
        // This is a simplified implementation - a full implementation would use dedicated sprite pipelines
        debug!("Staging UI sprite at ({}, {}) with size {}x{}", 
               sprite.x, sprite.y, sprite.width, sprite.height);
        Ok(())
    }
    
    /// Set sprite filter mode 
    pub fn set_sprite_filter_nearest(&mut self, nearest: bool) {
        let filter_mode = if nearest {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        };
        
        // Update the atlas sampler with new filter mode
        let new_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("text-atlas-sampler-updated"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter_mode,
            min_filter: filter_mode,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        
        self.text_atlas.sampler = new_sampler;
        debug!("Updated sprite filter mode to: {}", if nearest { "Nearest" } else { "Linear" });
    }
    
    /// Draw terminal cells using WGPU backend
    pub fn draw_cells<I: Iterator<Item = crate::display::content::RenderableCell>>(
        &mut self, 
        size_info: &crate::display::SizeInfo,
        glyph_cache: &mut crate::renderer::GlyphCache,
        cells: I
    ) -> Result<()> {
        // Convert cells to terminal cells with glyph atlas coordinates
        let terminal_cells: Vec<TerminalCell> = cells
            .map(|cell| {
                // For now, use placeholder UV coordinates - in a full implementation,
                // we'd look up the glyph in the atlas and get proper coordinates
                let glyph_coords = [0.0, 0.0, 1.0, 1.0]; // Full texture coordinates
                TerminalCell::from_renderable_cell(&cell, glyph_coords)
            })
            .collect();
            
        // Create a render texture for the current frame
        let window_ref = unsafe { &*self.window_ptr };
        let surface = unsafe { self.instance.create_surface(window_ref) }
            .context("Failed to create surface for cell rendering")?;
        surface.configure(&self.device, &self.config);
        
        let output = surface
            .get_current_texture()
            .context("Failed to get surface texture")?;
        
        // Render terminal content
        self.render_terminal_content(
            &output,
            &terminal_cells,
            (0, 0), // cursor position - should come from terminal state
            (size_info.columns() as u32, size_info.screen_lines() as u32)
        )?;
        
        // Present the frame
        output.present();
        
        debug!("Rendered {} cells with WGPU backend", terminal_cells.len());
        Ok(())
    }
    
    /// Draw a string at a specific position (for UI elements)
    pub fn draw_string(
        &mut self,
        point: openagent_terminal_core::index::Point<usize>,
        fg: crate::display::color::Rgb,
        bg: crate::display::color::Rgb, 
        string_chars: impl Iterator<Item = char>,
        size_info: &crate::display::SizeInfo,
        glyph_cache: &mut crate::renderer::GlyphCache
    ) -> Result<()> {
        // Convert the string to renderable cells
        let mut cells = Vec::new();
        for (i, character) in string_chars.enumerate() {
            let cell = crate::display::content::RenderableCell {
                point: openagent_terminal_core::index::Point::new(point.line, point.column + i),
                character,
                extra: None,
                flags: openagent_terminal_core::term::cell::Flags::empty(),
                bg_alpha: 1.0,
                fg,
                bg, 
                underline: fg,
            };
            cells.push(cell);
        }
        
        // Render using the draw_cells method
        self.draw_cells(size_info, glyph_cache, cells.into_iter())?;
        
        Ok(())
    }
    
    /// Load glyph interface for font cache integration
    pub fn with_loader<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(crate::renderer::text::LoaderApi<'_>) -> T,
    {
        // Create a dummy loader for WGPU that integrates with our atlas system
        // In a full implementation, this would create a proper LoaderApi that
        // uploads glyphs to the WGPU texture atlas
        crate::renderer::text::with_dummy_loader(func)
    }
    
    /// Stage a rounded rect for UI rendering
    pub fn stage_ui_rounded_rect(&mut self, _size_info: &crate::display::SizeInfo, rect: crate::renderer::ui::UiRoundedRect) {
        // For now, just log the rect - in a full implementation this would
        // add it to a pending UI elements buffer
        debug!("Staging UI rounded rect: {:?}", rect);
    }
    
    /// Find available space in the text atlas
    fn find_atlas_space(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        // Simple first-fit algorithm
        let atlas_size = self.text_atlas.size;
        
        for y in 0..atlas_size.1.saturating_sub(height) {
            for x in 0..atlas_size.0.saturating_sub(width) {
                if !self.is_atlas_space_occupied(x, y, width, height) {
                    // Mark space as occupied
                    self.text_atlas.used_space.push(AtlasRegion {
                        x, y, width, height, glyph_id: 0
                    });
                    return Some((x, y));
                }
            }
        }
        None
    }
    
    /// Check if atlas space is occupied
    fn is_atlas_space_occupied(&self, x: u32, y: u32, width: u32, height: u32) -> bool {
        for region in &self.text_atlas.used_space {
            let overlap_x = x < region.x + region.width && x + width > region.x;
            let overlap_y = y < region.y + region.height && y + height > region.y;
            if overlap_x && overlap_y {
                return true;
            }
        }
        false
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
