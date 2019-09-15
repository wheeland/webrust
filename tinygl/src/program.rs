use std::cell::RefCell;
use std::collections::HashMap;
use gl::types::*;
use cgmath::prelude::*;
use super::buffer::VertexBuffer;

static mut PRINT_SHADER_ERRORS: bool = true;

static mut BOUND_PROGRAM: i32 = 0;
static mut PROGRAM_ID: i32 = 0;
static mut CLEARED_VERTEX_BINDINGS: bool = true;


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
    id: i32,
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
    pub fn set_print_compilation_errors(print: bool) {
        unsafe { PRINT_SHADER_ERRORS = print; }
    }

    pub fn print_compilation_errors() -> bool {
        unsafe { PRINT_SHADER_ERRORS }
    }

    fn compile_shader(src: &str, shader_type: GLuint, version: i32) -> (bool, GLuint, String) {
        let glsl = match version {
            100 => "100",
            300 => "300 es",
            _ => "100"
        };
        let src = format!("#version {}
            precision highp float;
            precision highp int;
            precision highp sampler2D;
        ", glsl) + src;

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
        Self::new_versioned(vsrc, fsrc, 300)
    }

    fn print_lines(src: &str) {
        for ln in src.split("\n").enumerate() {
            println!("{:4} {}", ln.0 + 5, ln.1);
        }
    }

    pub fn new_versioned(vsrc: &str, fsrc: &str, version: i32) -> Self {
        let id = unsafe { let id = PROGRAM_ID; PROGRAM_ID += 1; id };
        let vs = Self::compile_shader(vsrc, gl::VERTEX_SHADER, version);
        let fs = Self::compile_shader(fsrc, gl::FRAGMENT_SHADER, version);
        let mut prog = None;
        let mut prog_log = None;
        let mut attrs = HashMap::new();
        let mut uniforms = HashMap::new();

        if Self::print_compilation_errors() && !vs.2.is_empty() {
            Self::print_lines(vsrc);
            Self::print_errors("Vertex Shader Log:", &vs.2);
        }
        if Self::print_compilation_errors() && !fs.2.is_empty() {
            Self::print_lines(fsrc);
            Self::print_errors("Fragment Shader Log:", &fs.2);
        }

        if vs.0 && fs.0 {
            let ret = Self::link_program(vs.1, fs.1);
            prog = ret.0;
            prog_log = Some(ret.1);

            if Self::print_compilation_errors() && !prog_log.as_ref().unwrap().is_empty() {
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
                    let loc = gl::GetAttribLocation(program, buf.as_ptr() as _);
                    attrs.insert(name, Some(loc as u32));
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
            id,
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

    fn assert_bound(&self) {
        assert!(unsafe { BOUND_PROGRAM } == self.id);
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
            unsafe { BOUND_PROGRAM = self.id; }
            assert!(unsafe { CLEARED_VERTEX_BINDINGS });
        }
    }

    pub fn handle(&self) -> Option<GLuint> {
        self.program
    }

    pub fn vertex_attrib_location(&self, attrib: &str) -> Option<u32> {
        self.assert_bound();

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
        self.assert_bound();

        if let Some(l) = self.vertex_attrib_location(attrib) {
            unsafe { gl::VertexAttribDivisor(l, divisor); }
        }
    }

    pub fn vertex_attrib_buffer(&self, attrib: &str, buffer: &VertexBuffer, size: GLint, datatype: GLenum, normed: bool, stride: GLsizei, offset: GLsizei) {
        self.assert_bound();
        unsafe { CLEARED_VERTEX_BINDINGS = false; }

        if let Some(l) = self.vertex_attrib_location(attrib) {
            buffer.bind();
            unsafe {
                // TODO: keep track of those to reduce GL calls:
                gl::EnableVertexAttribArray(l);
                gl::VertexAttribPointer(l, size, datatype, if normed {1} else {0}, stride, offset as *const GLvoid);
            }
        }
    }

    pub fn disable_vertex_attrib(&self, attrib: &str) {
        self.assert_bound();

        if let Some(l) = self.vertex_attrib_location(attrib) {
            unsafe { gl::DisableVertexAttribArray(l); }
        }
    }

    pub fn disable_all_vertex_attribs(&self) {
        self.assert_bound();

        for loc in self.attribute_locations.borrow().iter() {
            if let Some(loc) = loc.1 {
                unsafe { gl::DisableVertexAttribArray(*loc); }
            }
        }

        unsafe { CLEARED_VERTEX_BINDINGS = true; }
    }

    pub fn uniform(&self, uniform: &str, value: Uniform) {
        self.assert_bound();

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

