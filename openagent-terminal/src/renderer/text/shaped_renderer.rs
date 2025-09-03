// WGPU shaped text renderer integration
// Integrates HarfBuzz text shaping with the WGPU rendering pipeline

use anyhow::{Context, Result};
use std::borrow::Cow;
use std::collections::HashMap;
use std::mem;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::config::font::Font as FontConfig;
use crate::display::content::RenderableCell;
use crate::display::SizeInfo;
use crate::renderer::text::glyph_cache::{Glyph, GlyphCache};
use crate::text_shaping::integration::{
    IntegratedTextShaper, ShapedCell, ShapedCellGlyph, ShapedLine, ShapedTextRenderer,
    ShapingIntegrationConfig,
};

/// Vertex data for shaped text rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShapedTextVertex {
    /// Position in screen coordinates
    pub position: [f32; 2],
    /// UV coordinates in the glyph atlas
    pub uv: [f32; 2],
    /// Text color (RGBA)
    pub color: [f32; 4],
    /// Glyph flags (colored, subpixel, etc.)
    pub flags: u32,
    /// Atlas layer/page
    pub layer: u32,
    /// Glyph offset for proper positioning
    pub glyph_offset: [f32; 2],
}

/// Configuration for shaped text rendering
#[derive(Debug, Clone)]
pub struct ShapedRenderConfig {
    /// Maximum number of vertices per batch
    pub max_vertices_per_batch: usize,
    /// Enable subpixel rendering
    pub enable_subpixel: bool,
    /// Enable colored emoji rendering
    pub enable_colored_glyphs: bool,
}

impl Default for ShapedRenderConfig {
    fn default() -> Self {
        Self { max_vertices_per_batch: 16384, enable_subpixel: true, enable_colored_glyphs: true }
    }
}

/// WGPU-based shaped text renderer
pub struct WgpuShapedTextRenderer {
    text_shaper: IntegratedTextShaper,
    render_config: ShapedRenderConfig,

    // WGPU resources
    device: wgpu::Device,
    queue: wgpu::Queue,

    // Vertex buffer for batched rendering
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,

    // Staging data
    vertices: Vec<ShapedTextVertex>,
    indices: Vec<u32>,

    // Render pipeline
    render_pipeline: wgpu::RenderPipeline,
    bind_group: Option<wgpu::BindGroup>,

    // Cache for line shaping results
    shaped_line_cache: HashMap<String, ShapedLine>,
}

