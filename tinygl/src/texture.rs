use gl::types::*;

const NUM_TEX_TARGETS: usize = 128;
static mut CURRENT_TEXTURE_UNIT: usize = 0;
static mut BOUND_TEX_TARGETS: [GLenum; NUM_TEX_TARGETS] = [0; NUM_TEX_TARGETS];

pub struct Texture {
    tex: GLuint,
    target: GLenum,
    size: Option<(GLsizei, GLsizei)>,
}

impl Texture {
    pub fn new(target: GLenum) -> Texture {
        let mut tex = 0;
        unsafe { gl::GenTextures(1, &mut tex); }
        Texture {
            tex,
            target,
            size: None
        }
    }

    pub fn from_data_2d(data: &Vec<u8>, size: (i32, i32)) -> Self {
        let mut ret = Self::new(gl::TEXTURE_2D);
        unsafe { ret.teximage((size.0 as _, size.1 as _), gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE, data.as_ptr() as _) }
        ret.filter(gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR);
        ret.filter(gl::TEXTURE_MAG_FILTER, gl::LINEAR);
        ret.gen_mipmaps();
        ret
    }

    pub fn handle(&self) -> GLuint {
        self.tex
    }

    fn new_texture_unit_binding(&self) {
        // keep track of old binding to that unit and maybe un-bind
        let curr_unit = unsafe { CURRENT_TEXTURE_UNIT };
        if curr_unit < NUM_TEX_TARGETS {
            let bound = unsafe { BOUND_TEX_TARGETS[curr_unit] };
            if bound != self.target {
                if bound != 0 {
                    unsafe { gl::BindTexture(bound, 0); }
                }
                unsafe { BOUND_TEX_TARGETS[curr_unit] = self.target; }
            }
        }
    }

    fn bind(&self) {
        self.new_texture_unit_binding();
        unsafe { gl::BindTexture(self.target, self.tex); }
    }

    pub fn bind_at(&self, unit: u32) {
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0 + unit);
            unsafe { CURRENT_TEXTURE_UNIT = unit as _; }
            self.new_texture_unit_binding();
            gl::BindTexture(self.target, self.tex);
        }
    }

    pub fn filter(&mut self, minmag: GLenum, value: GLenum) {
        self.bind();
        unsafe { gl::TexParameteri(self.target, minmag, value as _); }
    }

    pub fn wrap(&mut self, wrap: GLenum, value: GLenum) {
        self.bind();
        unsafe { gl::TexParameteri(self.target, wrap, value as _); }
    }

    pub fn gen_mipmaps(&mut self) {
        self.bind();
        unsafe { gl::GenerateMipmap(self.target); }
    }

    pub unsafe fn teximage(&mut self, size: (GLsizei, GLsizei), internal: GLenum, format: GLenum, datatype: GLenum, data: *const GLvoid) {
        self.bind();
        gl::TexImage2D(self.target, 0, internal as _, size.0, size.1, 0, format, datatype, data);
        self.size = Some(size);
    }

    pub unsafe fn teximage_layer(&mut self, size: (GLsizei, GLsizei), layers: GLsizei, internal: GLenum, format: GLenum, datatype: GLenum, data: *const GLvoid) {
        self.bind();
        gl::TexImage3D(self.target, 0, internal as _, size.0, size.1, layers, 0, format, datatype, data);
        self.size = Some(size);
    }

    pub fn size(&self) -> Option<(GLsizei, GLsizei)> {
        self.size
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        if self.tex > 0 {
            unsafe { gl::DeleteTextures(1, &self.tex); }
        }
    }
}
struct OffscreenTexture {
    size: (GLsizei, GLsizei),
    internal: GLenum,
    format: GLenum,
    datatype: GLenum,
    texture: Option<Texture>,
    index: usize
}

