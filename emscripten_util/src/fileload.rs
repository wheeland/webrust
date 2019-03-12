extern crate base64;

use std::ffi::{CString};
use std::os::raw::{c_char, c_int};

extern "C" {
    fn UploadStart(element: *const c_char);
    fn UploadResultSize(element: *const c_char) -> c_int;
    fn UploadFilenameSize(element: *const c_char) -> c_int;
    fn UploadGetData(element: *const c_char, data: *mut u8, len: c_int) -> c_int;
    fn UploadGetFilename(element: *const c_char, data: *mut u8, len: c_int) -> c_int;
}

//pub fn show_upload_button(element: &str) {
//    let command = format!("document.getElementById('{}').type = 'file';", element);
//    super::webrunner::run_javascript(&command);
//}

pub fn start_upload(element: &str) {
    // prepare C string buffers
    let element = CString::new(element).unwrap();

    // call
    unsafe { UploadStart(element.as_ptr()) };
}

pub fn get_result(element: &str) -> Option<(String, Vec<u8>)> {
    // prepare C string buffers
    let element = CString::new(element).unwrap();

    // check if there is any data
    let result_sz = unsafe { UploadResultSize(element.as_ptr()) };
    if result_sz < 0 {
        return None;
    }

    // get filename size
    let filename_sz = unsafe { UploadFilenameSize(element.as_ptr()) };
    if filename_sz < 0 {
        return None;
    }

    // get data
    let mut filename = Vec::new();
    filename.resize(filename_sz as usize, 0);
    unsafe { UploadGetFilename(element.as_ptr(), filename.as_mut_ptr(), filename_sz); }
    let filename = unsafe { String::from_utf8_unchecked(filename) };

    let mut data = Vec::new();
    data.resize(result_sz as usize, 0);
    unsafe { UploadGetData(element.as_ptr(), data.as_mut_ptr(), result_sz); }

    Some((filename, data))
}

pub fn download(name: &str, data: &str) {
    let encoded = base64::encode(data);

    let command = format!("
        var element = document.createElement('a');
        element.setAttribute('href', 'data:application/octet-stream;charset=utf-8;base64,{}');
        element.setAttribute('download', '{}');
        element.style.display = 'none';
        document.body.appendChild(element);
        element.click();
        document.body.removeChild(element);
    ", encoded, name);

    super::run_javascript(&command);
}