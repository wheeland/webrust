extern crate imgui;
extern crate gl;
extern crate cgmath;

use imgui::{Context,Ui, DrawVert, DrawIdx, DrawCmd};
use std::mem;
use gl::types::*;

use super::tinygl::*;

pub struct Renderer {
    program: Program,
    vbo: GLuint,
    ebo: GLuint,
    font_texture: GLuint,
}

impl Renderer {
    pub fn new(imgui: &mut Context) -> Self {
        let vert_source = "
            uniform mat4 ProjMtx;
            attribute vec2 Position;
            attribute vec2 UV;
            attribute vec4 Color;
            varying vec2 Frag_UV;
            varying vec4 Frag_Color;
            void main()
            {
                Frag_UV = UV;
                Frag_Color = Color;
                gl_Position = ProjMtx * vec4(Position.xy, 0.0, 1.0);
            }";

        let frag_source = "
            uniform sampler2D Texture;
            varying vec2 Frag_UV;
            varying vec4 Frag_Color;
            void main()
            {
                gl_FragColor = Frag_Color * texture2D(Texture, Frag_UV.st);
            }";

        let program = Program::new_versioned(vert_source, frag_source, 100);

        unsafe {
            let vbo = return_param(|x| gl::GenBuffers(1, x) );
            let ebo = return_param(|x| gl::GenBuffers(1, x) );

            let mut current_texture = 0;
            gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut current_texture);

            let font_texture = return_param(|x| gl::GenTextures(1, x));
            gl::BindTexture(gl::TEXTURE_2D, font_texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
            
            let font_data = imgui.fonts().build_rgba32_texture();
            gl::TexImage2D(gl::TEXTURE_2D, 0, gl::RGBA as _, font_data.width as _, font_data.height as _, 0, gl::RGBA, gl::UNSIGNED_BYTE, font_data.data.as_ptr() as _);

            gl::BindTexture(gl::TEXTURE_2D, current_texture as _);

            Self {
                program,
                vbo,
                ebo,
                font_texture,
            }
        }
    }

    pub fn render(&mut self, imgui: &mut Context) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::Enable(gl::SCISSOR_TEST);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Disable(gl::CULL_FACE);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            
            let draw_data = imgui.render();

            let [width, height] = draw_data.display_size;
            let fb_width = width * draw_data.framebuffer_scale[0];
            let fb_height = height * draw_data.framebuffer_scale[1];

            gl::Viewport(0, 0, fb_width as _, fb_height as _);
            let matrix = cgmath::Matrix4::new(
                 2.0 / width as f32, 0.0,                     0.0, 0.0,
                 0.0,                2.0 / -(height as f32),  0.0, 0.0,
                 0.0,                0.0,                    -1.0, 0.0,
                -1.0,                1.0,                     0.0, 1.0,
            );
            self.program.bind();
            self.program.uniform("Texture", Uniform::Signed(0));
            self.program.uniform("ProjMtx", Uniform::Mat4(matrix));

            let ploc = self.program.vertex_attrib_location("Position").unwrap_or(0);
            let puv = self.program.vertex_attrib_location("UV").unwrap_or(0);
            let pcolor = self.program.vertex_attrib_location("Color").unwrap_or(0);
            gl::EnableVertexAttribArray(ploc);
            gl::EnableVertexAttribArray(puv);
            gl::EnableVertexAttribArray(pcolor);

            // draw_data.scale_clip_rects(draw_data.framebuffer_scale.into());

            for draw_list in draw_data.draw_lists() {
                gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
                gl::BufferData(gl::ARRAY_BUFFER, (draw_list.vtx_buffer().len() * mem::size_of::<DrawVert>()) as _, draw_list.vtx_buffer().as_ptr() as _, gl::STREAM_DRAW);

                gl::VertexAttribPointer(ploc,   2, gl::FLOAT,         gl::FALSE, mem::size_of::<DrawVert>() as _, field_offset::<DrawVert, _, _>(|v| &v.pos) as _);
                gl::VertexAttribPointer(puv,    2, gl::FLOAT,         gl::FALSE, mem::size_of::<DrawVert>() as _, field_offset::<DrawVert, _, _>(|v| &v.uv) as _);
                gl::VertexAttribPointer(pcolor, 4, gl::UNSIGNED_BYTE, gl::TRUE,  mem::size_of::<DrawVert>() as _, field_offset::<DrawVert, _, _>(|v| &v.col) as _);

                gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);
                gl::BufferData(gl::ELEMENT_ARRAY_BUFFER, (draw_list.idx_buffer().len() * mem::size_of::<DrawIdx>()) as _, draw_list.idx_buffer().as_ptr() as _, gl::STREAM_DRAW);

                let mut idx_start = 0;
                for cmd in draw_list.commands() {
                    match cmd {
                        DrawCmd::Elements { count, cmd_params } => {
                            gl::Scissor(cmd_params.clip_rect[0] as GLint,
                                        (fb_height - cmd_params.clip_rect[3]) as GLint,
                                        (cmd_params.clip_rect[2] - cmd_params.clip_rect[0]) as GLint,
                                        (cmd_params.clip_rect[3] - cmd_params.clip_rect[1]) as GLint);
                            let texture = if cmd_params.texture_id.id() == 0 {
                                self.font_texture
                            } else {
                                unimplemented!("no support for custom textures yet")
                            };
                            gl::BindTexture(gl::TEXTURE_2D, texture as _);
                            gl::DrawElements(gl::TRIANGLES, count as _, if mem::size_of::<DrawIdx>() == 2 { gl::UNSIGNED_SHORT } else { gl::UNSIGNED_INT }, idx_start as _);
                            idx_start += count * mem::size_of::<DrawIdx>();
                        },
                        DrawCmd::ResetRenderState => unimplemented!("Haven't implemented user callbacks yet"),
                        DrawCmd::RawCallback { .. } => unimplemented!("Haven't implemented user callbacks yet"),
                    }
                }
            }

            gl::DisableVertexAttribArray(ploc);
            gl::DisableVertexAttribArray(puv);
            gl::DisableVertexAttribArray(pcolor);

            gl::Disable(gl::SCISSOR_TEST);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteBuffers(1, &self.ebo);
            gl::DeleteTextures(1, &self.font_texture);
        }
    }
}

fn field_offset<T, U, F: for<'a> FnOnce(&'a T) -> &'a U>(f: F) -> usize {
    unsafe {
        let instance = mem::uninitialized::<T>();

        let offset = {
            let field: &U = f(&instance);
            field as *const U as usize - &instance as *const T as usize
        };

        mem::forget(instance);

        offset
    }
}

fn return_param<T, F>(f: F) -> T where F: FnOnce(&mut T), T: Default {
    let mut val = T::default();
    f(&mut val);
    val
}
