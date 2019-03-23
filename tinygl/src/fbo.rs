use std::collections::HashMap;
use gl::types::*;
use super::texture::Texture;

struct FrameBufferOutput {
    size: (GLsizei, GLsizei),
    internal: GLenum,
    format: GLenum,
    datatype: GLenum,
    texture: Option<Texture>,
    index: usize
}

impl FrameBufferOutput {
    fn create(size: (GLsizei, GLsizei), internal: GLenum, format: GLenum, datatype: GLenum) -> Texture {
        let mut texture = Texture::new(gl::TEXTURE_2D);
        texture.bind_at(0);
        texture.filter(gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
        texture.filter(gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
        unsafe { texture.teximage(size, internal, format, datatype, std::ptr::null()); }
        texture
    }

    fn new(size: (GLsizei, GLsizei), internal: GLenum, format: GLenum, datatype: GLenum, index: usize) -> Self {
        FrameBufferOutput {
            size,
            internal,
            format,
            datatype,
            texture: Some(Self::create(size, internal, format, datatype)),
            index
        }
    }

    fn take(&mut self) -> Texture {
        let ret = self.texture.take();
        self.texture = Some(Self::create(self.size, self.internal, self.format, self.datatype));
        ret.unwrap()
    }
}

pub struct FrameBufferObject {
    size: (GLsizei, GLsizei),
    fbo: GLuint,
    depth_rb: Option<GLuint>,
    depth_tex: Option<Texture>,
    textures: HashMap<String, FrameBufferOutput>
}

impl FrameBufferObject {
    pub fn new(size: (GLsizei, GLsizei)) -> Self {
        let mut fbo = 0;
        unsafe { gl::GenFramebuffers(1, &mut fbo); }

        FrameBufferObject {
            size,
            fbo,
            depth_rb: None,
            depth_tex: None,
            textures: HashMap::new()
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.size.0 as u32, self.size.1 as u32)
    }

    pub fn add(&mut self, name: &str, internal: GLenum, format: GLenum, datatype: GLenum) {
        if self.textures.contains_key(name) {
            println!("Texture {} already attached", name);
            return;
        }

        let index = self.textures.len() as _;
        let texture = FrameBufferOutput::new(self.size, internal, format, datatype, index);

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0 + index as u32, gl::TEXTURE_2D, texture.texture.as_ref().unwrap().handle(), 0);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
        self.textures.insert(name.to_string(), texture);
    }

    pub fn add_depth_renderbuffer(&mut self) {
        if self.depth_rb.is_none() && self.depth_tex.is_none() {
            unsafe {
                let mut depth = 0;
                gl::GenRenderbuffers(1, &mut depth);
                gl::BindRenderbuffer(gl::RENDERBUFFER, depth);
                gl::RenderbufferStorage(gl::RENDERBUFFER, gl::DEPTH_COMPONENT24, self.size.0, self.size.1);
                gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
                gl::FramebufferRenderbuffer(gl::FRAMEBUFFER, gl::DEPTH_ATTACHMENT, gl::RENDERBUFFER, depth);
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

                self.depth_rb = Some(depth);
            }
        }
    }

    pub fn add_depth_texture(&mut self) {
        if self.depth_rb.is_none() && self.depth_tex.is_none() {
            let mut texture = Texture::new(gl::TEXTURE_2D);
            texture.bind_at(0);
            texture.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            texture.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);

            unsafe {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_COMPARE_MODE, gl::COMPARE_REF_TO_TEXTURE as _);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_COMPARE_FUNC, gl::LEQUAL as _);
                texture.teximage(self.size, gl::DEPTH_COMPONENT24, gl::DEPTH_COMPONENT, gl::UNSIGNED_INT, std::ptr::null());
                gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
                gl::FramebufferTexture2D(gl::FRAMEBUFFER, gl::DEPTH_ATTACHMENT, gl::TEXTURE_2D, texture.handle(), 0);
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            }

            self.depth_tex = Some(texture);
        }
    }

    pub fn depth_texture(&self) -> Option<&Texture> {
        self.depth_tex.as_ref()
    }

    pub fn depth_texture_mut(&mut self) -> Option<&mut Texture> {
        self.depth_tex.as_mut()
    }

    pub fn bind(&self) {
        let mut draw_buffers: Vec<GLenum> = Vec::new();
        for i in 0..self.textures.len() {
            draw_buffers.push((gl::COLOR_ATTACHMENT0 + i as u32) as _);
        }
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::DrawBuffers(draw_buffers.len() as _, draw_buffers.as_ptr());
            gl::Viewport(0, 0, self.size.0 as _, self.size.1 as _);
        }
    }

    pub fn unbind() {
        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, 0) }
    }

    pub unsafe fn read(&self, name: &str, dst: *mut std::ffi::c_void) {
        let texture = self.textures.get(name).expect("No such texture found");

        gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
        gl::Viewport(0, 0, self.size.0 as _, self.size.1 as _);
        gl::ReadBuffer((gl::COLOR_ATTACHMENT0 + texture.index as u32) as _);
        gl::ReadPixels(0, 0, self.size.0 as _, self.size.1 as _, texture.format, texture.datatype, dst);
        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
    }

    pub fn texture(&self, name: &str) -> Option<&Texture> {
        self.textures.get(name).map(|tex| tex.texture.as_ref().unwrap())
    }

    pub fn texture_mut(&mut self, name: &str) -> Option<&mut Texture> {
        self.textures.get_mut(name).map(|tex| tex.texture.as_mut().unwrap())
    }

    pub fn take(&mut self, name: &str) -> Option<Texture> {
        let ret = self.textures.get_mut(name).map(|tex| tex.take());
        if let Some(new_tex) = self.textures.get(name).as_ref() {
            let glid = new_tex.texture.as_ref().unwrap().handle();
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
                gl::FramebufferTexture2D(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0 + new_tex.index as u32, gl::TEXTURE_2D, glid, 0);
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            }
        }
        ret
    }
}

impl Drop for FrameBufferObject {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &mut self.fbo);
            if let Some(depth) = self.depth_rb.take() {
                gl::DeleteRenderbuffers(1, &depth);
            }
        }
    }
}
