#![allow(dead_code)]

use log::debug;
use std::borrow::Cow;
use std::cell::Cell;

use crate::renderer::wgpu_rect_transfer::WgpuRectTransfer;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use crossfont::{BitmapBuffer, GlyphKey, Metrics, RasterizedGlyph};

use openagent_terminal_core::index::Point;

use crate::config::debug::{
    AtlasEvictionPolicy, RendererPreference, SrgbPreference, SubpixelOrientation,
    SubpixelPreference,
};
use crate::display::color::Rgb;
use crate::display::content::RenderableCell;
use crate::display::SizeInfo;

use super::rects::RenderRect;
use super::text::glyph_cache::Glyph;
use super::ui::{UiRoundedRect, UiSprite};
use super::{GlyphCache, LoadGlyph, LoaderApi};

const RECT_SHADER_WGSL: &str = r#"
// Uniforms for rect/underline rendering
struct RectUniforms {
  cell_size: vec2<f32>,        // (cellWidth, cellHeight)
  padding: vec2<f32>,          // (paddingX, paddingY)
  underline_position: f32,     // distance from baseline to underline center (pixels)
  underline_thickness: f32,    // thickness in pixels
  undercurl_position: f32,     // amplitude parameter for undercurl (approx half descent)
  _pad: f32,
};
@group(0) @binding(0) var<uniform> ru: RectUniforms;

struct VsOutV {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) kind: u32,
};

struct FsIn {
  @location(0) color: vec4<f32>,
  @location(1) kind: u32,
};

@vertex
fn vs_main(@location(0) pos: vec2<f32>,
           @location(1) color: vec4<f32>,
           @location(2) kind: u32) -> VsOutV {
  var out: VsOutV;
  out.pos = vec4<f32>(pos, 0.0, 1.0);
  out.color = color;
  out.kind = kind;
  // Pack pixel-space position into a separate varyings buffer via an additional vertex buffer attribute is more complex;
  // Instead, we rely on the fragment input struct carrying frag_xy via location(2);
  // However WGSL requires vertex outputs to feed fragment inputs. Since we use a single VS output struct with builtin only,
  // we will emulate by reconstructing frag_xy in FS from clip position and uniform scale below.
  return out;
}

fn fmod(a: f32, b: f32) -> f32 {
// Implement fmod-like behavior: a - b * floor(a / b)
  return a - b * floor(a / b);
}

fn draw_undercurl(x: f32, y: f32, color: vec4<f32>) -> vec4<f32> {
  // Use cos wave with amplitude based on undercurl_position
  let pi = 3.1415926538;
  let undercurl = ru.undercurl_position / 2.0 * cos((x + 0.5) * 2.0 * pi / ru.cell_size.x)
                + ru.undercurl_position - 1.0;
  let top = undercurl + max((ru.underline_thickness - 1.0), 0.0) / 2.0;
  let bottom = undercurl - max((ru.underline_thickness - 1.0), 0.0) / 2.0;
  // Distance from curve boundary; keep positive for AA mask
  let dst = max(y - top, max(bottom - y, 0.0));
  // Simple AA-like falloff to preserve thickness
  let alpha = max(0.0, 1.0 - dst * dst);
  return vec4<f32>(color.rgb, alpha);
}

fn draw_dotted_single_px(x: f32, y: f32, color: vec4<f32>, frag_pos: vec4<f32>) -> vec4<f32> {
  var cell_even: f32 = 0.0;
  if (i32(ru.cell_size.x) % 2 != 0) {
    cell_even = f32(i32(floor((frag_pos.x - ru.padding.x) / ru.cell_size.x)) % 2);
  }
  var alpha = 1.0 - abs(floor(ru.underline_position) - y);
  if (i32(x) % 2 != i32(cell_even)) {
    alpha = 0.0;
  }
  return vec4<f32>(color.rgb, alpha);
}

fn draw_dotted_aa(x: f32, y: f32, color: vec4<f32>) -> vec4<f32> {
  let dot_number = floor(x / ru.underline_thickness);
  let radius = ru.underline_thickness / 2.0;
  let center_y = ru.underline_position - 1.0;
  let left_center = (dot_number - (dot_number % 2.0)) * ru.underline_thickness + radius;
  let right_center = left_center + 2.0 * ru.underline_thickness;
  let dist_left = distance(vec2<f32>(x, y), vec2<f32>(left_center, center_y));
  let dist_right = distance(vec2<f32>(x, y), vec2<f32>(right_center, center_y));
  let d = min(dist_left, dist_right);
  let alpha = max(0.0, 1.0 - (d - radius));
  return vec4<f32>(color.rgb, alpha);
}

fn draw_dashed(x: f32, color: vec4<f32>) -> vec4<f32> {
  let half_dash = floor(ru.cell_size.x / 4.0 + 0.5);
  var alpha = 1.0;
  if (x > half_dash - 1.0 && x < ru.cell_size.x - half_dash) {
    alpha = 0.0;
  }
  return vec4<f32>(color.rgb, alpha);
}

@fragment
fn fs_main(in: FsIn, @builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
  // Compute pixel coordinates within a cell (x,y) using built-in position
  // Note: frag_pos is window-space in pixels for most backends; under Xvfb it may be undefined but we only run this on real GPUs
  let x = floor(fmod(frag_pos.x - ru.padding.x, ru.cell_size.x));
  let y = floor(fmod(frag_pos.y - ru.padding.y, ru.cell_size.y));

  switch (in.kind) {
    case 1u: { // undercurl
      let col = draw_undercurl(x, y, in.color);
      return vec4<f32>(col.rgb, col.a * in.color.a);
    }
    case 2u: { // dotted underline
      if (ru.underline_thickness < 2.0) {
        let col = draw_dotted_single_px(x, y, in.color, frag_pos);
        return vec4<f32>(col.rgb, col.a * in.color.a);
      } else {
        let col = draw_dotted_aa(x, y, in.color);
        return vec4<f32>(col.rgb, col.a * in.color.a);
      }
    }
    case 3u: { // dashed underline
      let col = draw_dashed(x, in.color);
      return vec4<f32>(col.rgb, col.a * in.color.a);
    }
    default: {
      return in.color;
    }
  }
}
"#;

const NUM_ATLAS_PAGES: u32 = 4;

