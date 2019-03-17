extern crate gl;

use gl::types::*;
use std::os::raw::{c_char, c_int};
use std::ffi::CString;

extern "C" {
    fn AtmosphereInit();
    fn AtmosphereInitModel();
    fn AtmosphereDestroy();
    fn AtmosphereGetShaderSource(buffer: *mut c_char, size: c_int) -> c_int;
    fn AtmospherePrepareShader(program: GLuint, first_tex_unit: c_int);

    static mut AtmosphereUseConstantSolarSpectrum: c_int;
    static mut AtmosphereUseOzone: c_int;
    static mut AtmosphereUseCombinedTextures: c_int;
    static mut AtmosphereUseHalfPrecision: c_int;

    static mut AtmosphereExposure: f32;
    static mut AtmosphereOuterRadius: f32;
}

static mut did_init: bool = false;

pub fn init() {
    unsafe {
        if !did_init {
            AtmosphereInit();
            did_init = true;
        }
    }
}

pub fn destroy() {
    unsafe {
        if did_init {
            AtmosphereDestroy();
            did_init = false;
        }
    }
}

pub fn shader_source() -> String {
    init();

    let size = unsafe { AtmosphereGetShaderSource(std::ptr::null_mut(), 0) };
    let mut buf = Vec::<u8>::new();
    buf.resize(size as _, 0);
    unsafe { AtmosphereGetShaderSource(buf.as_mut_ptr() as _, size); }

    CString::new(buf)
        .ok()
        .unwrap()
        .into_string()
        .ok()
        .unwrap()
}

pub fn prepare_shader(program: GLuint, first_tex_unit: usize) {
    unsafe { AtmospherePrepareShader(program, first_tex_unit as _); }
}