extern crate imgui;
extern crate gl;
extern crate cgmath;

use imgui::{ImGui,Ui};
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
    pub fn new(imgui: &mut ImGui) -> Self {
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

        let prog1 = Program::new_versioned(vert_source, frag_source, 100);

        unsafe {
            let vbo = return_param(|x| gl::GenBuffers(1, x) );
            let ebo = return_param(|x| gl::GenBuffers(1, x) );

            let mut current_texture = 0;
            gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut current_texture);

            let font_texture = return_param(|x| gl::GenTextures(1, x));
            gl::BindTexture(gl::TEXTURE_2D, font_texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);

            imgui.prepare_texture(|handle| {
                gl::TexImage2D(gl::TEXTURE_2D, 0, gl::RGBA as _, handle.width as _, handle.height as _, 0, gl::RGBA, gl::UNSIGNED_BYTE, handle.pixels.as_ptr() as _);
            });

            gl::BindTexture(gl::TEXTURE_2D, current_texture as _);

            imgui.set_texture_id(font_texture as usize);

            Self{
                program: prog1,
                vbo,
                ebo,
                font_texture,
            }
        }
    }

    pub fn render<'ui>(&mut self, ui: Ui<'ui>) {
        use imgui::{ImDrawVert,ImDrawIdx};

        unsafe {
            gl::Enable(gl::BLEND);
            gl::Enable(gl::SCISSOR_TEST);
            gl::BlendEquation(gl::FUNC_ADD);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Disable(gl::CULL_FACE);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);

            let (width, height) = ui.imgui().display_size();
            let fb_width = width * ui.imgui().display_framebuffer_scale().0;
            let fb_height = height * ui.imgui().display_framebuffer_scale().1;

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

            ui.render::<_, ()>(|ui, mut draw_data| {
                draw_data.scale_clip_rects(ui.imgui().display_framebuffer_scale());

                for draw_list in &draw_data {
                    gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
                    gl::BufferData(gl::ARRAY_BUFFER, (draw_list.vtx_buffer.len() * mem::size_of::<ImDrawVert>()) as _, draw_list.vtx_buffer.as_ptr() as _, gl::STREAM_DRAW);

                    gl::VertexAttribPointer(ploc,   2, gl::FLOAT,         gl::FALSE, mem::size_of::<ImDrawVert>() as _, field_offset::<ImDrawVert, _, _>(|v| &v.pos) as _);
                    gl::VertexAttribPointer(puv,    2, gl::FLOAT,         gl::FALSE, mem::size_of::<ImDrawVert>() as _, field_offset::<ImDrawVert, _, _>(|v| &v.uv) as _);
                    gl::VertexAttribPointer(pcolor, 4, gl::UNSIGNED_BYTE, gl::TRUE,  mem::size_of::<ImDrawVert>() as _, field_offset::<ImDrawVert, _, _>(|v| &v.col) as _);

                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);
                    gl::BufferData(gl::ELEMENT_ARRAY_BUFFER, (draw_list.idx_buffer.len() * mem::size_of::<ImDrawIdx>()) as _, draw_list.idx_buffer.as_ptr() as _, gl::STREAM_DRAW);

                    let mut idx_start = 0;
                    for cmd in draw_list.cmd_buffer {
                        if let Some(_callback) = cmd.user_callback {
                            unimplemented!("Haven't implemented user callbacks yet");
                        } else {
                            gl::Scissor(cmd.clip_rect.x as GLint,
                                        (fb_height - cmd.clip_rect.w) as GLint,
                                        (cmd.clip_rect.z - cmd.clip_rect.x) as GLint,
                                        (cmd.clip_rect.w - cmd.clip_rect.y) as GLint);
                            gl::BindTexture(gl::TEXTURE_2D, cmd.texture_id as _);
                            gl::DrawElements(gl::TRIANGLES, cmd.elem_count as _, if mem::size_of::<ImDrawIdx>() == 2 { gl::UNSIGNED_SHORT } else { gl::UNSIGNED_INT }, idx_start as _);
                        }
                        idx_start += cmd.elem_count * mem::size_of::<ImDrawIdx>() as u32;
                    }
                }

                Ok(())
            });

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

fn return_param<T, F>(f: F) -> T where F: FnOnce(&mut T) {
    let mut val = unsafe{ mem::uninitialized() };
    f(&mut val);
    val
}
