use std::ffi::{CStr, CString};
use std::io::Read;

#[cfg(target_os = "emscripten")]
use emscripten_sys::*;

#[cfg(not(target_os = "emscripten"))]
use curl::easy::Easy;

const FETCH_LOAD_TO_MEMORY: u32 = 1;
const FETCH_STREAM_DATA: u32 = 2;
const FETCH_PERSIST_FILE: u32 = 4;
const FETCH_APPEND: u32 = 8;
const FETCH_REPLACE: u32 = 16;
const FETCH_NO_DOWNLOAD: u32 = 32;
const FETCH_SYNCHRONOUS: u32 = 64;
const FETCH_WAITABLE: u32 = 128;

const STATUS_UNSENT: u16 = 0;
const STATUS_OPENED: u16 = 1;
const STATUS_HEADERS_RECEIVED: u16 = 2;
const STATUS_LOADING: u16 = 3;
const STATUS_DONE: u16 = 4;

pub enum State {
    Waiting,
    Done,
    Error,
}

pub struct Fetch {
    #[cfg(target_os = "emscripten")]
    request_data: CString,

    #[cfg(target_os = "emscripten")]
    fetch: *mut emscripten_fetch_t,

    #[cfg(not(target_os = "emscripten"))]
    data: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Request {
    Get,
    Post,
}

impl Fetch {
    fn new(request: Request, path: &str, attributes: u32, request_data: &str) -> Self {
        #[cfg(target_os = "emscripten")]
        {
            let mut attr = emscripten_fetch_attr_t {
                requestMethod: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                userData: std::ptr::null_mut(),
                onsuccess: None,
                onerror: None,
                onprogress: None,
                attributes: 0,
                timeoutMSecs: 0,
                withCredentials: 0,
                destinationPath: std::ptr::null(),
                userName: std::ptr::null(),
                password: std::ptr::null(),
                requestHeaders: std::ptr::null(),
                overriddenMimeType: std::ptr::null(),
                requestData: std::ptr::null(),
                requestDataSize: 0,
            };

            unsafe { emscripten_fetch_attr_init(&mut attr) }

            // Set Request POST/GET/...
            let request = match request {
                Request::Get => vec!('G', 'E', 'T'),
                Request::Post  => vec!('P', 'O', 'S', 'T'),
            };
            for i in 0..request.len().min(28) {
                attr.requestMethod[i] = request[i] as i8;
            }

            let request_data = CString::new(request_data).unwrap();
            attr.requestData = request_data.as_ptr();
            attr.requestDataSize = request_data.as_bytes().len();

            attr.attributes = attributes;

            let path = CString::new(path).unwrap();
            let fetch = unsafe { emscripten_fetch(&mut attr, path.as_ptr()) };

            Fetch {
                request_data,
                fetch
            }
        }

        #[cfg(not(target_os = "emscripten"))]
        {
            let mut handle = Easy::new();
            let mut data = Vec::new();

            let request_data = CString::new(request_data).unwrap();

            handle.url(&format!("127.0.0.1/{}", path)).unwrap();
            handle.port(8080).unwrap();
            if let Request::Post = request {
                handle.post(true).unwrap()
            }
            let success = {
                let mut transfer = handle.transfer();

                transfer.write_function(|new_data| {
                    data.extend_from_slice(new_data);
                    Ok(new_data.len())
                }).unwrap();

                if let Request::Post = request {
                    transfer.read_function(|into| {
                        Ok(request_data.to_bytes().read(into).unwrap())
                    }).unwrap();
                }

                let ret = transfer.perform();
                match ret {
                    Ok(_) => true,
                    Err(e) => {
                        println!("HTTPClient: {:?}", e);
                        false
                    }
                }
            };

            Fetch {
                data: if success { Some(data) } else { None }
            }
        }
    }

    pub fn get(path: &str) -> Self {
        Self::new(Request::Get, path, FETCH_LOAD_TO_MEMORY, "")
    }

    pub fn post(path: &str, request_data: &str) -> Self {
        Self::new(Request::Post, path, FETCH_LOAD_TO_MEMORY, request_data)
    }

    pub fn state(&self) -> State {
        #[cfg(target_os = "emscripten")]
        {
            let fetch = unsafe { &*self.fetch };
            match fetch.readyState {
                STATUS_UNSENT => State::Waiting,
                STATUS_OPENED => State::Waiting,
                STATUS_HEADERS_RECEIVED => State::Waiting,
                STATUS_LOADING => State::Waiting,
                STATUS_DONE => State::Done,
                _ => State::Error,
            }
        }

        #[cfg(not(target_os = "emscripten"))]
        {
            if self.data().is_some() {
                State::Done
            } else {
                State::Error
            }
        }
    }

    pub fn data(&self) -> Option<Vec<u8>> {
        #[cfg(target_os = "emscripten")]
        {
            let fetch = unsafe { &*self.fetch };
            unsafe { Some(std::slice::from_raw_parts(fetch.data as *const u8, fetch.numBytes as usize).to_vec()) }
        }

        #[cfg(not(target_os = "emscripten"))]
        {
            self.data.clone()
        }
    }
}

impl Drop for Fetch {
    fn drop(&mut self) {
        #[cfg(target_os = "emscripten")]
        unsafe { emscripten_fetch_close(self.fetch); }
    }
}
