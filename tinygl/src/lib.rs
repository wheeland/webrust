extern crate gl;
extern crate cgmath;

use std::cell::RefCell;
use std::collections::HashMap;
use gl::types::*;
use cgmath::prelude::*;

pub mod shapes;

struct BufferBase {
    target: GLenum,
    buffer: GLuint
}

impl BufferBase {
    fn new<T>(target: GLenum, usage: GLenum, data: &Vec<T>) -> BufferBase {
        let mut buffer = 0;

        unsafe {
            gl::GenBuffers(1, &mut buffer);
            gl::BindBuffer(target, buffer);
            gl::BufferData(target,
                           (data.len() * std::mem::size_of::<T>()) as GLsizeiptr,
                           data.as_ptr() as *const GLvoid,
                           usage);
            gl::BindBuffer(target, 0);
        }

        BufferBase {
            target,
            buffer
        }
    }

    fn bind(&self) {
        if self.buffer != 0 {
            unsafe {
                gl::BindBuffer(self.target, self.buffer);
            }
        }
    }
}

impl Drop for BufferBase {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.buffer);
        }
    }
}

pub struct VertexBuffer {
    buffer: BufferBase
}

pub struct IndexBuffer {
    buffer: BufferBase,
    idx_type: GLenum,
    size: usize
}

impl VertexBuffer {
    pub fn from<T>(data: &Vec<T>) -> VertexBuffer {
        VertexBuffer {
            buffer: BufferBase::new(gl::ARRAY_BUFFER, gl::STATIC_DRAW, data)
        }
    }

    pub fn bind(&self) {
        self.buffer.bind();
    }
}

impl IndexBuffer {
    pub fn from32(data: &Vec<u32>) -> IndexBuffer {
        IndexBuffer {
            buffer: BufferBase::new(gl::ELEMENT_ARRAY_BUFFER, gl::STATIC_DRAW, data),
            idx_type: gl::UNSIGNED_INT,
            size: data.len()
        }
    }

    pub fn from16(data: &Vec<u16>) -> IndexBuffer {
        IndexBuffer {
            buffer: BufferBase::new(gl::ELEMENT_ARRAY_BUFFER, gl::STATIC_DRAW, data),
            idx_type: gl::UNSIGNED_SHORT,
            size: data.len()
        }
    }

    pub fn bind(&self) {
        self.buffer.bind();
    }

    pub fn draw(&self, mode: GLenum, count: GLsizei, ofs: GLsizei) {
        self.buffer.bind();
        unsafe { gl::DrawElements(mode, count, self.idx_type, ofs as _); }
    }

    pub fn draw_all(&self, mode: GLenum) -> usize {
        self.draw(mode, self.size as _, 0);
        self.size
    }

    pub fn count(&self) -> usize {
        self.size
    }
}

pub enum Uniform {
    Signed(i32),
    Unsigned(u32),

    Float(f32),
    Vec2(cgmath::Vector2<f32>),
    Vec3(cgmath::Vector3<f32>),
    Vec4(cgmath::Vector4<f32>),

    Mat2(cgmath::Matrix2<f32>),
    Mat3(cgmath::Matrix3<f32>),
    Mat4(cgmath::Matrix4<f32>),
}

pub struct Program {
    vertex_source: String,
    fragment_source: String,

    vertex_shader_log: String,
    fragment_shader_log: String,
    program_log: Option<String>,

    program: Option<GLuint>,

    attribute_locations: RefCell<HashMap<String, Option<u32>>>,
    uniform_locations: RefCell<HashMap<String, Option<i32>>>
}

