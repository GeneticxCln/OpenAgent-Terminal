# [FEATURE] Complete WGPU Rendering Backend Implementation

## Priority  
🟠 **High** - Modern rendering pipeline and cross-platform compatibility

## Description
The WGPU rendering backend is partially implemented but missing critical functionality. Multiple TODO comments in shaders and renderer code indicate incomplete features needed for a fully functional terminal.

## Current Status

Status update (v0.16.1): WGPU is now the default renderer. Recent fixes resolved WGSL shader issues (GLSL mod() compatibility and fragment input built-ins). Core terminal content and text rendering paths are functional for default usage. The items below capture remaining work for advanced features and performance tooling.

The basic WGPU infrastructure exists, but core rendering operations are not implemented:

### Missing Critical Features
1. **Text Rendering** - Glyph atlas sampling and cell-based text rendering
2. **Cursor Rendering** - Proper cursor positioning and blinking animation  
3. **Terminal Content Rendering** - Cell grid rendering with colors and attributes
4. **Performance HUD** - Debug overlay for monitoring frame times and GPU metrics
5. **UI Sprite Support** - Sprite rendering for UI elements (sprites, filters)

### Locations with TODOs

#### WGSL Shaders (`src/renderer/shaders/terminal.wgsl`)
- **Line 59**: "TODO: Sample from glyph atlas texture" - Missing texture sampling for text
- **Line 118**: "TODO: Get cursor position from uniform buffer" - Cursor positioning incomplete

#### WGPU Renderer (`src/renderer/wgpu_renderer.rs`)  
- **Line 260**: "TODO: Implement terminal rendering" - Core terminal content rendering missing
- **Line 271**: "TODO: Implement HUD rendering" - Performance HUD not implemented

#### Display Backend (`openagent-terminal/src/display/mod.rs`)
- **Line 2324**: "TODO: implement for WGPU backend" - UI sprite staging missing  
- **Line 2335**: "TODO: implement for WGPU backend" - Sprite filter control missing

## Implementation Plan

### Phase 1: Text Rendering Foundation
1. **Glyph Atlas Integration**
   ```wgsl
   // Add texture sampling to fragment shader
   @group(0) @binding(0) var glyph_atlas: texture_2d<f32>;
   @group(0) @binding(1) var atlas_sampler: sampler;
   
   @fragment
   fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
       let glyph_color = textureSample(glyph_atlas, atlas_sampler, input.tex_coords);
       return input.color * glyph_color;
   }
   ```

2. **Cell Data Uniform Buffer**
   ```rust
   #[repr(C)]
   struct CellData {
       character: u32,
       foreground: [f32; 4],
       background: [f32; 4], 
       glyph_coords: [f32; 4], // atlas UV coordinates
   }
   ```

### Phase 2: Terminal Content Rendering  
1. **Implement `render_terminal_content()`**
   ```rust
   fn render_terminal_content(&self, render_pass: &mut wgpu::RenderPass, terminal_state: &TerminalState) {
       // Upload cell data to uniform buffer
       self.update_cell_uniforms(terminal_state);
       
       // Set pipeline and bindings
       render_pass.set_pipeline(&self.text_pipeline);
       render_pass.set_bind_group(0, &self.atlas_bind_group, &[]);
       render_pass.set_bind_group(1, &self.cell_data_bind_group, &[]);
       
       // Draw instanced quads for each cell
       let instance_count = (terminal_state.viewport.cols * terminal_state.viewport.rows) as u32;
       render_pass.draw(0..6, 0..instance_count); // 6 vertices per quad
   }
   ```

2. **Cell Grid Shader Enhancement**
   ```wgsl
   struct CellUniforms {
       cells: array<CellData, 8192>, // Max cells per screen
       cursor_position: vec2<u32>,
       cursor_blink: f32,
   }
   @group(1) @binding(0) var<uniform> cell_uniforms: CellUniforms;
   ```

