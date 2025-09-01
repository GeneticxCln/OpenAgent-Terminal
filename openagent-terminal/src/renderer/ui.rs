use crate::display::color::Rgb;
use crate::display::SizeInfo;
use crate::gl;
use crate::gl::types::*;
use crate::renderer::shader::{ShaderError, ShaderProgram, ShaderVersion};

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

#[derive(Debug)]
pub struct UiGlRenderer {
    vao: GLuint,
    vbo: GLuint,
    program: ShaderProgram,
    u_origin: GLint,
    u_size: GLint,
    u_radius: GLint,
    u_color: GLint,
}

const UI_SHADER_V: &str = include_str!("../../res/ui_rect.v.glsl");
const UI_SHADER_F: &str = include_str!("../../res/ui_rect.f.glsl");

impl UiGlRenderer {
    pub fn new(shader_version: ShaderVersion) -> Result<Self, ShaderError> {
        let program = ShaderProgram::new(shader_version, None, UI_SHADER_V, UI_SHADER_F)?;
        let u_origin = program.get_uniform_location(c"uOrigin")?;
        let u_size = program.get_uniform_location(c"uSize")?;
        let u_radius = program.get_uniform_location(c"uRadius")?;
        let u_color = program.get_uniform_location(c"uColor")?;

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

        Ok(Self { vao, vbo, program, u_origin, u_size, u_radius, u_color })
    }

    pub fn draw(&mut self, size_info: &SizeInfo, shapes: &[UiRoundedRect]) {
        if shapes.is_empty() {
            return;
        }
        let half_w = size_info.width() / 2.0;
        let half_h = size_info.height() / 2.0;

        unsafe {
            gl::UseProgram(self.program.id());
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
        }

        for s in shapes {
            let ndc_x = s.x / half_w - 1.0;
            let ndc_y = -s.y / half_h + 1.0;
            let ndc_w = s.width / half_w;
            let ndc_h = s.height / half_h;

            let quad: [f32; 12] = [
                ndc_x,
                ndc_y,
                ndc_x,
                ndc_y - ndc_h,
                ndc_x + ndc_w,
                ndc_y,
                ndc_x + ndc_w,
                ndc_y,
                ndc_x + ndc_w,
                ndc_y - ndc_h,
                ndc_x,
                ndc_y - ndc_h,
            ];

            unsafe {
                gl::Uniform2f(self.u_origin, s.x, s.y);
                gl::Uniform2f(self.u_size, s.width, s.height);
                gl::Uniform1f(self.u_radius, s.radius);
                let (r, g, b) = s.color.as_tuple();
                gl::Uniform4f(
                    self.u_color,
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
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindVertexArray(0);
            gl::UseProgram(0);
        }
    }
}

impl Drop for UiGlRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}
