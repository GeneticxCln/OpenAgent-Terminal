// Terminal rendering shader in WGSL

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct PushConstants {
    // Viewport transformation
    viewport_size: vec2<f32>,
    viewport_offset: vec2<f32>,

    // Cell dimensions
    cell_width: f32,
    cell_height: f32,

    // Time for animations
    time: f32,

    // Padding
    _padding: f32,
}

var<push_constant> pc: PushConstants;

// Vertex shader for quad generation
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var output: VertexOutput;

    // Generate quad vertices
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Calculate cell position from instance
    let cell_x = f32(instance_index % 256u);
    let cell_y = f32(instance_index / 256u);

    // Transform to NDC coordinates
    let pos_x = (cell_x * pc.cell_width + x * pc.cell_width - pc.viewport_offset.x) / pc.viewport_size.x * 2.0 - 1.0;
    let pos_y = 1.0 - (cell_y * pc.cell_height + y * pc.cell_height - pc.viewport_offset.y) / pc.viewport_size.y * 2.0;

    output.position = vec4<f32>(pos_x, pos_y, 0.0, 1.0);
    output.tex_coords = vec2<f32>(x, y);

    // Default color (will be overridden by cell data)
    output.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);

    return output;
}

// Fragment shader for cell rendering
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // TODO: Sample from glyph atlas texture
    // For now, return a simple color
    return input.color;
}

// Performance HUD vertex shader
@vertex
fn vs_hud(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var output: VertexOutput;

    // Generate HUD quad in top-right corner
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Position HUD in top-right (NDC coordinates)
    output.position = vec4<f32>(
        0.5 + x * 0.5,  // Right side
        0.5 + y * 0.5,  // Top side
        0.0,
        1.0
    );

    output.tex_coords = vec2<f32>(x, y);
    output.color = vec4<f32>(0.0, 0.0, 0.0, 0.8); // Semi-transparent black

    return output;
}

// Performance HUD fragment shader
@fragment
fn fs_hud(input: VertexOutput) -> @location(0) vec4<f32> {
    // Background with slight transparency
    var color = vec4<f32>(0.1, 0.1, 0.1, 0.9);

    // Add border
    let border_width = 0.02;
    if (input.tex_coords.x < border_width ||
        input.tex_coords.x > 1.0 - border_width ||
        input.tex_coords.y < border_width ||
        input.tex_coords.y > 1.0 - border_width) {
        color = vec4<f32>(0.3, 0.3, 0.3, 1.0);
    }

    return color;
}

// Cursor rendering shader
@vertex
fn vs_cursor(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var output: VertexOutput;

    // Generate cursor quad
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // TODO: Get cursor position from uniform buffer
    let cursor_x = 0.0;
    let cursor_y = 0.0;

    output.position = vec4<f32>(
        cursor_x + x * pc.cell_width,
        cursor_y + y * pc.cell_height,
        0.0,
        1.0
    );

    output.tex_coords = vec2<f32>(x, y);

    // Blinking cursor effect
    let blink = sin(pc.time * 6.0) * 0.5 + 0.5;
    output.color = vec4<f32>(1.0, 1.0, 1.0, blink);

    return output;
}

// Selection highlight shader
@fragment
fn fs_selection(input: VertexOutput) -> @location(0) vec4<f32> {
    // Semi-transparent selection color
    return vec4<f32>(0.3, 0.5, 0.8, 0.4);
}