### Phase 3: Cursor and UI Elements
1. **Dynamic Cursor Positioning** 
   ```wgsl
   @vertex
   fn vs_cursor(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
       // Get cursor position from uniform buffer  
       let cursor_pos = vec2<f32>(
           f32(cell_uniforms.cursor_position.x),
           f32(cell_uniforms.cursor_position.y)
       );
       
       // Generate cursor quad at correct position
       let x = f32(vertex_index & 1u);
       let y = f32((vertex_index >> 1u) & 1u);
       
       let world_pos = cursor_pos * vec2<f32>(pc.cell_width, pc.cell_height);
       // ... transform to NDC
   }
   ```

2. **UI Sprite Rendering**
   ```rust
   impl WgpuRenderer {
       fn stage_ui_sprite(&mut self, sprite: UiSprite) {
           self.ui_sprites.push(sprite);
       }
       
       fn set_sprite_filter_nearest(&mut self, nearest: bool) {
           let filter = if nearest { 
               wgpu::FilterMode::Nearest 
           } else { 
               wgpu::FilterMode::Linear 
           };
           
           self.sprite_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
               mag_filter: filter,
               min_filter: filter,
               ..Default::default()
           });
       }
   }
   ```

### Phase 4: Performance and Debugging
1. **Performance HUD Implementation**
   ```rust  
   fn render_performance_hud(&self, render_pass: &mut wgpu::RenderPass) {
       // Background quad
       render_pass.set_pipeline(&self.hud_pipeline);
       render_pass.draw(0..6, 0..1);
       
       // Text overlay with metrics
       self.draw_hud_text(&format!("Frame Time: {:.2}ms", self.metrics.frame_time_ms));
       self.draw_hud_text(&format!("Draw Calls: {}", self.metrics.draw_calls));
       
       // Frame time graph
       self.draw_frame_time_graph(render_pass);
   }
   ```

2. **GPU Memory Monitoring**
   ```rust
   fn update_gpu_metrics(&mut self) {
       // Track texture memory usage
       self.metrics.texture_memory_mb = self.calculate_texture_memory();
       
       // Track buffer memory usage  
       self.metrics.buffer_memory_mb = self.calculate_buffer_memory();
       
       // GPU utilization (if available)
       self.metrics.gpu_utilization = self.query_gpu_utilization();
   }
   ```

## Technical Details

### Rendering Pipeline
1. **Vertex Generation**: Generate quads for each terminal cell using instanced rendering
2. **Atlas Sampling**: Sample from glyph atlas texture using computed UV coordinates  
3. **Color Blending**: Combine glyph alpha with foreground/background colors
4. **Cursor Overlay**: Render blinking cursor with time-based animation
5. **UI Elements**: Render sprites and performance overlays

### Performance Considerations
- Use instanced rendering to minimize draw calls
- Pack cell data efficiently in uniform buffers
- Implement atlas eviction and reloading for dynamic glyph sets
- Use compute shaders for complex text shaping if needed

### Cross-Platform Compatibility
- Test on Vulkan, Metal, DX12, and WebGL backends
- Handle different GPU memory constraints
- Ensure shader compatibility across WGSL compilation targets

## Files to Modify

### Core Renderer
- `src/renderer/wgpu_renderer.rs`
- `src/renderer/shaders/terminal.wgsl`

### Integration Layer
- `openagent-terminal/src/display/mod.rs`
- `openagent-terminal/src/renderer/` (if separate backend module)

### Glyph Management  
- Atlas management and loading code
- Font rasterization integration

## Testing Requirements
- [ ] Text rendering works with various fonts and sizes
- [ ] Cursor positioning accurate across different cell sizes
- [ ] Performance acceptable on integrated GPUs
- [ ] Cross-platform shader compilation successful
- [ ] Memory usage stays within reasonable bounds
- [ ] HUD accurately reflects performance metrics

## Labels
- `priority/high`
- `type/feature`
- `component/renderer`
- `graphics/wgpu`

## Definition of Done
- [ ] All terminal content renders correctly  
- [ ] Cursor positioning and blinking implemented
- [ ] UI sprite rendering functional
- [ ] Performance HUD shows accurate metrics
- [ ] Cross-platform testing complete
- [ ] All TODO comments resolved
- [ ] Performance benchmarks acceptable
- [ ] Memory leak testing passed
