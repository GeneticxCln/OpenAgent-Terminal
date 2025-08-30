#![allow(dead_code)]

use std::borrow::Cow;
use std::cell::Cell;
use log::debug;

use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use crossfont::{BitmapBuffer, GlyphKey, Metrics, RasterizedGlyph};

use openagent_terminal_core::index::Point;

use crate::config::debug::{RendererPreference, SrgbPreference, SubpixelPreference, AtlasEvictionPolicy};
use crate::display::SizeInfo;
use crate::display::color::Rgb;
use crate::display::content::RenderableCell;

use super::rects::RenderRect;
use super::{Glyph, GlyphCache, LoaderApi, LoadGlyph};

const RECT_SHADER_WGSL: &str = r#"
struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> VsOut {
  var out: VsOut;
  out.pos = vec4<f32>(pos, 0.0, 1.0);
  out.color = color;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  return in.color;
}
"#;

const NUM_ATLAS_PAGES: u32 = 4;

const TEXT_SHADER_WGSL: &str = r#"
struct Proj {
  offset_x: f32,
  offset_y: f32,
  scale_x: f32,
  scale_y: f32,
};

@group(0) @binding(0) var<uniform> proj: Proj;
@group(0) @binding(1) var atlas: texture_2d_array<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VsIn {
  @location(0) pos: vec2<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) color: vec4<f32>,
  @location(3) flags: u32,
  @location(4) layer: u32,
};

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) flags: u32,
  @location(3) layer: u32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
  var out: VsOut;
  let ndc = vec2<f32>(proj.offset_x + in.pos.x * proj.scale_x,
                      proj.offset_y + in.pos.y * proj.scale_y);
  out.pos = vec4<f32>(ndc, 0.0, 1.0);
  out.uv = in.uv;
  out.color = in.color;
  out.flags = in.flags;
  out.layer = in.layer;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let sample = textureSample(atlas, atlas_sampler, in.uv, i32(in.layer));
  let is_colored = (in.flags & 1u) != 0u;
  let is_subpixel = (in.flags & 2u) != 0u;
  if (is_colored) {
    return sample;
  } else if (is_subpixel) {
    let a = max(sample.r, max(sample.g, sample.b)) * in.color.a;
    return vec4<f32>(in.color.rgb * sample.rgb, a);
  } else {
    let a = sample.a * in.color.a;
    return vec4<f32>(in.color.rgb, a);
  }
}
"#;

#[derive(Debug)]
pub enum Error {
    Init(String),
}

#[derive(Debug)]
pub struct WgpuRenderer {
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    // Keep a raw pointer to the winit window; we recreate the surface on demand.
    window_ptr: *const winit::window::Window,
    // Pipelines
    rect_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    // Atlas resources
    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    atlas_sampler: wgpu::Sampler,
    atlas_pages: Vec<WgpuAtlas>,
    page_meta: Vec<AtlasPageMeta>,
    current_page: u32,
    use_clock: u64,
    pending_eviction: Option<u32>,
    // Uniforms/bindings
    proj_buffer: wgpu::Buffer,
    text_bind_group: wgpu::BindGroup,
    // Preferences/state
    is_srgb_surface: bool,
    subpixel_enabled: bool,
    zero_evicted_layer: bool,
    policy: AtlasEvictionPolicy,
    // Scratch
    zero_scratch: Vec<u8>,
    // Counters
    atlas_inserts: u64,
    atlas_insert_misses: u64,
    atlas_evictions_count: u64,
    // Frame state
    pending_clear: Cell<Option<[f64; 4]>>,
    pending_text: Vec<TextVertex>,
    pending_bg: Vec<RenderRect>,
    atlas_evicted: Cell<bool>,
}

impl From<String> for Error {
    fn from(s: String) -> Self { Self::Init(s) }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    pos: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
    flags: u32,
    layer: u32,
}