const UI_SHADER_WGSL: &str = r#"
struct VsIn {
  @location(0) pos: vec2<f32>,
  @location(1) origin: vec2<f32>,
  @location(2) size: vec2<f32>,
  @location(3) radius: f32,
  @location(4) color: vec4<f32>,
};

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) origin: vec2<f32>,
  @location(1) size: vec2<f32>,
  @location(2) radius: f32,
  @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
  var out: VsOut;
  out.pos = vec4<f32>(in.pos, 0.0, 1.0);
  out.origin = in.origin;
  out.size = in.size;
  out.radius = in.radius;
  out.color = in.color;
  return out;
}

fn sdRoundedBox(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
  let q = abs(p) - (b - vec2<f32>(r, r));
  return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let center = in.origin + in.size * 0.5;
  let p = in.pos.xy - center;
  let half_size = in.size * 0.5;
  let d = sdRoundedBox(p, half_size, in.radius);
  let aa = fwidth(d);
  let alpha = smoothstep(0.0, -aa, d);
  return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;

const TEXT_SHADER_WGSL: &str = r#"
struct Proj {
  offset_x: f32,
  offset_y: f32,
  scale_x: f32,
  scale_y: f32,
  gamma: f32,
};

fn to_lin_gamma(u: f32, gamma: f32) -> f32 {
  if (u <= 0.04045) { return u / 12.92; }
  return pow((u + 0.055) / 1.055, gamma);
}

fn srgb_to_linear_gamma(c: vec3<f32>, gamma: f32) -> vec3<f32> {
  return vec3<f32>(to_lin_gamma(c.r, gamma), to_lin_gamma(c.g, gamma), to_lin_gamma(c.b, gamma));
}

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
  let s = textureSample(atlas, atlas_sampler, in.uv, i32(in.layer));
  let is_colored = (in.flags & 1u) != 0u;
  let subpixel = (in.flags & 2u) != 0u;
  let bgr = (in.flags & 4u) != 0u;

  let fg_lin = srgb_to_linear_gamma(in.color.rgb, proj.gamma);

  // Keep multicolor glyphs (emoji/color fonts) as-is
  if (is_colored) {
    return s;
  }

  if (subpixel) {
    // Approximate LCD subpixel rendering by sampling alpha at 3 horizontal subpixel offsets.
    // Map one screen pixel to UV delta using derivatives.
    let du = abs(dpdx(in.uv).x);
    if (du > 0.0) {
      let off = du / 3.0;
      let a_l = textureSample(atlas, atlas_sampler, in.uv + vec2<f32>(-off, 0.0), i32(in.layer)).a;
      let a_m = s.a;
      let a_r = textureSample(atlas, atlas_sampler, in.uv + vec2<f32>(off, 0.0), i32(in.layer)).a;
      var cov = vec3<f32>(a_l, a_m, a_r);
      if (bgr) { cov = vec3<f32>(a_r, a_m, a_l); }
      let out_rgb_lin = fg_lin * cov;
      let out_alpha = max(max(cov.r, cov.g), cov.b) * in.color.a;
      return vec4<f32>(out_rgb_lin, out_alpha);
    }
  }

  // Grayscale fallback: use alpha coverage tinted with foreground color.
  let coverage = s.a;
  let out_alpha = coverage * in.color.a;
  return vec4<f32>(fg_lin, out_alpha);
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
    // Pipelines
    rect_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    // Sprite pipeline/resources
    sprite_pipeline: wgpu::RenderPipeline,
    sprite_texture: wgpu::Texture,
    sprite_view: wgpu::TextureView,
    sprite_sampler_linear: wgpu::Sampler,
    sprite_sampler_nearest: wgpu::Sampler,
    sprite_bind_group_linear: wgpu::BindGroup,
    sprite_bind_group_nearest: wgpu::BindGroup,
    // Rect uniforms/bindings
    rect_uniform_buffer: wgpu::Buffer,
    rect_bind_group: wgpu::BindGroup,
    // Batched rect transfer helper
    rect_transfer: WgpuRectTransfer,
    // Persistent CPU-side rect vertex buffer to avoid per-frame allocations
    rect_vertices_cpu: Vec<RectVertex>,
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
    subpixel_bgr: bool,
    subpixel_gamma: f32,
    perf_hud_enabled: bool,
    // Last-frame stats
    last_frame_ms: f32,
    last_draw_calls: u32,
    last_vertices: u32,
    // Per-frame metrics
    metrics: PerformanceMetrics,
    zero_evicted_layer: bool,
    policy: AtlasEvictionPolicy,
    // Scratch
    zero_scratch: Vec<u8>,
    // Counters
    atlas_inserts: u64,
    atlas_insert_misses: u64,
    atlas_evictions_count: u64,
    // Reporting
    report_interval_frames: u32,
    renderer_report_interval_frames: u32,
    frame_counter: u64,
    // Frame state
    pending_clear: Cell<Option<[f64; 4]>>,
    pending_text: Vec<TextVertex>,
    pending_bg: Vec<RenderRect>,
    pending_ui: Vec<UiVertex>,
    // Sprite staging (two queues for filter override)
    pending_sprites_linear: Vec<SpriteVertex>,
    pending_sprites_nearest: Vec<SpriteVertex>,
    sprite_default_filter_nearest: bool,
    // Perf history (last N frame times)
    perf_history: Vec<f32>,
    atlas_evicted: Cell<bool>,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct PerformanceMetrics {
    pub draw_calls: u32,
    pub vertices_submitted: u32,
    pub rect_bytes_copied: u64,
    pub rect_flush_count: u32,
    pub primitives_batched: u32,
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Init(s)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    pos: [f32; 2],
    color: [u8; 4],
    kind: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct UiVertex {
    pos: [f32; 2],
    origin: [f32; 2],
    size: [f32; 2],
    radius: f32,
    color: [f32; 4],
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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    tint: [f32; 4],
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
    ProjParams {
        offset_x: -1.0,
        offset_y: 1.0,
        scale_x: 2.0 / w,
        scale_y: -2.0 / h,
    }
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
        Self {
            width: size,
            height: size,
            row_extent: 0,
            row_baseline: 0,
            row_tallest: 0,
            used_area: 0,
        }
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
        if !self.room_in_row(w, h) && !self.advance_row() {
            return None;
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
    /// Preload common ASCII glyphs into the WGPU atlas and cache.
    pub fn preload_glyphs(&mut self, glyph_cache: &mut GlyphCache) {
        let mut loader = WgpuGlyphLoader { renderer: self };
        glyph_cache.reset_glyph_cache(&mut loader);
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        window_handle: &winit::window::Window,
        size: PhysicalSize<u32>,
        _renderer_preference: Option<RendererPreference>,
        _srgb_pref: SrgbPreference,
        subpixel_pref: SubpixelPreference,
        subpixel_orientation: SubpixelOrientation,
        zero_evicted_layer: bool,
        policy: AtlasEvictionPolicy,
        atlas_report_interval_frames: u32,
        renderer_report_interval_frames: u32,
    ) -> Result<Self, Error> {
        // Prefer Vulkan backend explicitly. This build is WGPU-only; no other graphics API fallback.
        // changed later.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
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
            .map_err(|e| Error::Init(format!("adapter: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("wgpu-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .map_err(|e| Error::Init(format!("device: {e}")))?;

        let surface_caps = surface.get_capabilities(&adapter);
        // Choose surface format based on preference.
        let formats = surface_caps.formats.clone();
        let pick_non_srgb = || formats.iter().copied().find(|f| !f.is_srgb());
        // Force non-sRGB format for reliable blending/gamma on some Wayland setups.
        let format = pick_non_srgb().unwrap_or(formats[0]);
        let is_srgb_surface = false;

        // Prefer vsync-capable present modes when available to avoid tearing.
        let present_mode = if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::AutoVsync)
        {
            wgpu::PresentMode::AutoVsync
        } else if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Fifo)
        {
            wgpu::PresentMode::Fifo
        } else {
            surface_caps.present_modes[0]
        };
        // Prefer opaque alpha mode to avoid compositor blending artifacts.
        let alpha_mode = if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            wgpu::CompositeAlphaMode::Opaque
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Auto)
        {
            wgpu::CompositeAlphaMode::Auto
        } else {
            surface_caps.alpha_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        // Initial configure; surface is recreated per-draw, so this is a sanity check only.
        surface.configure(&device, &config);

        // Resolve subpixel rendering mode based on preference and surface format.
        let subpixel_enabled = match subpixel_pref {
            SubpixelPreference::Enabled => true,
            SubpixelPreference::Disabled => false,
            SubpixelPreference::Auto => is_srgb_surface,
        };
        let subpixel_bgr = matches!(subpixel_orientation, SubpixelOrientation::BGR);

        // Build rectangle pipeline.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(RECT_SHADER_WGSL)),
        });
        // Rect uniforms/bindings
        let rect_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rect-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        // Initialize with zeros; will be populated per-draw
        let rect_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect-uniform-buffer"),
            size: 32, // 8 f32 values (2 vec2 + 4 scalars) = 32 bytes
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rect_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect-bind-group"),
            layout: &rect_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: rect_uniform_buffer.as_entire_binding(),
            }],
        });
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(UI_SHADER_WGSL)),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect-pipeline-layout"),
            bind_group_layouts: &[&rect_bgl],
            push_constant_ranges: &[],
        });
        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("rect-pipeline"),
            layout: Some(&pipeline_layout),
