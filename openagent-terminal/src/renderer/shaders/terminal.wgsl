// Terminal rendering shader for WGPU
// Handles text rendering, background colors, and effects

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct Uniforms {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    time: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var t_diffuse: texture_2d<f32>;

@group(0) @binding(2)
var s_diffuse: sampler;

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
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture for glyph rendering
    let tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    
    // Apply text color
    var final_color = vec4<f32>(in.color.rgb, in.color.a * tex_color.a);
    
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