#[derive(Debug, Clone, Copy)]
struct ProjParams {
    offset_x: f32,
    offset_y: f32,
    scale_x: f32,
    scale_y: f32,
}

fn projection_from_size(size: PhysicalSize<u32>) -> ProjParams {
    let w = size.width.max(1) as f32;
    let h = size.height.max(1) as f32;
    ProjParams { offset_x: -1.0, offset_y: 1.0, scale_x: 2.0 / w, scale_y: -2.0 / h }
}

#[derive(Debug, Clone, Copy)]
struct AtlasPageMeta {
    last_use: u64,
}

#[derive(Debug)]
struct WgpuAtlas {
    width: u32,
    height: u32,
    row_extent: i32,
    row_baseline: i32,
    row_tallest: i32,
    used_area: u64,
}

impl WgpuAtlas {
    fn new(size: u32) -> Self {
        Self { width: size, height: size, row_extent: 0, row_baseline: 0, row_tallest: 0, used_area: 0 }
    }

    fn clear(&mut self) {
        self.row_extent = 0;
        self.row_baseline = 0;
        self.row_tallest = 0;
        self.used_area = 0;
    }

    fn room_in_row(&self, w: i32, h: i32) -> bool {
        let next_extent = self.row_extent + w;
        let enough_width = next_extent <= self.width as i32;
        let enough_height = h < (self.height as i32 - self.row_baseline);
        enough_width && enough_height
    }

    fn advance_row(&mut self) -> bool {
        let advance_to = self.row_baseline + self.row_tallest;
        if self.height as i32 - advance_to <= 0 {
            return false;
        }
        self.row_baseline = advance_to;
        self.row_extent = 0;
        self.row_tallest = 0;
        true
    }

    fn insert(&mut self, w: i32, h: i32) -> Option<(i32, i32)> {
        if w > self.width as i32 || h > self.height as i32 {
            return None;
        }
        if !self.room_in_row(w, h) {
            if !self.advance_row() {
                return None;
            }
        }
        if !self.room_in_row(w, h) {
            return None;
        }
        let offset_x = self.row_extent;
        let offset_y = self.row_baseline;
        self.row_extent += w;
        if h > self.row_tallest {
            self.row_tallest = h;
        }
        Some((offset_x, offset_y))
    }
}

