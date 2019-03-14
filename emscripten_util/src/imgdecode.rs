extern crate base64;

/*

PLAN:

- give encoded data to C++, receive a unique ID int
- tell javascript to call C++ getter with provided length and ID:
    var data = new Uint8Array(length);
    cpp_get_data(uniqueId, data);
- now that C++ copied the data (hopefully), create an img / canvas
    - if can b sync: directly push result to C++, then get it from C++ in rust
    - if can not be sync: give result to C++ when done, rust asks C++ regularly


*/
use std::os::raw::{c_uchar, c_int};

extern "C" {
    fn DecodeStart(element: *const c_uchar, size: c_int) -> c_int;
    fn DecodeGetResultSize(id: c_int) -> c_int;
    fn DecodeGetResult(id: c_int, buffer: *mut c_uchar, size: c_int, width: *mut c_int, height: *mut c_int) -> c_int;
}

pub fn start(data: Vec<u8>) -> i32 {
    let encoded = base64::encode(&data);
    unsafe { DecodeStart(encoded.as_ptr(), encoded.len() as _) }
}

pub fn get(id: i32) -> Option<((i32, i32), Vec<u8>)> {
    let retsz = unsafe { DecodeGetResultSize(id) };
    if retsz < 0 {
        return None;
    }

    let mut retbuf = Vec::new();
    let mut width = 0;
    let mut height = 0;
    retbuf.resize(retsz as _, 0);
    unsafe { DecodeGetResult(id, retbuf.as_mut_ptr(), retsz, &mut width, &mut height); }

    if width > 0 && height > 0 && retsz > 0 {
        Some(((width as _, height as _), retbuf))
    } else {
        None
    }
}