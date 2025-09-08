use crate::display::color::Rgb;

#[derive(Clone, Copy, Debug)]
pub struct UiRoundedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub radius: f32,
    pub color: Rgb,
    pub alpha: f32,
}

impl UiRoundedRect {
    pub fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        color: Rgb,
        alpha: f32,
    ) -> Self {
        Self { x, y, width, height, radius, color, alpha }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct UiSprite {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    // UV rect in normalized atlas coordinates (0..1)
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    pub tint: Rgb,
    pub alpha: f32,
    /// Optional per-sprite filter override: true=NEAREST, false=LINEAR. None => default behavior.
    pub filter_nearest: Option<bool>,
}

impl UiSprite {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        uv_x: f32,
        uv_y: f32,
        uv_w: f32,
        uv_h: f32,
        tint: crate::display::color::Rgb,
        alpha: f32,
        filter_nearest: Option<bool>,
    ) -> Self {
        Self { x, y, width, height, uv_x, uv_y, uv_w, uv_h, tint, alpha, filter_nearest }
    }
}
/*
in vec2 vUV;
out vec4 FragColor;
uniform sampler2D uTex;
uniform vec4 uTint; // rgb + alpha
void main() {
    vec4 tex = texture(uTex, vUV);
    FragColor = vec4(tex.rgb * uTint.rgb, tex.a * uTint.a);
}
"#;

    pub fn new(shader_version: ShaderVersion) -> Result<Self, ShaderError> {
        let program = ShaderProgram::new(shader_version, None, UI_SPRITE_V, UI_SPRITE_F)?;
        let u_origin = program.get_uniform_location(c"uOrigin")?;
        let u_size = program.get_uniform_location(c"uSize")?;
        let u_uv_rect = program.get_uniform_location(c"uUvRect")?;
        let u_tint = program.get_uniform_location(c"uTint")?;
        let u_viewport = program.get_uniform_location(c"uViewport")?;

        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::VertexAttribPointer(
                0,
                2,
                gl::FLOAT,
                gl::FALSE,
                (std::mem::size_of::<f32>() * 2) as i32,
                std::ptr::null(),
            );
            gl::EnableVertexAttribArray(0);
            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }

        // Build a multi-icon atlas: 9 slots horizontally, 16x16 each
        // Slot order must match palette mapping
        let icon_names: [&str; 9] = [
            "action",   // 0 magnifier/search
            "workflow", // 1 branching
            "tab",      // 2 plus
            "split_v",  // 3 vertical split
            "split_h",  // 4 horizontal split
            "focus",    // 5 target
            "zoom",     // 6 square
            "blocks",   // 7 grid
            "gear",     // 8 settings gear
        ];
        let tile = 16usize;
        let atlas_w = icon_names.len() * tile;
        let atlas_h = tile;
        let mut pixels = vec![0u8; atlas_w * atlas_h * 4];
        // PNG loader with downsample
        fn load_png_rgba(path: &str) -> Option<(u32, u32, Vec<u8>)> {
            #[cfg(target_os = "macos")]
            {
                let _ = path;
                return None;
            }
            #[cfg(not(target_os = "macos"))]
            {
                let decoder = png::Decoder::new(std::fs::File::open(path).ok()?);
                let mut reader = decoder.read_info().ok()?;
                let mut buf = vec![0; reader.output_buffer_size()];
                let info = reader.next_frame(&mut buf).ok()?;
                Some((info.width, info.height, buf[..info.buffer_size()].to_vec()))
            }
        }
        fn downsample_to_16(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
            if rgba.len() < (w * h * 4) as usize {
                return None;
            }
            if w == 16 && h == 16 {
                return Some(rgba.to_vec());
            }
            let sx = (w / 16).max(1);
            let sy = (h / 16).max(1);
            let mut out = vec![0u8; 16 * 16 * 4];
            for y in 0..16u32 {
                for x in 0..16u32 {
                    let src_x = (x * sx).min(w - 1);
                    let src_y = (y * sy).min(h - 1);
                    let src_idx = ((src_y * w + src_x) * 4) as usize;
                    let dst_idx = ((y * 16 + x) * 4) as usize;
                    out[dst_idx..dst_idx + 4].copy_from_slice(&rgba[src_idx..src_idx + 4]);
                }
            }
            Some(out)
        }
        // Helpers
        let mut put = |tx: usize, x: usize, y: usize, rgba: [u8; 4]| {
            if x >= tile || y >= tile {
                return;
            }
            let ax = tx * tile + x;
            let idx = (y * atlas_w + ax) * 4;
            pixels[idx..idx + 4].copy_from_slice(&rgba);
        };
        let draw_rect =
            |tx: usize,
             x0: usize,
             y0: usize,
             w: usize,
             h: usize,
             rgba: [u8; 4],
             put: &mut dyn FnMut(usize, usize, usize, [u8; 4])| {
                for yy in y0..(y0 + h).min(tile) {
                    for xx in x0..(x0 + w).min(tile) {
                        put(tx, xx, yy, rgba);
                    }
                }
            };
        let draw_circle =
            |tx: usize,
             cx: f32,
             cy: f32,
             r: f32,
             rgba: [u8; 4],
             put: &mut dyn FnMut(usize, usize, usize, [u8; 4])| {
                let r2 = r * r;
                for y in 0..tile {
                    for x in 0..tile {
                        let dx = x as f32 - cx;
                        let dy = y as f32 - cy;
                        if dx * dx + dy * dy <= r2 {
                            put(tx, x, y, rgba);
                        }
                    }
                }
            };
        // Build each slot
        for (i, name) in icon_names.iter().enumerate() {
            let path = format!("extra/icons/palette_{}.png", name);
            let loaded = load_png_rgba(&path).and_then(|(w, h, img)| downsample_to_16(w, h, &img));
            if let Some(img) = loaded {
                for y in 0..tile {
                    for x in 0..tile {
                        let src = (y * tile + x) * 4;
                        put(i, x, y, [img[src], img[src + 1], img[src + 2], img[src + 3]]);
                    }
                }
                continue;
            }
            // Procedural fallback
            match *name {
                "action" => {
                    // magnifier: circle + diagonal handle
                    draw_circle(i, 7.0, 7.0, 5.0, [255, 255, 255, 255], &mut put);
                    for t in 0..5 {
                        let x = 10 + t;
                        let y = 10 + t;
                        if x < tile && y < tile {
                            put(i, x, y, [255, 255, 255, 255]);
                        }
                    }
                },
                "workflow" => {
                    // three nodes
                    draw_circle(i, 4.0, 4.0, 2.0, [255, 255, 255, 255], &mut put);
                    draw_circle(i, 12.0, 4.0, 2.0, [255, 255, 255, 255], &mut put);
                    draw_circle(i, 8.0, 12.0, 2.0, [255, 255, 255, 255], &mut put);
                    // connectors
                    draw_rect(i, 4, 3, 8, 1, [255, 255, 255, 255], &mut put);
                    draw_rect(i, 7, 4, 2, 8, [255, 255, 255, 255], &mut put);
                },
                "tab" => {
                    // plus
                    draw_rect(i, 7, 3, 2, 10, [255, 255, 255, 255], &mut put);
                    draw_rect(i, 3, 7, 10, 2, [255, 255, 255, 255], &mut put);
                },
                "split_v" => {
                    draw_rect(i, 7, 1, 2, 14, [255, 255, 255, 255], &mut put);
                },
                "split_h" => {
                    draw_rect(i, 1, 7, 14, 2, [255, 255, 255, 255], &mut put);
                },
                "focus" => {
                    draw_circle(i, 8.0, 8.0, 6.5, [255, 255, 255, 64], &mut put);
                    draw_rect(i, 0, 7, 16, 2, [255, 255, 255, 255], &mut put);
                    draw_rect(i, 7, 0, 2, 16, [255, 255, 255, 255], &mut put);
                },
                "zoom" => {
                    // square border
                    draw_rect(i, 3, 3, 10, 2, [255, 255, 255, 255], &mut put);
                    draw_rect(i, 3, 11, 10, 2, [255, 255, 255, 255], &mut put);
                    draw_rect(i, 3, 3, 2, 10, [255, 255, 255, 255], &mut put);
                    draw_rect(i, 11, 3, 2, 10, [255, 255, 255, 255], &mut put);
                },
                "blocks" => {
                    for by in 0..3 {
                        for bx in 0..3 {
                            draw_rect(
                                i,
                                3 + bx * 4,
                                3 + by * 4,
                                2,
                                2,
                                [255, 255, 255, 255],
                                &mut put,
                            );
                        }
                    }
                },
                "gear" => {
                    // Simple procedural gear: outer circle, inner hole, and 8 teeth rectangles
                    // Outer ring
                    draw_circle(i, 8.0, 8.0, 6.5, [255, 255, 255, 255], &mut put);
                    // Inner hole (erase with transparent by overdrawing with alpha 0)
                    // Since we can't erase easily, draw a slightly dimmer hole using low alpha
                    for y in 0..tile {
                        for x in 0..tile {
                            let dx = x as f32 - 8.0;
                            let dy = y as f32 - 8.0;
                            let r2 = dx * dx + dy * dy;
                            if r2 <= 3.5 * 3.5 {
                                put(i, x, y, [255, 255, 255, 32]);
                            }
                        }
                    }
                    // Teeth at 8 directions
                    let mut tooth = |tx: usize, cx: usize, cy: usize, w: usize, h: usize| {
                        draw_rect(tx, cx, cy, w, h, [255, 255, 255, 255], &mut put)
                    };
                    // Up, Down, Left, Right
                    tooth(i, 7, 0, 2, 3);
                    tooth(i, 7, 13, 2, 3);
                    tooth(i, 0, 7, 3, 2);
                    tooth(i, 13, 7, 3, 2);
                    // Diagonals
                    for t in 0..3 {
                        let a = t as usize;
                        if 2 + a < tile { put(i, 2 + a, 2 + a, [255,255,255,255]); }
                        if 12 >= a && 2 + a < tile { put(i, 12 - a, 2 + a, [255,255,255,255]); }
                        if 2 + a < tile && 12 >= a { put(i, 2 + a, 12 - a, [255,255,255,255]); }
                        if 12 >= a { put(i, 12 - a, 12 - a, [255,255,255,255]); }
                    }
                },
                _ => {},
            }
        }
        let mut tex: GLuint = 0;
        unsafe {
            gl::GenTextures(1, &mut tex);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                atlas_w as i32,
                atlas_h as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                pixels.as_ptr().cast(),
            );
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }

        Ok(Self {
            vao,
            vbo,
            program,
            texture: tex,
            u_origin,
            u_size,
            u_uv_rect,
            u_tint,
            u_viewport,
        })
    }

    pub fn draw(&mut self, size_info: &SizeInfo, sprites: &[UiSprite]) {
        if sprites.is_empty() {
            return;
        }
        unsafe {
            gl::UseProgram(self.program.id());
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::Uniform2f(self.u_viewport, size_info.width(), size_info.height());
        }
        for s in sprites {
            let quad: [f32; 12] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];

            // Per-sprite filter (default to LINEAR when unspecified)
            self.set_filter_nearest(s.filter_nearest.unwrap_or(false));

            unsafe {
                gl::Uniform2f(self.u_origin, s.x, s.y);
                gl::Uniform2f(self.u_size, s.width, s.height);
                gl::Uniform4f(self.u_uv_rect, s.uv_x, s.uv_y, s.uv_w, s.uv_h);
                let (r, g, b) = s.tint.as_tuple();
                gl::Uniform4f(
                    self.u_tint,
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    s.alpha,
                );
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (quad.len() * std::mem::size_of::<f32>()) as isize,
                    quad.as_ptr().cast(),
                    gl::STREAM_DRAW,
                );
                gl::DrawArrays(gl::TRIANGLES, 0, 6);
            }
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
            gl::UseProgram(0);
        }
    }
}

impl UiSpriteGlRenderer {
    /// Set texture filtering to nearest (true) or linear (false).
    pub fn set_filter_nearest(&mut self, nearest: bool) {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            let filter = if nearest { gl::NEAREST as i32 } else { gl::LINEAR as i32 };
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, filter);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, filter);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
    }
}
*/