impl WgpuRenderer {
    pub async fn new(
        window_handle: &winit::window::Window,
        size: PhysicalSize<u32>,
        _renderer_preference: Option<RendererPreference>,
        srgb_pref: SrgbPreference,
        subpixel_pref: SubpixelPreference,
        zero_evicted_layer: bool,
        policy: AtlasEvictionPolicy,
    ) -> Result<Self, Error> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window_handle)
            .map_err(|e| Error::Init(format!("surface: {e}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| Error::Init(String::from("no adapter")))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wgpu-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| Error::Init(format!("device: {e}")))?;

        let surface_caps = surface.get_capabilities(&adapter);
        // Choose surface format based on preference.
        let formats = surface_caps.formats.clone();
        let pick_srgb = || formats.iter().copied().find(|f| f.is_srgb());
        let pick_non_srgb = || formats.iter().copied().find(|f| !f.is_srgb());
        let format = match srgb_pref {
            SrgbPreference::Enabled => pick_srgb().unwrap_or(formats[0]),
            SrgbPreference::Disabled => pick_non_srgb().unwrap_or(formats[0]),
            SrgbPreference::Auto => pick_srgb().unwrap_or(formats[0]),
        };
        let is_srgb_surface = format.is_srgb();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        // Resolve subpixel rendering mode based on preference and surface format.
        let subpixel_enabled = match subpixel_pref {
            SubpixelPreference::Enabled => true,
            SubpixelPreference::Disabled => false,
            SubpixelPreference::Auto => is_srgb_surface,
        };

        // Build rectangle pipeline.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(RECT_SHADER_WGSL)),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect-pipeline-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // Create text atlas resources.
        const ATLAS_SIZE: u32 = 2048;
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text-atlas"),
            size: wgpu::Extent3d { width: ATLAS_SIZE, height: ATLAS_SIZE, depth_or_array_layers: NUM_ATLAS_PAGES },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("text-atlas-view"),
            format: None,
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("text-atlas-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Projection uniform and bind group for text.
        let proj = projection_from_size(size);
        let proj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text-proj-buffer"),
            contents: bytemuck::bytes_of(&[proj.offset_x, proj.offset_y, proj.scale_x, proj.scale_y]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let text_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let text_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text-bind-group"),
            layout: &text_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: proj_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&atlas_sampler) },
            ],
        });

        // Text pipeline.
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(TEXT_SHADER_WGSL)),
        });
        let text_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text-pipeline-layout"),
            bind_group_layouts: &[&text_bgl],
            push_constant_ranges: &[],
        });
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text-pipeline"),
            layout: Some(&text_pl_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4, 3 => Uint32, 4 => Uint32],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        let mut renderer = Self {
            instance,
            device,
            queue,
            config,
            size,
            window_ptr: window_handle as *const _ as *const winit::window::Window,
            rect_pipeline,
            text_pipeline,
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_pages: (0..NUM_ATLAS_PAGES).map(|_| WgpuAtlas::new(ATLAS_SIZE)).collect(),
            page_meta: (0..NUM_ATLAS_PAGES).map(|_| AtlasPageMeta { last_use: 0 }).collect(),
            current_page: 0,
            use_clock: 1,
            pending_eviction: None,
            proj_buffer,
            text_bind_group,
            is_srgb_surface,
            subpixel_enabled,
            zero_evicted_layer: false, // set by caller via new() params below
            policy: AtlasEvictionPolicy::LruMinOccupancy, // set by caller via new()        };
        // Apply constructor preferences that depend on caller config.
        renderer.zero_evicted_layer = zero_evicted_layer;
        renderer.policy = policy;

        Ok(renderer)
    }

    pub fn resize(&mut self, size: &SizeInfo) {
        self.size = PhysicalSize::new(size.width() as u32, size.height() as u32);
        self.config.width = self.size.width.max(1);
        self.config.height = self.size.height.max(1);
        // Update projection uniform for text.
        let proj = projection_from_size(self.size);
        self.queue.write_buffer(
            &self.proj_buffer,
            0,
            bytemuck::bytes_of(&[proj.offset_x, proj.offset_y, proj.scale_x, proj.scale_y]),
        );
        // Reconfiguration will happen on the next draw when recreating the surface.
    }

    pub fn clear(&self, color: Rgb, alpha: f32) {
        // Record clear color for the next draw pass; do not present immediately.
        let r = (color.r as f32 / 255.0).min(1.0) * alpha;
        let g = (color.g as f32 / 255.0).min(1.0) * alpha;
        let b = (color.b as f32 / 255.0).min(1.0) * alpha;
        self.pending_clear.set(Some([r as f64, g as f64, b as f64, alpha as f64]));
    }

    pub fn finish(&self) {
        // No-op for wgpu; presentation happens in draw paths.
    }

    pub fn draw_rects(&mut self, size_info: &SizeInfo, _metrics: &Metrics, rects_in: Vec<RenderRect>) {
        // Acquire frame.
        // Create a fresh surface for the current frame and (re)configure it.
        let window_ref = unsafe { &*self.window_ptr };
        let surface = match self.instance.create_surface(window_ref) {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut config = self.config.clone();
        config.width = self.size.width.max(1);
        config.height = self.size.height.max(1);
        surface.configure(&self.device, &config);

        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(err) => {
                match err {
                    wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost => {
                        surface.configure(&self.device, &config);
                    },
                    wgpu::SurfaceError::OutOfMemory => return,
                    wgpu::SurfaceError::Timeout => return,
                }
                match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(_) => return,
                }
            },
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor { label: Some("frame-view"), ..Default::default() });

        // Build vertices for all rects in NDC coordinates, including staged backgrounds.
        let half_w = size_info.width() / 2.0;
        let half_h = size_info.height() / 2.0;
        let mut all_rects = Vec::with_capacity(self.pending_bg.len() + rects_in.len());
        all_rects.extend(self.pending_bg.drain(..));
        all_rects.extend(rects_in);
        let mut vertices: Vec<RectVertex> = Vec::with_capacity(all_rects.len() * 6);
        for rect in all_rects.iter() {
            let x = rect.x / half_w - 1.0;
            let y = -rect.y / half_h + 1.0;
            let w = rect.width / half_w;
            let h = rect.height / half_h;

            let a = (rect.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            let color = [rect.color.r, rect.color.g, rect.color.b, a];

            let v0 = RectVertex { pos: [x, y], color };
            let v1 = RectVertex { pos: [x, y - h], color };
            let v2 = RectVertex { pos: [x + w, y], color };
            let v3 = RectVertex { pos: [x + w, y - h], color };

            // Two triangles: (0,1,2) and (2,3,1)
            vertices.extend_from_slice(&[v0, v1, v2, v2, v3, v1]);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rects-encoder") });

        // Clear color from pending state if present.
        let clear = if let Some(c) = self.pending_clear.get() {
            self.pending_clear.set(None);
            wgpu::Color { r: c[0], g: c[1], b: c[2], a: c[3] }
        } else {
            wgpu::Color::TRANSPARENT
        };

        // Create vertex buffer before beginning the pass so it outlives the pass borrow.
        let vbuf_opt = (!vertices.is_empty()).then(|| {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rects-vertex-buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rects-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(ref vbuf) = vbuf_opt {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
        }

        // Draw staged text after rects, if any.
        if !self.pending_text.is_empty() {
            let text_vbuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("text-vertex-buffer"),
                    contents: bytemuck::cast_slice(&self.pending_text),
                    usage: wgpu::BufferUsages::VERTEX,
                });

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("text-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.text_pipeline);
                pass.set_bind_group(0, &self.text_bind_group, &[]);
                pass.set_vertex_buffer(0, text_vbuf.slice(..));
                pass.draw(0..self.pending_text.len() as u32, 0..1);
            }
            self.pending_text.clear();
        }

        self.queue.submit([encoder.finish()]);
        frame.present();
    }

    pub fn draw_cells<I: Iterator<Item = RenderableCell>>(
        &mut self,
        size_info: &SizeInfo,
        glyph_cache: &mut GlyphCache,
        cells: I,
    ) {
        // Stage text vertices to render on the next draw_rects pass.
        let mut loader = WgpuGlyphLoader { renderer: self };
        let mut staged: Vec<TextVertex> = Vec::new();
        let mut staged_bg: Vec<RenderRect> = Vec::new();

        for mut cell in cells {
            // Stage full-cell background quad first.
            if cell.bg_alpha > 0.0 {
                let x = cell.point.column.0 as f32 * size_info.cell_width() + size_info.padding_x();
                let y = cell.point.line as f32 * size_info.cell_height() + size_info.padding_y();
                staged_bg.push(RenderRect::new(
                    x,
                    y,
                    size_info.cell_width(),
                    size_info.cell_height(),
                    cell.bg,
                    cell.bg_alpha as f32,
                ));
            }
            // Skip hidden or tab cells by rendering as space.
            let hidden = cell.flags.contains(openagent_terminal_core::term::cell::Flags::HIDDEN);
            if cell.character == '\t' || hidden {
                cell.character = ' ';
            }

            // Select font based on style flags.
            let font_key = match cell.flags & openagent_terminal_core::term::cell::Flags::BOLD_ITALIC {
                openagent_terminal_core::term::cell::Flags::BOLD_ITALIC => glyph_cache.bold_italic_key,
                openagent_terminal_core::term::cell::Flags::ITALIC => glyph_cache.italic_key,
                openagent_terminal_core::term::cell::Flags::BOLD => glyph_cache.bold_key,
                _ => glyph_cache.font_key,
            };

            // Primary glyph.
            let glyph_key = GlyphKey { font_key, size: glyph_cache.font_size, character: cell.character };
            let g = glyph_cache.get(glyph_key, &mut loader, true);
            staged.extend_from_slice(&build_text_vertices(size_info, &cell, &g, self.subpixel_enabled));

            // Zero-width characters.
            if let Some(zw) = cell
                .extra
                .as_mut()
                .and_then(|extra| extra.zerowidth.take().filter(|_| !hidden))
            {
                let mut key = glyph_key;
                for ch in zw {
                    key.character = ch;
                    let gzw = glyph_cache.get(key, &mut loader, false);
                    staged.extend_from_slice(&build_text_vertices(size_info, &cell, &gzw, self.subpixel_enabled));
                }
            }
        }

        self.pending_text.extend(staged);
        self.pending_bg.extend(staged_bg);
    }

    pub fn draw_string(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        _bg: Rgb,
        string_chars: impl Iterator<Item = char>,
        size_info: &SizeInfo,
        glyph_cache: &mut GlyphCache,
    ) {
        // Minimal implementation: render string via staged text path.
        let mut loader = WgpuGlyphLoader { renderer: self };
        let mut col = point.column.0;
        let mut staged: Vec<TextVertex> = Vec::new();
        let mut staged_bg: Vec<RenderRect> = Vec::new();
        for ch in string_chars {
            let glyph_key = GlyphKey { font_key: glyph_cache.font_key, size: glyph_cache.font_size, character: ch };
            let mut cell = RenderableCell {
                point: Point::new(point.line, openagent_terminal_core::index::Column(col)),
                character: ch,
                extra: None,
                flags: openagent_terminal_core::term::cell::Flags::empty(),
                bg_alpha: 1.0,
                fg,
                bg: _bg,
                underline: fg,
            };
            // Background for draw_string cells (solid).
            let x = cell.point.column.0 as f32 * size_info.cell_width() + size_info.padding_x();
            let y = cell.point.line as f32 * size_info.cell_height() + size_info.padding_y();
            staged_bg.push(RenderRect::new(
                x,
                y,
                size_info.cell_width(),
                size_info.cell_height(),
                cell.bg,
                1.0,
            ));
            let g = glyph_cache.get(glyph_key, &mut loader, true);
            staged.extend_from_slice(&build_text_vertices(size_info, &cell, &g, self.subpixel_enabled));
            col += 1;
        }
        self.pending_text.extend(staged);
        self.pending_bg.extend(staged_bg);
    }

    pub fn with_loader<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(LoaderApi<'_>) -> T,
    {
        // Not applicable for WGPU text; fall back to a dummy loader to keep code paths working.
        super::text::with_dummy_loader(func)
    }

    pub fn take_atlas_evicted(&self) -> bool {
        let ev = self.atlas_evicted.get();
        if ev { self.atlas_evicted.set(false); }
        ev
    }

    pub fn reset_atlas(&mut self) {
        for page in &mut self.atlas_pages { page.clear(); }
        for meta in &mut self.page_meta { meta.last_use = 0; }
        self.current_page = 0;
        self.pending_eviction = None;
        // Optionally we could zero the atlas texture, but new uploads will overwrite as needed.
    }

    /// Clear a single pending eviction page if any. Returns true if a page was cleared.
    pub fn evict_one_page(&mut self) -> bool {
        if let Some(layer) = self.pending_eviction.take() {
            // Debug stats before clearing.
            let page = &self.atlas_pages[layer as usize];
            let capacity = (page.width as u64) * (page.height as u64);
            let used = page.used_area.min(capacity);
            let pct = if capacity > 0 { (used as f64 / capacity as f64) * 100.0 } else { 0.0 };
            debug!(
                "WGPU atlas eviction: layer={} used={} / {} ({:.1}%), policy={:?}, counters: inserts={}, misses={}, evictions={}",
                layer,
                used,
                capacity,
                pct,
                self.policy,
                self.atlas_inserts,
                self.atlas_insert_misses,
                self.atlas_evictions_count + 1
            );

            // Clear CPU state.
            if let Some(page_mut) = self.atlas_pages.get_mut(layer as usize) {
                page_mut.clear();
            }
            if let Some(meta) = self.page_meta.get_mut(layer as usize) {
                meta.last_use = 0;
            }

            // Optionally clear GPU layer to zeros (cosmetic).
            if self.zero_evicted_layer {
                let width = self.atlas_pages[0].width;
                let height = self.atlas_pages[0].height;
                let extent = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
                self.queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &self.atlas_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &self.zero_scratch,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width),
                        rows_per_image: Some(height),
                    },
                    extent,
                );
            }

            self.current_page = layer;
            self.atlas_evictions_count = self.atlas_evictions_count.wrapping_add(1);
            return true;
        }
        false
    }

    pub fn was_context_reset(&self) -> bool { false }
    pub fn set_viewport(&self, _size: &SizeInfo) {}
}