impl Program {
    fn compile_shader(src: &str, shader_type: GLuint) -> (bool, GLuint, String) {
        let src = String::from("#version 300 es
            precision highp float;
            precision highp int;
            precision highp sampler2D;
        ") + src;

        let csrc = std::ffi::CString::new(src.as_bytes()).expect("Invalid string");

        unsafe {
            let shader = gl::CreateShader(shader_type);

            gl::ShaderSource(shader, 1, &(csrc.as_ptr() as *const GLchar), &(csrc.as_bytes().len() as GLint));
            gl::CompileShader(shader);

            let mut result = 0;
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut result);

            let mut log = String::new();
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);

            if len > 1 {
                let mut buf = Vec::with_capacity(len as usize);
                buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
                gl::GetShaderInfoLog(
                    shader,
                    len,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr() as *mut GLchar,
                );
                log = String::from_utf8(buf).ok().unwrap();
            }

            ((result != 0), shader, log)
        }
    }

    fn link_program(vs: GLuint, fs: GLuint) -> (Option<GLuint>, String) {
        unsafe {
            let program = gl::CreateProgram();
            gl::AttachShader(program, vs);
            gl::AttachShader(program, fs);
            gl::LinkProgram(program);

            let mut result = 0;
            gl::GetProgramiv(program, gl::LINK_STATUS, &mut result);

            let mut prog_log = String::new();
            let mut len = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);

            if len > 1 {
                let mut buf = Vec::with_capacity(len as usize);
                buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
                gl::GetProgramInfoLog(
                    program,
                    len,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr() as *mut GLchar,
                );
                prog_log = String::from_utf8(buf).ok().unwrap();
            }

            if result == 0 {
                gl::DeleteProgram(program);
                (None, prog_log)
            } else {
                (Some(program), prog_log)
            }
        }
    }

    pub fn new(vsrc: &str, fsrc: &str) -> Self {
        let vs = Self::compile_shader(vsrc, gl::VERTEX_SHADER);
        let fs = Self::compile_shader(fsrc, gl::FRAGMENT_SHADER);
        let mut prog = None;
        let mut prog_log = None;
        let mut attrs = HashMap::new();
        let mut uniforms = HashMap::new();

        if !vs.0 {
            Self::print_errors("Vertex Shader Log:", &vs.2);
        }
        if !fs.0 {
            Self::print_errors("Fragment Shader Log:", &fs.2);
        }

        if vs.0 && fs.0 {
            let ret = Self::link_program(vs.1, fs.1);
            prog = ret.0;
            prog_log = Some(ret.1);

            if let None = ret.0 {
                Self::print_errors("Program Link Log:", prog_log.as_ref().unwrap());
            }
        }

        unsafe {
            gl::DeleteShader(vs.1);
            gl::DeleteShader(fs.1);
        }

        // load attribute and uniform locations
        if let Some(program) = prog {
            unsafe {
                let mut uniform_count = 0;
                let mut attribute_count = 0;

                gl::GetProgramiv(program, gl::ACTIVE_ATTRIBUTES, &mut attribute_count);
                gl::GetProgramiv(program, gl::ACTIVE_UNIFORMS, &mut uniform_count);

                let buf_len = 255;
                let mut buf = Vec::with_capacity(255 + 1);
                buf.set_len(buf_len);

                let mut name_len = 0;
                let mut size = 0;
                let mut datatype = 0;

                for i in 0..attribute_count {
                    gl::GetActiveAttrib(program, i as GLuint, buf_len as GLsizei, &mut name_len, &mut size, &mut datatype, buf.as_mut_ptr() as *mut GLchar);
                    let name = std::ffi::CStr::from_ptr(buf.as_ptr()).to_str().unwrap().to_owned();
                    attrs.insert(name, Some(i as u32));
                }

                for i in 0..uniform_count {
                    gl::GetActiveUniform(program, i as GLuint, buf_len as GLsizei, &mut name_len, &mut size, &mut datatype, buf.as_mut_ptr() as *mut GLchar);
                    let name = std::ffi::CStr::from_ptr(buf.as_ptr()).to_str().unwrap().to_owned();
                    let location = gl::GetUniformLocation(program, buf.as_ptr());
                    uniforms.insert(name, Some(location));
                }

                gl::UseProgram(program);
            }
        }

        Program {
            vertex_source: String::from(vsrc),
            fragment_source: String::from(fsrc),
            vertex_shader_log: vs.2,
            fragment_shader_log: fs.2,
            program_log: prog_log,
            program: prog,
            attribute_locations: RefCell::new(attrs),
            uniform_locations: RefCell::new(uniforms)
        }
    }

    pub fn vertex_source(&self) -> &String {
        &self.vertex_source
    }

    pub fn fragment_source(&self) -> &String {
        &self.fragment_source
    }

    pub fn valid(&self) -> bool {
        self.program.is_some()
    }

    fn print_errors(header: &str, log: &str) {
        println!("{}", header);
        for (a, b) in Self::errors(log) {
            for ln in b {
                println!("{} {}", a, ln);
            }
        }
    }

    fn errors(log: &str) -> HashMap<i32, Vec<String>> {
        let mut ret = HashMap::new();

        for line in log.lines() {
            // only for reasonably sized lines
            if line.len() < 2 {
                continue;
            }

            let mut lineno = 0;
            let mut message = line;

            // WebGL format
            if line.starts_with("ERROR: ") {
                let line = line.split_at(7).1;

                // extract first colon
                line.find(":").map(|firstcolon| {
                    // extract second colon
                    let line = line.split_at(firstcolon + 1).1;
                    line.find(":").map(|secondcolon| {
                        if let Some(ln) = line.split_at(secondcolon).0.parse().ok() {
                            lineno = ln;
                        }
                        message = line.split_at(secondcolon + 2).1;
                    })
                });
            }

            ret.entry(lineno).or_insert(Vec::new()).push(message.to_string());
        }

        ret
    }

    pub fn vertex_errors(&self) -> HashMap<i32, Vec<String>> {
        Self::errors(&self.vertex_shader_log)
    }

    pub fn fragment_errors(&self) -> HashMap<i32, Vec<String>> {
        Self::errors(&self.fragment_shader_log)
    }

    pub fn link_errors(&self) -> Option<HashMap<i32, Vec<String>>> {
        self.program_log.as_ref().map(|log| Self::errors(log))
    }

    pub fn vertex_log(&self) -> String {
        self.vertex_shader_log.clone()
    }

    pub fn fragment_log(&self) -> String {
        self.fragment_shader_log.clone()
    }

    pub fn link_log(&self) -> Option<String> {
        self.program_log.as_ref().map(|str| str.clone())
    }

    pub fn print_log(&self) {
        println!("Vertex Shader Log:");
        println!("{}", self.vertex_shader_log);
        println!("Fragment Shader Log:");
        println!("{}", self.fragment_shader_log);
        if let Some(proglog) = &self.program_log {
            println!("Program Link Log:");
            println!("{}", proglog);
        }
        println!("Attributes: {:?}", self.attribute_locations);
        println!("Uniforms: {:?}", self.uniform_locations);
    }

    pub fn bind(&self) {
        if let Some(prog) = self.program {
            unsafe { gl::UseProgram(prog); }
        }
    }

    pub fn vertex_attrib_location(&self, attrib: &str) -> Option<u32> {
        // spit warning on first try, then remember that it's not there
        let opt = self.attribute_locations.borrow().get(attrib).map(|r| *r);
        match opt {
            None => {
                println!("No such attribute: {}", attrib);
                self.attribute_locations.borrow_mut().insert(attrib.to_string().clone(), None);
                None
            }
            Some(l) => l
        }
    }

    pub fn vertex_attrib_divisor(&self, attrib: &str, divisor: u32) {
        if let Some(l) = self.vertex_attrib_location(attrib) {
            unsafe { gl::VertexAttribDivisor(l, divisor); }
        }
    }

    pub fn vertex_attrib_buffer(&self, attrib: &str, buffer: &VertexBuffer, size: GLint, datatype: GLenum, normed: bool, stride: GLsizei, offset: GLsizei) {
        if let Some(l) = self.vertex_attrib_location(attrib) {
            buffer.bind();
            unsafe {
                gl::EnableVertexAttribArray(l);
                gl::VertexAttribPointer(l, size, datatype, if normed {1} else {0}, stride, offset as *const GLvoid);
            }
        }
    }

    pub fn disable_vertex_attrib(&self, attrib: &str) {
        if let Some(l) = self.vertex_attrib_location(attrib) {
            unsafe { gl::DisableVertexAttribArray(l); }
        }
    }

    pub fn uniform(&self, uniform: &str, value: Uniform) {
        // spit warning on first try, then remember that it's not there
        let opt = self.uniform_locations.borrow().get(uniform).map(|r| *r);
        let loc = match opt {
            None => {
                println!("No such uniform: {}", uniform);
                self.uniform_locations.borrow_mut().insert(uniform.to_string(), None);
                None
            }
            Some(l) => l
        };

        if let Some(l) = loc {
            unsafe {
                match value {
                    Uniform::Signed(i) => gl::Uniform1i(l, i),
                    Uniform::Unsigned(ui) => gl::Uniform1ui(l, ui),
                    Uniform::Float(f) => gl::Uniform1f(l, f),
                    Uniform::Vec2(v) => gl::Uniform2fv(l, 1, v.as_ptr()),
                    Uniform::Vec3(v) => gl::Uniform3fv(l, 1, v.as_ptr()),
                    Uniform::Vec4(v) => gl::Uniform4fv(l, 1, v.as_ptr()),
                    Uniform::Mat2(m) => gl::UniformMatrix2fv(l, 1, 0, m.as_ptr()),
                    Uniform::Mat3(m) => gl::UniformMatrix3fv(l, 1, 0, m.as_ptr()),
                    Uniform::Mat4(m) => gl::UniformMatrix4fv(l, 1, 0, m.as_ptr()),
                }
            }
        }
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        if let Some(prog) = self.program {
            unsafe { gl::DeleteProgram(prog); }
        }
    }
}

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

    pub fn handle(&self) -> GLuint {
        self.tex
    }

    pub fn bind(&self) {
        unsafe { gl::BindTexture(self.target, self.tex); }
    }

    pub fn bind_at(&self, unit: u32) {
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0 + unit);
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

