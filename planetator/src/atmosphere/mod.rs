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
    static mut AtmosphereRaleighScattering: f32;
    static mut AtmosphereRaleighHeight: f32;
    static mut AtmosphereMieScattering: f32;
    static mut AtmosphereMieHeight: f32;
}

static mut did_init: bool = false;
static mut dirty: bool = false;

pub fn is_dirty() -> bool {
    unsafe { dirty }
}

pub fn half_precision() -> bool {
    unsafe { AtmosphereUseHalfPrecision != 0 }
}
pub fn set_half_precision(half: bool) {
    unsafe {
        if half_precision() != half {
            AtmosphereUseHalfPrecision = if half { 1 } else { 0 };
            dirty = true;
        }
    }
}

pub fn outer_radius() -> f32 {
    unsafe { AtmosphereOuterRadius }
}
pub fn set_outer_radius(r: f32) {
    unsafe {
        if AtmosphereOuterRadius != r {
            AtmosphereOuterRadius = r;
            dirty = true;
        }
    }
}

pub fn raleigh_scattering() -> f32 {
    unsafe { AtmosphereRaleighScattering }
}
pub fn set_raleigh_scattering(r: f32) {
    unsafe {
        if AtmosphereRaleighScattering != r {
            AtmosphereRaleighScattering = r;
            dirty = true;
        }
    }
}

pub fn raleigh_height() -> f32 {
    unsafe { AtmosphereRaleighHeight }
}
pub fn set_raleigh_height(r: f32) {
    unsafe {
        if AtmosphereRaleighHeight != r {
            AtmosphereRaleighHeight = r;
            dirty = true;
        }
    }
}

pub fn mie_scattering() -> f32 {
    unsafe { AtmosphereMieScattering }
}
pub fn set_mie_scattering(r: f32) {
    unsafe {
        if AtmosphereMieScattering != r {
            AtmosphereMieScattering = r;
            dirty = true;
        }
    }
}

pub fn mie_height() -> f32 {
    unsafe { AtmosphereMieHeight }
}
pub fn set_mie_height(r: f32) {
    unsafe {
        if AtmosphereMieHeight != r {
            AtmosphereMieHeight = r;
            dirty = true;
        }
    }
}

pub fn recreate() {
    unsafe {
        if did_init {
            AtmosphereInitModel();
            dirty = false;
        }
    }
}

pub fn init() {
    unsafe {
        if !did_init {
            AtmosphereInit();
            dirty = false;
            did_init = true;
        }
    }
}

pub fn destroy() {
    unsafe {
        if did_init {
            AtmosphereDestroy();
            dirty = true;
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