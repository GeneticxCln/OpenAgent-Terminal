// Terminal rendering shader for WGPU
// Handles text rendering, background colors, and effects

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) cell_index: u32,
    @location(4) atlas_layer: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) cell_index: u32,
    @location(3) atlas_layer: u32,
    @location(4) world_pos: vec2<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    time: f32,
    _padding: f32,
}

struct CursorUniforms {
    position: vec2<f32>,
    size: vec2<f32>,
    color: vec4<f32>,
    blink_phase: f32,
    _padding: vec3<f32>,
}

struct CellData {
    character: u32,
    foreground: vec4<f32>,
    background: vec4<f32>,
    glyph_coords: vec4<f32>, // UV coordinates in atlas
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<uniform> cursor: CursorUniforms;

@group(0) @binding(2)
var glyph_atlas: texture_2d_array<f32>;

@group(0) @binding(3)
var atlas_sampler: sampler;

@group(1) @binding(0)
var<storage, read> cell_data: array<CellData>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform position to clip space
    let normalized_pos = vec2<f32>(
        (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0,
        1.0 - (in.position.y / uniforms.screen_size.y) * 2.0
    );

    out.clip_position = vec4<f32>(normalized_pos, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    out.color = in.color;
    out.cell_index = in.cell_index;
    out.atlas_layer = in.atlas_layer;
    out.world_pos = in.position;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Get cell data for this fragment
    let cell = cell_data[in.cell_index];
    
    // Sample from glyph atlas using proper UV coordinates
    let glyph_color = textureSample(glyph_atlas, atlas_sampler, 
                                   cell.glyph_coords.xy + in.tex_coords * cell.glyph_coords.zw, 
                                   i32(in.atlas_layer));

    // Determine if this is a background or foreground fragment
    var final_color: vec4<f32>;
    
    if (glyph_color.a < 0.1) {
        // Background cell
        final_color = cell.background;
    } else {
        // Foreground text - blend glyph with foreground color
        final_color = vec4<f32>(
            cell.foreground.rgb,
            cell.foreground.a * glyph_color.a
        );
    }
    
    // Check if cursor should be rendered at this position
    let cursor_bounds = vec4<f32>(
        cursor.position.x,
        cursor.position.y,
        cursor.position.x + cursor.size.x,
        cursor.position.y + cursor.size.y
    );
    
    let in_cursor = in.world_pos.x >= cursor_bounds.x && 
                    in.world_pos.x <= cursor_bounds.z &&
                    in.world_pos.y >= cursor_bounds.y && 
                    in.world_pos.y <= cursor_bounds.w;
    
    if (in_cursor && cursor.blink_phase > 0.5) {
        // Apply cursor color with blending
        final_color = mix(final_color, cursor.color, cursor.color.a);
    }

    // Apply gamma correction for better text rendering
    final_color.rgb = pow(final_color.rgb, vec3<f32>(2.2));

    return final_color;
}

// Performance HUD vertex shader
@vertex
fn vs_hud(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Create a quad for HUD background
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-0.9, 0.9),   // Top-left
        vec2<f32>(-0.5, 0.9),   // Top-right
        vec2<f32>(-0.5, 0.6),   // Bottom-right
        vec2<f32>(-0.9, 0.9),   // Top-left
        vec2<f32>(-0.5, 0.6),   // Bottom-right
        vec2<f32>(-0.9, 0.6),   // Bottom-left
    );

    out.clip_position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.tex_coords = vec2<f32>(0.0, 0.0);
    out.color = vec4<f32>(0.0, 0.0, 0.0, 0.8); // Semi-transparent black

    return out;
}

// Performance HUD fragment shader
@fragment
fn fs_hud(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
