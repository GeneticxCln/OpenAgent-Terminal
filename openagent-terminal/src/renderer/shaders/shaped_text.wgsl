// Shaped text rendering shader for advanced text shaping
// Supports HarfBuzz-shaped glyphs with proper positioning, ligatures, and bidirectional text

struct Proj {
    offset_x: f32,
    offset_y: f32,
    scale_x: f32,
    scale_y: f32,
}

@group(0) @binding(0) var<uniform> proj: Proj;
@group(0) @binding(1) var glyph_atlas: texture_2d_array<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) flags: u32,
    @location(4) layer: u32,
    @location(5) glyph_offset: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) flags: u32,
    @location(3) layer: u32,
    @location(4) glyph_offset: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply glyph offset for proper shaping positioning
    let adjusted_pos = input.position + input.glyph_offset;

    // Transform to normalized device coordinates
    let ndc = vec2<f32>(
        proj.offset_x + adjusted_pos.x * proj.scale_x,
        proj.offset_y + adjusted_pos.y * proj.scale_y
    );

    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = input.uv;
    out.color = input.color;
    out.flags = input.flags;
    out.layer = input.layer;
    out.glyph_offset = input.glyph_offset;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the glyph texture
    let glyph_sample = textureSample(glyph_atlas, atlas_sampler, input.uv, i32(input.layer));

    // Extract flags
    let is_colored = (input.flags & 1u) != 0u;
    let is_subpixel = (input.flags & 2u) != 0u;
    let is_emoji = (input.flags & 4u) != 0u;
    let is_ligature = (input.flags & 8u) != 0u;

    var final_color: vec4<f32>;

    if (is_colored) {
        // Colored glyphs (emojis, etc.) - use texture color directly
        final_color = glyph_sample;
    } else if (is_subpixel) {
        // Subpixel rendering for better text clarity
        let alpha = max(glyph_sample.r, max(glyph_sample.g, glyph_sample.b));
        final_color = vec4<f32>(
            input.color.rgb * glyph_sample.rgb,
            alpha * input.color.a
        );
    } else {
        // Standard grayscale text rendering
        final_color = vec4<f32>(
            input.color.rgb,
            glyph_sample.a * input.color.a
        );
    }

    // Apply gamma correction for better text appearance
    final_color.rgb = pow(final_color.rgb, vec3<f32>(1.0 / 2.2));

    return final_color;
}

// Alternative vertex shader for outline/shadow effects
@vertex
fn vs_outline(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply both glyph offset and outline offset
    let outline_offset = vec2<f32>(1.0, 1.0); // Could be uniform
    let adjusted_pos = input.position + input.glyph_offset + outline_offset;

    let ndc = vec2<f32>(
        proj.offset_x + adjusted_pos.x * proj.scale_x,
        proj.offset_y + adjusted_pos.y * proj.scale_y
    );

    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = input.uv;
    out.color = vec4<f32>(0.0, 0.0, 0.0, input.color.a * 0.5); // Semi-transparent black
    out.flags = input.flags;
    out.layer = input.layer;
    out.glyph_offset = input.glyph_offset;

    return out;
}

// Fragment shader for outline/shadow effects
@fragment
fn fs_outline(input: VertexOutput) -> @location(0) vec4<f32> {
    let glyph_sample = textureSample(glyph_atlas, atlas_sampler, input.uv, i32(input.layer));

    // Simple outline effect
    return vec4<f32>(
        input.color.rgb,
        glyph_sample.a * input.color.a
    );
}
