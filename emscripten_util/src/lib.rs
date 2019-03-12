extern crate emscripten_sys;

pub mod localstorage;
pub mod fileload;
pub mod imgdecode;

pub fn run_javascript(code: &str) {
    let code = code.replace("\n", " ");
    let code = code.replace("\"", "\\\"");
    let code = String::from("try { eval(\"") + &code + "\"); } catch (error) { console.log('Error: ' + error); }";
    let code = std::ffi::CString::new(code).unwrap();

    unsafe { emscripten_sys::emscripten_run_script_string(code.as_ptr()); }
}

pub fn set_overlay_position(name: &str, pos: (f32, f32), size: (f32, f32)) {
    run_javascript(&format!("
        var state = document.getElementById('{}');
        state.style.left = '{}px';
        state.style.top = '{}px';
        state.style.width = '{}px';
        state.style.height = '{}px';
    ", name, pos.0 as i32, pos.1 as i32, size.0 as i32, size.1 as i32));
}