impl OffscreenTexture {
    fn create(size: (GLsizei, GLsizei), internal: GLenum, format: GLenum, datatype: GLenum) -> Texture {
        let mut texture = Texture::new(gl::TEXTURE_2D);
        texture.bind();
        texture.filter(gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
        texture.filter(gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
        unsafe { texture.teximage(size, internal, format, datatype, std::ptr::null()); }
        texture
    }
    fn new(size: (GLsizei, GLsizei), internal: GLenum, format: GLenum, datatype: GLenum, index: usize) -> Self {
        OffscreenTexture {
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

pub struct OffscreenBuffer {
    size: (GLsizei, GLsizei),
    fbo: GLuint,
    textures: HashMap<String, OffscreenTexture>
}

impl OffscreenBuffer {
    pub fn new(size: (GLsizei, GLsizei)) -> Self {
        let mut fbo = 0;
        unsafe { gl::GenFramebuffers(1, &mut fbo); }

        OffscreenBuffer {
            size,
            fbo,
            textures: HashMap::new()
        }
    }

    pub fn add(&mut self, name: &str, internal: GLenum, format: GLenum, datatype: GLenum) {
        if self.textures.contains_key(name) {
            println!("Texture {} already attached", name);
            return;
        }

        let index = self.textures.len() as _;
        let texture = OffscreenTexture::new(self.size, internal, format, datatype, index);

        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0 + index as u32, gl::TEXTURE_2D, texture.texture.as_ref().unwrap().handle(), 0);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
        self.textures.insert(name.to_string(), texture);
    }

    pub fn draw(&self) {
        let mut draw_buffers: Vec<GLenum> = Vec::new();
        for i in 0..self.textures.len() {
            draw_buffers.push((gl::COLOR_ATTACHMENT0 + i as u32) as _);
        }
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::DrawBuffers(draw_buffers.len() as _, draw_buffers.as_ptr());
            gl::Viewport(0, 0, self.size.0 as _, self.size.1 as _);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
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

impl Drop for OffscreenBuffer {
    fn drop(&mut self) {
        unsafe { gl::DeleteFramebuffers(1, &mut self.fbo); }
    }
}