impl WgpuShapedTextRenderer {
    /// Create a new shaped text renderer
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        font_config: &FontConfig,
        shaping_config: ShapingIntegrationConfig,
        render_config: ShapedRenderConfig,
    ) -> Result<Self> {
        let text_shaper = IntegratedTextShaper::new(font_config, shaping_config)
            .context("Failed to create integrated text shaper")?;

        // Create vertex buffer
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shaped Text Vertex Buffer"),
            size: (render_config.max_vertices_per_batch * mem::size_of::<ShapedTextVertex>())
                as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create index buffer
        let max_indices = render_config.max_vertices_per_batch * 6 / 4; // Assuming quads
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shaped Text Index Buffer"),
            size: (max_indices * mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create render pipeline
        let render_pipeline = Self::create_render_pipeline(&device, surface_format)?;

        Ok(Self {
            text_shaper,
            render_config,
            device,
            queue,
            vertex_buffer,
            index_buffer,
            vertices: Vec::with_capacity(render_config.max_vertices_per_batch),
            indices: Vec::with_capacity(max_indices),
            render_pipeline,
            bind_group: None,
            shaped_line_cache: HashMap::new(),
        })
    }

    /// Render a line of terminal cells with advanced text shaping
    pub fn render_shaped_line<I>(
        &mut self,
        cells: I,
        glyph_cache: &mut GlyphCache,
        size_info: &SizeInfo,
        base_position: [f32; 2],
    ) -> Result<()>
    where
        I: Iterator<Item = RenderableCell> + Clone,
    {
        // Shape the text line
        let shaped_line = self.text_shaper.shape_line(cells, glyph_cache, size_info)?;

        // Convert to vertices
        self.convert_shaped_line_to_vertices(&shaped_line, base_position, size_info)?;

        // Render the vertices
        self.flush_vertices()?;

        Ok(())
    }

    /// Convert a shaped line to renderable vertices
    fn convert_shaped_line_to_vertices(
        &mut self,
        shaped_line: &ShapedLine,
        base_position: [f32; 2],
        size_info: &SizeInfo,
    ) -> Result<()> {
        let cell_width = size_info.cell_width();
        let cell_height = size_info.cell_height();

        for shaped_cell in &shaped_line.cells {
            let cell_x = base_position[0] + shaped_cell.cell_index as f32 * cell_width;
            let cell_y = base_position[1];

            self.render_shaped_cell(shaped_cell, [cell_x, cell_y], cell_width, cell_height)?;
        }

        Ok(())
    }

    /// Render a single shaped cell
    fn render_shaped_cell(
        &mut self,
        shaped_cell: &ShapedCell,
        position: [f32; 2],
        cell_width: f32,
        cell_height: f32,
    ) -> Result<()> {
        let mut x_offset = 0.0;

        for shaped_glyph in &shaped_cell.shaped_glyphs {
            self.add_glyph_vertices(
                &shaped_glyph.glyph,
                [
                    position[0] + x_offset + shaped_glyph.x_offset,
                    position[1] + shaped_glyph.y_offset,
                ],
                [1.0, 1.0, 1.0, 1.0], // Default white color - should be from cell
                shaped_glyph,
            )?;

            x_offset += shaped_glyph.x_advance;
        }

        Ok(())
    }

    /// Add vertices for a single glyph
    fn add_glyph_vertices(
        &mut self,
        glyph: &Glyph,
        position: [f32; 2],
        color: [f32; 4],
        shaped_glyph: &ShapedCellGlyph,
    ) -> Result<()> {
        if self.vertices.len() + 4 > self.render_config.max_vertices_per_batch {
            self.flush_vertices()?;
        }

        let flags = if glyph.multicolor { 1 } else { 0 }
            | if self.render_config.enable_subpixel { 2 } else { 0 };

        // Glyph dimensions
        let glyph_width = glyph.width as f32;
        let glyph_height = glyph.height as f32;

        // Glyph position (accounting for bearing)
        let x = position[0] + glyph.left as f32;
        let y = position[1] - glyph.top as f32;

        // UV coordinates in atlas
        let uv_left = glyph.uv_left;
        let uv_bot = glyph.uv_bot;
        let uv_width = glyph.uv_width;
        let uv_height = glyph.uv_height;

        let base_index = self.vertices.len() as u32;

        // Add four vertices for the quad
        self.vertices.extend_from_slice(&[
            ShapedTextVertex {
                position: [x, y],
                uv: [uv_left, uv_bot + uv_height],
                color,
                flags,
                layer: glyph.tex_id,
                glyph_offset: [shaped_glyph.x_offset, shaped_glyph.y_offset],
            },
            ShapedTextVertex {
                position: [x + glyph_width, y],
                uv: [uv_left + uv_width, uv_bot + uv_height],
                color,
                flags,
                layer: glyph.tex_id,
                glyph_offset: [shaped_glyph.x_offset, shaped_glyph.y_offset],
            },
            ShapedTextVertex {
                position: [x + glyph_width, y + glyph_height],
                uv: [uv_left + uv_width, uv_bot],
                color,
                flags,
                layer: glyph.tex_id,
                glyph_offset: [shaped_glyph.x_offset, shaped_glyph.y_offset],
            },
            ShapedTextVertex {
                position: [x, y + glyph_height],
                uv: [uv_left, uv_bot],
                color,
                flags,
                layer: glyph.tex_id,
                glyph_offset: [shaped_glyph.x_offset, shaped_glyph.y_offset],
            },
        ]);

        // Add indices for two triangles (quad)
        self.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);

        Ok(())
    }

    /// Flush accumulated vertices to the GPU and render
    fn flush_vertices(&mut self) -> Result<()> {
        if self.vertices.is_empty() {
            return Ok(());
        }

        // Update vertex buffer
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));

        // Update index buffer
        self.queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.indices));

        // Clear staging data
        self.vertices.clear();
        self.indices.clear();

        Ok(())
    }

    /// Create the shaped text render pipeline
    fn create_render_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Result<wgpu::RenderPipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shaped Text Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../shaders/shaped_text.wgsl"
            ))),
        });

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ShapedTextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // UV
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Flags
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
                // Layer
                wgpu::VertexAttribute {
                    offset: 36,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
                // Glyph offset
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shaped Text Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shaped Text Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_buffer_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(render_pipeline)
    }

    /// Clear caches
    pub fn clear_caches(&mut self) {
        self.text_shaper.clear_caches();
        self.shaped_line_cache.clear();
    }

    /// Get reference to the text shaper
    pub fn text_shaper(&self) -> &IntegratedTextShaper {
        &self.text_shaper
    }

    /// Get mutable reference to the text shaper
    pub fn text_shaper_mut(&mut self) -> &mut IntegratedTextShaper {
        &mut self.text_shaper
    }
}

impl ShapedTextRenderer for WgpuShapedTextRenderer {
    fn render_shaped_line(&mut self, shaped_line: &ShapedLine, size_info: &SizeInfo) -> Result<()> {
        // Convert shaped line to vertices and render
        self.convert_shaped_line_to_vertices(shaped_line, [0.0, 0.0], size_info)?;
        self.flush_vertices()
    }

    fn supports_shaped_text(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shaped_render_config_default() {
        let config = ShapedRenderConfig::default();
        assert_eq!(config.max_vertices_per_batch, 16384);
        assert!(config.enable_subpixel);
        assert!(config.enable_colored_glyphs);
    }
}