fn build_text_vertices(size_info: &SizeInfo, cell: &RenderableCell, glyph: &Glyph, subpixel: bool) -> [TextVertex; 6] {
    let cell_x = cell.point.column.0 as f32 * size_info.cell_width() + size_info.padding_x();
    let gx = cell_x + glyph.left as f32;
    let gy = (cell.point.line + 1) as f32 * size_info.cell_height() + size_info.padding_y() - glyph.top as f32;

    let x0 = gx;
    let y0 = gy - glyph.height as f32;
    let x1 = gx + glyph.width as f32;
    let y1 = gy;

    let u0 = glyph.uv_left;
    let v0 = glyph.uv_bot;
    let u1 = u0 + glyph.uv_width;
    let v1 = v0 + glyph.uv_height;

    let color = [cell.fg.r, cell.fg.g, cell.fg.b, 255];
    let mut flags = if glyph.multicolor { 1u32 } else { 0u32 };
    // Enable subpixel path only if configured.
    if subpixel { flags |= 2u32; }

    let layer = if glyph.tex_id > 0 { glyph.tex_id - 1 } else { 0 };

    [
        TextVertex { pos: [x0, y0], uv: [u0, v0], color, flags, layer },
        TextVertex { pos: [x0, y1], uv: [u0, v1], color, flags, layer },
        TextVertex { pos: [x1, y0], uv: [u1, v0], color, flags, layer },
        TextVertex { pos: [x1, y0], uv: [u1, v0], color, flags, layer },
        TextVertex { pos: [x1, y1], uv: [u1, v1], color, flags, layer },
        TextVertex { pos: [x0, y1], uv: [u0, v1], color, flags, layer },
    ]
}