vertex: wgpu::VertexState {
                module: &shader,
                compilation_options: Default::default(),
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4, 2 => Uint32],
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
                compilation_options: Default::default(),
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // UI pipeline (rounded rects)
        let ui_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui-pipeline-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("ui-pipeline"),
            layout: Some(&ui_pipeline_layout),
vertex: wgpu::VertexState {
                module: &ui_shader,
                compilation_options: Default::default(),
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32, 4 => Float32x4],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                compilation_options: Default::default(),
                entry_point: Some("fs_main"),
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
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: NUM_ATLAS_PAGES,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("text-atlas-view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
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
            contents: bytemuck::bytes_of(&[
                proj.offset_x,
                proj.offset_y,
                proj.scale_x,
                proj.scale_y,
                2.2f32,
            ]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let text_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: proj_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
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
            cache: None,
            label: Some("text-pipeline"),
            layout: Some(&text_pl_layout),
vertex: wgpu::VertexState {
                module: &text_shader,
                compilation_options: Default::default(),
                entry_point: Some("vs_main"),
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
                compilation_options: Default::default(),
                entry_point: Some("fs_main"),
targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    // Enable standard alpha blending for text rendering.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // Prepare zero scratch buffer for optional layer clearing.
        let zero_scratch = vec![0u8; (ATLAS_SIZE as usize) * (ATLAS_SIZE as usize) * 4];

        // Create a rect transfer helper (staging+vertex) for batched uploads.
        let initial_rect_vb_capacity_vertices: usize = 64 * 1024; // 64k vertices
        let rect_transfer = WgpuRectTransfer::new(
            &device,
            initial_rect_vb_capacity_vertices,
            std::mem::size_of::<RectVertex>(),
        );

        // --- Sprite pipeline & resources ---
        // Minimal sprite shader (textured quad with tint)
        let sprite_shader_src: &str = r#"
struct VsIn {
  @location(0) pos: vec2<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) tint: vec4<f32>,
};
struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) tint: vec4<f32>,
};
@vertex
fn vs_main(in: VsIn) -> VsOut {
  var out: VsOut;
  out.pos = vec4<f32>(in.pos, 0.0, 1.0);
  out.uv = in.uv;
  out.tint = in.tint;
  return out;
}
@group(0) @binding(0) var sprite_tex: texture_2d<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let tex = textureSample(sprite_tex, sprite_sampler, in.uv);
  return vec4<f32>(tex.rgb * in.tint.rgb, tex.a * in.tint.a);
}
"#;
        let sprite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(sprite_shader_src)),
        });
        let sprite_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sprite_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite-pl"),
            bind_group_layouts: &[&sprite_bgl],
            push_constant_ranges: &[],
        });
        let sprite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    cache: None,
    label: Some("sprite-pipeline"),
    layout: Some(&sprite_pl_layout),
    vertex: wgpu::VertexState {
        module: &sprite_shader,
        compilation_options: Default::default(),
        entry_point: Some("vs_main"),
        buffers: &[wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
        }],
    },
    primitive: wgpu::PrimitiveState::default(),
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    fragment: Some(wgpu::FragmentState {
        module: &sprite_shader,
        compilation_options: Default::default(),
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
            format: config.format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
    }),
    multiview: None,
});
        // Build a tiny 2x2 checkerboard as the initial sprite atlas (no external files)
        let sprite_tex_size = wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        };
        let sprite_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite-texture"),
            size: sprite_tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let sprite_view = sprite_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sprite_sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite-sampler-linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let sprite_sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite-sampler-nearest"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let sprite_bind_group_linear = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite-bg-linear"),
            layout: &sprite_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sprite_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sprite_sampler_linear),
                },
            ],
        });
        let sprite_bind_group_nearest = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite-bg-nearest"),
            layout: &sprite_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sprite_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sprite_sampler_nearest),
                },
            ],
        });
        // Upload checkerboard pixels
        let checker: [u8; 16] = [
            255, 255, 255, 255, 32, 32, 32, 255, 32, 32, 32, 255, 255, 255, 255, 255,
        ];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &sprite_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &checker,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * 2),
                rows_per_image: Some(2),
            },
            sprite_tex_size,
        );

        let mut renderer = Self {
            instance,
            device,
            queue,
            config,
            size,
            rect_pipeline,
            ui_pipeline,
            text_pipeline,
            sprite_pipeline,
            sprite_texture,
            sprite_view,
            sprite_sampler_linear,
            sprite_sampler_nearest,
            sprite_bind_group_linear,
            sprite_bind_group_nearest,
            rect_uniform_buffer,
            rect_bind_group,
            rect_transfer,
            rect_vertices_cpu: Vec::new(),
            atlas_texture,
            atlas_view,
            atlas_sampler,
            atlas_pages: (0..NUM_ATLAS_PAGES)
                .map(|_| WgpuAtlas::new(ATLAS_SIZE))
                .collect(),
            page_meta: (0..NUM_ATLAS_PAGES)
                .map(|_| AtlasPageMeta { last_use: 0 })
                .collect(),
            current_page: 0,
            use_clock: 1,
            pending_eviction: None,
            proj_buffer,
            text_bind_group,
            is_srgb_surface,
            subpixel_enabled,
            subpixel_bgr,
            subpixel_gamma: 2.2,
            perf_hud_enabled: false,
            // Init last-frame stats
            last_frame_ms: 0.0,
            last_draw_calls: 0,
            last_vertices: 0,
            metrics: PerformanceMetrics::default(),
            zero_evicted_layer: false,                    // set below
            policy: AtlasEvictionPolicy::LruMinOccupancy, // set below
            zero_scratch,
            atlas_inserts: 0,
            atlas_insert_misses: 0,
            atlas_evictions_count: 0,
            report_interval_frames: 0,
            renderer_report_interval_frames: 0,
            frame_counter: 0,
            pending_clear: Cell::new(None),
            pending_text: Vec::new(),
            pending_bg: Vec::new(),
            pending_ui: Vec::new(),
            pending_sprites_linear: Vec::new(),
            pending_sprites_nearest: Vec::new(),
            sprite_default_filter_nearest: false,
            perf_history: Vec::new(),
            atlas_evicted: Cell::new(false),
        };
        renderer.zero_evicted_layer = zero_evicted_layer;
        renderer.policy = policy;
        renderer.report_interval_frames = atlas_report_interval_frames;
        renderer.renderer_report_interval_frames = renderer_report_interval_frames;

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
            bytemuck::bytes_of(&[
                proj.offset_x,
                proj.offset_y,
                proj.scale_x,
                proj.scale_y,
                self.subpixel_gamma,
            ]),
        );
    }

    pub fn clear(&self, color: Rgb, _alpha: f32) {
        // Force opaque clear to avoid compositor transparency interactions.
        let r = (color.r as f32 / 255.0).min(1.0);
        let g = (color.g as f32 / 255.0).min(1.0);
        let b = (color.b as f32 / 255.0).min(1.0);
        self.pending_clear
            .set(Some([r as f64, g as f64, b as f64, 1.0]));
    }

    pub fn finish(&self) {
        // No-op for wgpu; presentation happens in draw paths.
    }

    pub fn metrics(&self) -> PerformanceMetrics {
        self.metrics
    }

    pub fn last_frame_ms(&self) -> f32 {
        self.last_frame_ms
    }

    pub fn record_frame_time(&mut self, ms: f32) {
        self.last_frame_ms = ms;
        self.perf_history.push(ms);
        if self.perf_history.len() > 120 {
            let drop = self.perf_history.len() - 120;
            self.perf_history.drain(0..drop);
        }
    }

    /// Rolling stats of the last up-to-60 frames: (avg_ms, min_ms, max_ms)
    pub fn frame_ms_stats(&self) -> Option<(f32, f32, f32)> {
        if self.perf_history.is_empty() {
            return None;
        }
        let n = self.perf_history.len().min(60);
        let slice = &self.perf_history[self.perf_history.len() - n..];
        let mut sum = 0.0f32;
        let mut min = f32::INFINITY;
        let mut max = 0.0f32;
        for &v in slice {
            sum += v;
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }
        let avg = sum / n as f32;
        Some((avg, min, max))
    }

    pub fn draw_rects(
        &mut self,
        window: &winit::window::Window,
        size_info: &SizeInfo,
        metrics: &Metrics,
        rects_in: Vec<RenderRect>,
    ) {
        // Frame start timestamp (for perf HUD)
        let frame_start = std::time::Instant::now();

        // Create and configure surface for this frame.
        let surface = match self.instance.create_surface(window) {
            Ok(s) => s,
            Err(_) => return,
        };
        surface.configure(&self.device, &self.config);
        // Acquire frame from surface.
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(err) => {
                match err {
                    wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost => {
                        surface.configure(&self.device, &self.config);
                    }
                    wgpu::SurfaceError::OutOfMemory => return,
                    wgpu::SurfaceError::Timeout => return,
                    _ => return,
                }
                match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(_) => return,
                }
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("frame-view"),
            ..Default::default()
        });

        // Build vertices for all rects in NDC coordinates, including staged backgrounds.
        let half_w = size_info.width() / 2.0;
        let half_h = size_info.height() / 2.0;
        let mut all_rects = Vec::with_capacity(self.pending_bg.len() + rects_in.len());
        all_rects.append(&mut self.pending_bg);
        all_rects.extend(rects_in);
        // Reuse CPU vertex buffer
        let vertices = &mut self.rect_vertices_cpu;
        vertices.clear();
        vertices.reserve(all_rects.len() * 6);
        for rect in all_rects.iter() {
            let x = rect.x / half_w - 1.0;
            let y = -rect.y / half_h + 1.0;
            let w = rect.width / half_w;
            let h = rect.height / half_h;

            let a = (rect.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            let color = [rect.color.r, rect.color.g, rect.color.b, a];
            let kind = rect.kind as u32;

            let v0 = RectVertex {
                pos: [x, y],
                color,
                kind,
            };
            let v1 = RectVertex {
                pos: [x, y - h],
                color,
                kind,
            };
            let v2 = RectVertex {
                pos: [x + w, y],
                color,
                kind,
            };
            let v3 = RectVertex {
                pos: [x + w, y - h],
                color,
                kind,
            };

            // Two triangles: (0,1,2) and (2,3,1)
            vertices.extend_from_slice(&[v0, v1, v2, v2, v3, v1]);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rects-encoder"),
            });

        // Clear color from pending state if present.
        let clear = if let Some(c) = self.pending_clear.get() {
            self.pending_clear.set(None);
            wgpu::Color {
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
            }
        } else {
            wgpu::Color::TRANSPARENT
        };

        // Upload vertices via staging+copy to minimize queue writes
        self.rect_transfer.begin_frame();
        if !self.rect_vertices_cpu.is_empty() {
            // Use the typed path by default, but we could also append_raw if we had &[u8]
            self.rect_transfer
                .append_vertices(&self.device, &self.rect_vertices_cpu);
        }

        // Optionally stage perf HUD background as rounded rects before creating buffers
        if self.perf_hud_enabled {
            // Draw small background in top-left (8px padding)
            let bg = UiRoundedRect {
                x: 8.0,
                y: 8.0,
                width: 140.0,
                height: 40.0,
                radius: 6.0,
                color: crate::display::color::Rgb::new(0, 0, 0),
                alpha: 0.5,
            };
            self.stage_ui_rounded_rect(size_info, bg);
        }

        // Prepare UI vertex buffer outside the pass to satisfy borrow checker.
        let ui_buf_opt = (!self.pending_ui.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui-vertex-buffer"),
                    contents: bytemuck::cast_slice(&self.pending_ui),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        // Prepare sprite vertex buffers
        let spr_buf_linear_opt = (!self.pending_sprites_linear.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sprite-vertex-buffer-linear"),
                    contents: bytemuck::cast_slice(&self.pending_sprites_linear),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let spr_buf_nearest_opt = (!self.pending_sprites_nearest.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sprite-vertex-buffer-nearest"),
                    contents: bytemuck::cast_slice(&self.pending_sprites_nearest),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        {
            // Update rect uniforms (cell size, padding, underline metrics)
            let cw = size_info.cell_width();
            let ch = size_info.cell_height();
            let px = size_info.padding_x();
            let py = size_info.padding_y();
            let u_pos = metrics.underline_position;
            let u_th = metrics.underline_thickness.max(1.0);
            let u_curl = metrics.descent.abs().max(1.0);
            let rect_uniforms: [f32; 8] = [cw, ch, px, py, u_pos, u_th, u_curl, 0.0];
            self.queue.write_buffer(
                &self.rect_uniform_buffer,
                0,
                bytemuck::cast_slice(&rect_uniforms),
            );

            // Flush staging -> GPU vertex buffer before drawing rects/UI
            let copied = self.rect_transfer.flush(&mut encoder, &self.device);
            if copied > 0 {
                self.metrics.rect_bytes_copied = copied;
                self.metrics.rect_flush_count += 1;
            }

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rects-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if !self.rect_vertices_cpu.is_empty() {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_bind_group(0, &self.rect_bind_group, &[]);
                let used_bytes = (self.rect_vertices_cpu.len() * std::mem::size_of::<RectVertex>())
                    as wgpu::BufferAddress;
                pass.set_vertex_buffer(0, self.rect_transfer.vertex_buffer().slice(0..used_bytes));
                pass.draw(0..self.rect_vertices_cpu.len() as u32, 0..1);
                self.metrics.draw_calls += 1;
                self.metrics.vertices_submitted = self
                    .metrics
                    .vertices_submitted
                    .saturating_add(self.rect_vertices_cpu.len() as u32);
            }

            // Draw pending UI rounded rects in the same pass for correct layering.
            if let Some(ref ui_buf) = ui_buf_opt {
                pass.set_pipeline(&self.ui_pipeline);
                pass.set_vertex_buffer(0, ui_buf.slice(..));
                pass.draw(0..self.pending_ui.len() as u32, 0..1);
                self.metrics.draw_calls += 1;
                self.metrics.vertices_submitted = self
                    .metrics
                    .vertices_submitted
                    .saturating_add(self.pending_ui.len() as u32);
            }

            // Draw sprites (linear then nearest) for proper filter usage
            if spr_buf_linear_opt.is_some() || spr_buf_nearest_opt.is_some() {
                pass.set_pipeline(&self.sprite_pipeline);
            }
            if let Some(ref spr_buf) = spr_buf_linear_opt {
                pass.set_bind_group(0, &self.sprite_bind_group_linear, &[]);
                pass.set_vertex_buffer(0, spr_buf.slice(..));
                pass.draw(0..self.pending_sprites_linear.len() as u32, 0..1);
                self.metrics.draw_calls += 1;
                self.metrics.vertices_submitted = self
                    .metrics
                    .vertices_submitted
                    .saturating_add(self.pending_sprites_linear.len() as u32);
            }
            if let Some(ref spr_buf) = spr_buf_nearest_opt {
                pass.set_bind_group(0, &self.sprite_bind_group_nearest, &[]);
                pass.set_vertex_buffer(0, spr_buf.slice(..));
                pass.draw(0..self.pending_sprites_nearest.len() as u32, 0..1);
                self.metrics.draw_calls += 1;
                self.metrics.vertices_submitted = self
                    .metrics
                    .vertices_submitted
                    .saturating_add(self.pending_sprites_nearest.len() as u32);
            }
        }

        // After the pass, it's safe to clear pending UI and sprite vertices.
        if !self.pending_ui.is_empty() {
            self.pending_ui.clear();
        }
        if !self.pending_sprites_linear.is_empty() {
            self.pending_sprites_linear.clear();
        }
        if !self.pending_sprites_nearest.is_empty() {
            self.pending_sprites_nearest.clear();
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
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.text_pipeline);
                pass.set_bind_group(0, &self.text_bind_group, &[]);
                pass.set_vertex_buffer(0, text_vbuf.slice(..));
                pass.draw(0..self.pending_text.len() as u32, 0..1);
                self.metrics.draw_calls += 1;
                self.metrics.vertices_submitted = self
                    .metrics
                    .vertices_submitted
                    .saturating_add(self.pending_text.len() as u32);
            }

            self.pending_text.clear();
        }

        self.queue.submit([encoder.finish()]);
        frame.present();

        // Update perf metrics
        self.last_frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.perf_history.push(self.last_frame_ms);
        if self.perf_history.len() > 120 {
            self.perf_history.remove(0);
        }
        self.last_draw_calls = self.metrics.draw_calls;
        self.last_vertices = self.metrics.vertices_submitted;
        self.metrics.primitives_batched = all_rects.len() as u32;

        self.frame_counter += 1;
        // Renderer-level reporting cadence
        let report_interval = self.renderer_report_interval_frames as u64;
        if report_interval > 0 && self.frame_counter % report_interval == 0 {
            debug!(
                "wgpu frame={} dt_ms={:.2} draw_calls={} vertices={} rect_copy_bytes={} rect_flushes={} batched_rects={}",
                self.frame_counter,
                self.last_frame_ms,
                self.metrics.draw_calls,
                self.metrics.vertices_submitted,
                self.metrics.rect_bytes_copied,
                self.metrics.rect_flush_count,
                self.metrics.primitives_batched
            );
        }

        // Reset per-frame metrics counters
        self.metrics.draw_calls = 0;
        self.metrics.vertices_submitted = 0;
        self.metrics.rect_bytes_copied = 0;
        self.metrics.rect_flush_count = 0;
        self.metrics.primitives_batched = 0;

        // If perf HUD enabled, render a minimal overlay background (text overlay is postponed to Display-level draw)
        let hud_cfg = crate::config::UiConfig::default().debug.renderer_perf_hud;
        if self.perf_hud_enabled || hud_cfg {
            let _pad = 6.0f32;
            let bg_h = 20.0f32;
            let bg_w = 180.0f32;
            let bg_x = 12.0f32;
            let bg_y = 12.0f32;
            let tokens = crate::display::color::Rgb::new(20, 20, 20);
            let pill = UiRoundedRect::new(bg_x, bg_y, bg_w, bg_h, 6.0, tokens, 0.65);
            let si = SizeInfo::new(
                self.size.width.max(1) as f32,
                self.size.height.max(1) as f32,
                8.0,
                16.0,
                0.0,
                0.0,
                false,
            );
            self.stage_ui_rounded_rect(&si, pill);
        }
        // Also allow config to toggle HUD globally
        let hud_cfg = crate::config::UiConfig::default().debug.renderer_perf_hud;
        if self.perf_hud_enabled || hud_cfg {
            let _hud_text = format!("{:.1} ms", self.last_frame_ms);
            // Stage a rounded bg rect was already done before pass; now stage text on top via second pass
            // Convert pixels to a character string at an approximate top-left area using text vertices
            // We render it in the text pass by staging a tiny text quad set; reuse draw_string path
            // Build a throwaway glyph loader by calling draw_string directly
            // We emulate Display::draw_ai_text call here
            // Approximate a single-cell baseline in the top left
            // We piggy-back the text pass by creating text vertices via the glyph loader API
            // For simplicity, draw at pixel (16,16) via a background rect already placed
            // The text pass uses pending_text, populated by draw_string; call a small helper
            // We'll map a minimal point (line=0,col=0) and rely on Display to position normally.
            // Since we don't have Display here, we stage with a helper in draw_cells path next frame.
            // As a compromise, draw using UI sprite path is not suitable; so we skip adding text here.
            // Instead, draw a tiny white rect as a visual indicator next to the bg.
            let ind = RenderRect::new(
                12.0,
                12.0,
                40.0,
                2.0,
                crate::display::color::Rgb::new(255, 255, 255),
                0.8,
            );
            self.pending_bg.push(ind);
        }

        // Periodic atlas reporting.
        if self.report_interval_frames > 0 {
            self.frame_counter = self.frame_counter.wrapping_add(1);
            if (self.frame_counter as u32) % self.report_interval_frames == 0 {
                self.dump_atlas_stats();
            }
        }
    }

    // UI sprite staging API (textured quads with tint)
    pub fn stage_ui_sprite(&mut self, sprite: UiSprite) {
        // Convert pixel-space to NDC using current surface size
        let half_w = self.size.width.max(1) as f32 / 2.0;
        let half_h = self.size.height.max(1) as f32 / 2.0;
        let x_ndc = sprite.x / half_w - 1.0;
        let y_ndc = -sprite.y / half_h + 1.0;
        let w_ndc = sprite.width / half_w;
        let h_ndc = sprite.height / half_h;

        // UVs
        let u0 = sprite.uv_x;
        let v0 = sprite.uv_y;
        let u1 = sprite.uv_x + sprite.uv_w;
        let v1 = sprite.uv_y + sprite.uv_h;

        let tint = [
            sprite.tint.r as f32 / 255.0,
            sprite.tint.g as f32 / 255.0,
            sprite.tint.b as f32 / 255.0,
            sprite.alpha,
        ];

        // Two triangles
        let verts = [
            SpriteVertex {
                pos: [x_ndc, y_ndc],
                uv: [u0, v0],
                tint,
            },
            SpriteVertex {
                pos: [x_ndc, y_ndc - h_ndc],
                uv: [u0, v1],
                tint,
            },
            SpriteVertex {
                pos: [x_ndc + w_ndc, y_ndc],
                uv: [u1, v0],
                tint,
            },
            SpriteVertex {
                pos: [x_ndc + w_ndc, y_ndc],
                uv: [u1, v0],
                tint,
            },
            SpriteVertex {
                pos: [x_ndc, y_ndc - h_ndc],
                uv: [u0, v1],
                tint,
            },
            SpriteVertex {
                pos: [x_ndc + w_ndc, y_ndc - h_ndc],
                uv: [u1, v1],
                tint,
            },
        ];

        let use_nearest = sprite
            .filter_nearest
            .unwrap_or(self.sprite_default_filter_nearest);
        if use_nearest {
            self.pending_sprites_nearest.extend_from_slice(&verts);
        } else {
            self.pending_sprites_linear.extend_from_slice(&verts);
        }
    }

    pub fn set_sprite_filter_nearest(&mut self, nearest: bool) {
        self.sprite_default_filter_nearest = nearest;
    }

    pub fn set_subpixel_gamma(&mut self, gamma: f32) {
        self.subpixel_gamma = gamma.clamp(1.4, 3.0);
        // Update uniform immediately
        let proj = projection_from_size(self.size);
        self.queue.write_buffer(
            &self.proj_buffer,
            0,
            bytemuck::bytes_of(&[
                proj.offset_x,
                proj.offset_y,
                proj.scale_x,
                proj.scale_y,
                self.subpixel_gamma,
            ]),
        );
    }

    pub fn adjust_subpixel_gamma(&mut self, delta: f32) {
        let g = (self.subpixel_gamma + delta).clamp(1.4, 3.0);
        self.set_subpixel_gamma(g);
    }

    pub fn toggle_perf_hud(&mut self) {
        self.perf_hud_enabled = !self.perf_hud_enabled;
    }

    pub fn perf_hud_enabled(&self) -> bool {
        self.perf_hud_enabled
    }

    pub fn toggle_subpixel_enabled(&mut self) -> bool {
        self.subpixel_enabled = !self.subpixel_enabled;
        self.subpixel_enabled
    }

    pub fn set_subpixel_enabled(&mut self, enabled: bool) {
        self.subpixel_enabled = enabled;
    }

    pub fn cycle_subpixel_orientation(&mut self) -> SubpixelOrientation {
        self.subpixel_bgr = !self.subpixel_bgr;
        if self.subpixel_bgr {
            SubpixelOrientation::BGR
        } else {
            SubpixelOrientation::RGB
        }
    }

    pub fn set_subpixel_orientation(&mut self, orientation: SubpixelOrientation) {
        self.subpixel_bgr = matches!(orientation, SubpixelOrientation::BGR);
    }

    pub fn stage_ui_rounded_rect(&mut self, size_info: &SizeInfo, rect: UiRoundedRect) {
        let half_w = size_info.width() / 2.0;
        let half_h = size_info.height() / 2.0;
        let x = rect.x / half_w - 1.0;
        let y = -rect.y / half_h + 1.0;
        let w = rect.width / half_w;
        let h = rect.height / half_h;
        let color = [
            rect.color.r as f32 / 255.0,
            rect.color.g as f32 / 255.0,
            rect.color.b as f32 / 255.0,
            rect.alpha,
        ];
        let v0 = UiVertex {
            pos: [x, y],
            origin: [rect.x, rect.y],
            size: [rect.width, rect.height],
            radius: rect.radius,
            color,
        };
        let v1 = UiVertex {
            pos: [x, y - h],
            origin: [rect.x, rect.y],
            size: [rect.width, rect.height],
            radius: rect.radius,
            color,
        };
        let v2 = UiVertex {
            pos: [x + w, y],
            origin: [rect.x, rect.y],
            size: [rect.width, rect.height],
            radius: rect.radius,
            color,
        };
        let v3 = UiVertex {
            pos: [x + w, y - h],
            origin: [rect.x, rect.y],
            size: [rect.width, rect.height],
            radius: rect.radius,
            color,
        };
        self.pending_ui.extend_from_slice(&[v0, v1, v2, v2, v3, v1]);
    }

    pub fn draw_cells<I: Iterator<Item = RenderableCell>>(
        &mut self,
        size_info: &SizeInfo,
        glyph_cache: &mut GlyphCache,
        cells: I,
    ) {
        // Stage text vertices to render on the next draw_rects pass.
        let subpixel = self.subpixel_enabled;
        let bgr = self.subpixel_bgr;
        let mut loader = WgpuGlyphLoader { renderer: self };
        let mut staged: Vec<TextVertex> = Vec::new();
        let mut staged_bg: Vec<RenderRect> = Vec::new();

        // Coalesce adjacent full-cell background quads per line (same color/alpha) to reduce vertices.
        #[derive(Clone, Copy)]
        struct BgRun {
            line: usize,
            start_col: usize,
            end_col: usize,
            color: Rgb,
            alpha: f32,
        }
        let mut run: Option<BgRun> = None;

        let cw = size_info.cell_width();
        let ch = size_info.cell_height();
        let px = size_info.padding_x();
        let py = size_info.padding_y();

        let flush_run = |staged_bg: &mut Vec<RenderRect>, r: BgRun| {
            let x = r.start_col as f32 * cw + px;
            let y = r.line as f32 * ch + py;
            let width = (r.end_col - r.start_col + 1) as f32 * cw;
            let height = ch;
            staged_bg.push(RenderRect::new(x, y, width, height, r.color, r.alpha));
        };

        for mut cell in cells {
            // Stage full-cell background quads by merging horizontally contiguous cells.
            if cell.bg_alpha > 0.0 {
                let line = cell.point.line;
                let col = cell.point.column.0;
                let color = cell.bg;
                let alpha = cell.bg_alpha;
                match run {
                    Some(ref mut r)
                        if r.line == line
                            && r.color == color
                            && r.alpha == alpha
                            && col == r.end_col + 1 =>
                    {
                        r.end_col = col;
                    }
                    Some(r_prev) => {
                        // Flush previous run and start a new one.
                        flush_run(&mut staged_bg, r_prev);
                        run = Some(BgRun {
                            line,
                            start_col: col,
                            end_col: col,
                            color,
                            alpha,
                        });
                    }
                    None => {
                        run = Some(BgRun {
                            line,
                            start_col: col,
                            end_col: col,
                            color,
                            alpha,
                        });
                    }
                }
            }

            // Skip hidden or tab cells by rendering as space.
            let hidden = cell
                .flags
                .contains(openagent_terminal_core::term::cell::Flags::HIDDEN);
            if cell.character == '\t' || hidden {
                cell.character = ' ';
            }

            // Select font based on style flags.
            let font_key =
                match cell.flags & openagent_terminal_core::term::cell::Flags::BOLD_ITALIC {
                    openagent_terminal_core::term::cell::Flags::BOLD_ITALIC => {
                        glyph_cache.bold_italic_key
                    }
                    openagent_terminal_core::term::cell::Flags::ITALIC => glyph_cache.italic_key,
                    openagent_terminal_core::term::cell::Flags::BOLD => glyph_cache.bold_key,
                    _ => glyph_cache.font_key,
                };

            // Primary glyph.
            let glyph_key = GlyphKey {
                font_key,
                size: glyph_cache.font_size,
                character: cell.character,
            };
            let g = glyph_cache.get(glyph_key, &mut loader, true);
            staged.extend_from_slice(&build_text_vertices(size_info, &cell, &g, subpixel, bgr));

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
                    staged.extend_from_slice(&build_text_vertices(
                        size_info, &cell, &gzw, subpixel, bgr,
                    ));
                }
            }
        }

        // Flush any remaining run at the end of the row scan.
        if let Some(r) = run.take() {
            flush_run(&mut staged_bg, r);
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
        let subpixel = self.subpixel_enabled;
        let bgr = self.subpixel_bgr;
        let mut loader = WgpuGlyphLoader { renderer: self };
        let mut col = point.column.0;
        let mut staged: Vec<TextVertex> = Vec::new();
        let mut staged_bg: Vec<RenderRect> = Vec::new();
        for ch in string_chars {
            let glyph_key = GlyphKey {
                font_key: glyph_cache.font_key,
                size: glyph_cache.font_size,
                character: ch,
            };
            let cell = RenderableCell {
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
            staged.extend_from_slice(&build_text_vertices(size_info, &cell, &g, subpixel, bgr));
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
        if ev {
            self.atlas_evicted.set(false);
        }
        ev
    }

    pub fn reset_atlas(&mut self) {
        for page in &mut self.atlas_pages {
            page.clear();
        }
        for meta in &mut self.page_meta {
            meta.last_use = 0;
        }
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
            let pct = if capacity > 0 {
                (used as f64 / capacity as f64) * 100.0
            } else {
                0.0
            };
            debug!(
                "WGPU atlas eviction: layer={} used={} / {} ({:.1}%), policy={:?}, counters: \
                 inserts={}, misses={}, evictions={}",
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
                let extent = wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                };
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.atlas_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &self.zero_scratch,
                    wgpu::TexelCopyBufferLayout {
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

    pub fn was_context_reset(&self) -> bool {
        false
    }

    pub fn set_viewport(&self, _size: &SizeInfo) {}

    pub fn dump_atlas_stats(&self) {
        let mut lines = Vec::new();
        let mut total_used = 0u64;
        let mut total_capacity = 0u64;
        for (i, page) in self.atlas_pages.iter().enumerate() {
            let cap = (page.width as u64) * (page.height as u64);
            let used = page.used_area.min(cap);
            total_used += used;
            total_capacity += cap;
            let pct = if cap > 0 {
                (used as f64 / cap as f64) * 100.0
            } else {
                0.0
            };
            let ts = self.page_meta[i].last_use;
            lines.push(format!(
                "layer={} used={} / {} ({:.1}%), last_use={}",
                i, used, cap, pct, ts
            ));
        }
        let total_pct = if total_capacity > 0 {
            (total_used as f64 / total_capacity as f64) * 100.0
        } else {
            0.0
        };
        debug!(
            "WGPU atlas stats: policy={:?} inserts={} misses={} evictions={} total_used={} / {} \
             ({:.1}%)\n{}",
            self.policy,
            self.atlas_inserts,
            self.atlas_insert_misses,
            self.atlas_evictions_count,
            total_used,
            total_capacity,
            total_pct,
            lines.join("\n")
        );
    }
}

fn build_text_vertices(
    size_info: &SizeInfo,
    cell: &RenderableCell,
    glyph: &Glyph,
    subpixel: bool,
    bgr: bool,
) -> [TextVertex; 6] {
    let cell_x = cell.point.column.0 as f32 * size_info.cell_width() + size_info.padding_x();
    let gx = cell_x + glyph.left as f32;
    let gy = (cell.point.line + 1) as f32 * size_info.cell_height() + size_info.padding_y()
        - glyph.top as f32;

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
    if subpixel {
        flags |= 2u32;
    }
    if bgr {
        flags |= 4u32;
    }

    let layer = if glyph.tex_id > 0 {
        glyph.tex_id - 1
    } else {
        0
    };

    [
        TextVertex {
            pos: [x0, y0],
            uv: [u0, v0],
            color,
            flags,
            layer,
        },
        TextVertex {
            pos: [x0, y1],
            uv: [u0, v1],
            color,
            flags,
            layer,
        },
        TextVertex {
            pos: [x1, y0],
            uv: [u1, v0],
            color,
            flags,
            layer,
        },
        TextVertex {
            pos: [x1, y0],
            uv: [u1, v0],
            color,
            flags,
            layer,
        },
        TextVertex {
            pos: [x1, y1],
            uv: [u1, v1],
            color,
            flags,
            layer,
        },
        TextVertex {
            pos: [x0, y1],
            uv: [u0, v1],
            color,
            flags,
            layer,
        },
    ]
}

struct WgpuGlyphLoader<'a> {
    renderer: &'a mut WgpuRenderer,
}

impl LoadGlyph for WgpuGlyphLoader<'_> {
    fn load_glyph(&mut self, rasterized: &RasterizedGlyph) -> Glyph {
        // Insert into atlas, uploading to GPU.
        let w = rasterized.width;
        let h = rasterized.height;
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
                    AtlasEvictionPolicy::RoundRobin => {
                        (self.renderer.current_page + 1) % NUM_ATLAS_PAGES
                    }
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
                self.renderer.atlas_insert_misses =
                    self.renderer.atlas_insert_misses.wrapping_add(1);
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
            }
        };

        // Prepare pixel data (RGBA8). For RGB, store alpha in A and zero RGB.
        let (rgba, multicolor) = match &rasterized.buffer {
            BitmapBuffer::Rgba(buf) => (buf.clone(), true),
            BitmapBuffer::Rgb(buf) => {
                let mut out =
                    Vec::with_capacity((rasterized.width * rasterized.height * 4) as usize);
                for chunk in buf.chunks_exact(3) {
                    // Use red channel as alpha; set RGB to 0.
                    let a = chunk[0];
                    out.extend_from_slice(&[0, 0, 0, a]);
                }
                (out, false)
            }
        };

        // Upload the glyph into the atlas texture.
        let extent = wgpu::Extent3d {
            width: rasterized.width as u32,
            height: rasterized.height as u32,
            depth_or_array_layers: 1,
        };
        self.renderer.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.renderer.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: ox as u32,
                    y: oy as u32,
                    z: page_idx,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
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
        let u1 = (ox + rasterized.width) as f32 / page_dims.width as f32;
        let v1 = (oy + rasterized.height) as f32 / page_dims.height as f32;

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