struct WgpuGlyphLoader<'a> {
    renderer: &'a mut WgpuRenderer,
}

impl LoadGlyph for WgpuGlyphLoader<'_> {
    fn load_glyph(&mut self, rasterized: &RasterizedGlyph) -> Glyph {
        // Insert into atlas, uploading to GPU.
        let w = rasterized.width as i32;
        let h = rasterized.height as i32;
        // Choose a page with space, starting at current_page.
        let mut chosen: Option<(u32, i32, i32)> = None;
        for i in 0..NUM_ATLAS_PAGES {
            let page = ((self.renderer.current_page + i) % NUM_ATLAS_PAGES) as usize;
            if let Some((ox, oy)) = self.renderer.atlas_pages[page].insert(w, h) {
                self.renderer.current_page = page as u32;
                // Update LRU metadata.
                let ts = self.renderer.use_clock;
                self.renderer.use_clock = self.renderer.use_clock.wrapping_add(1);
                if let Some(meta) = self.renderer.page_meta.get_mut(page) {
                    meta.last_use = ts;
                }
                if let Some(page_mut) = self.renderer.atlas_pages.get_mut(page) {
                    page_mut.used_area = page_mut.used_area.saturating_add((w as u64) * (h as u64));
                }
                self.renderer.atlas_inserts = self.renderer.atlas_inserts.wrapping_add(1);
                chosen = Some((page as u32, ox, oy));
                break;
            }
        }
        let (page_idx, ox, oy) = match chosen {
            Some(v) => v,
            None => {
                // Select a victim page based on policy.
                let victim_idx: u32 = match self.renderer.policy {
                    AtlasEvictionPolicy::RoundRobin => (self.renderer.current_page + 1) % NUM_ATLAS_PAGES,
                    AtlasEvictionPolicy::LruMinOccupancy => {
                        let mut best_i: u32 = 0;
                        let mut best_key = (u64::MAX, u64::MAX);
                        for i in 0..(NUM_ATLAS_PAGES as usize) {
                            let ts = self.renderer.page_meta[i].last_use;
                            let used = self.renderer.atlas_pages[i].used_area;
                            let key = (ts, used);
                            if key < best_key {
                                best_key = key;
                                best_i = i as u32;
                            }
                        }
                        best_i
                    }
                };
                // Request eviction of the victim page on the next frame.
                self.renderer.pending_eviction.get_or_insert(victim_idx);
                self.renderer.atlas_evicted.set(true);
                self.renderer.atlas_insert_misses = self.renderer.atlas_insert_misses.wrapping_add(1);
                return Glyph {
                    tex_id: 0,
                    multicolor: false,
                    top: rasterized.top as i16,
                    left: rasterized.left as i16,
                    width: 0,
                    height: 0,
                    uv_bot: 0.0,
                    uv_left: 0.0,
                    uv_width: 0.0,
                    uv_height: 0.0,
                };
            },
        };

        // Prepare pixel data (RGBA8). For RGB, store alpha in A and zero RGB.
        let (rgba, multicolor) = match &rasterized.buffer {
            BitmapBuffer::Rgba(buf) => (buf.clone(), true),
            BitmapBuffer::Rgb(buf) => {
                let mut out = Vec::with_capacity((rasterized.width * rasterized.height * 4) as usize);
                for chunk in buf.chunks_exact(3) {
                    // Use red channel as alpha; set RGB to 0.
                    let a = chunk[0];
                    out.extend_from_slice(&[0, 0, 0, a]);
                }
                (out, false)
            },
        };

        // Upload the glyph into the atlas texture.
        let extent = wgpu::Extent3d { width: rasterized.width as u32, height: rasterized.height as u32, depth_or_array_layers: 1 };
        self.renderer.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.renderer.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: ox as u32, y: oy as u32, z: page_idx },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * rasterized.width as u32),
                rows_per_image: Some(rasterized.height as u32),
            },
            extent,
        );

        // UVs normalized (top-left origin).
        // Use the dimensions of the first page for UV normalization (all pages share size).
        let page_dims = &self.renderer.atlas_pages[0];
        let u0 = ox as f32 / page_dims.width as f32;
        let v0 = oy as f32 / page_dims.height as f32;
        let u1 = (ox + rasterized.width as i32) as f32 / page_dims.width as f32;
        let v1 = (oy + rasterized.height as i32) as f32 / page_dims.height as f32;

        Glyph {
            tex_id: page_idx + 1,
            multicolor,
            top: rasterized.top as i16,
            left: rasterized.left as i16,
            width: rasterized.width as i16,
            height: rasterized.height as i16,
            uv_bot: v0,
            uv_left: u0,
            uv_width: u1 - u0,
            uv_height: v1 - v0,
        }
    }

    fn clear(&mut self) {
        for page in &mut self.renderer.atlas_pages {
            page.clear();
        }
        self.renderer.current_page = 0;
    }
}
