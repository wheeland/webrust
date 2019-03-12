use std::ffi::CString;
use std::os::raw::{c_void, c_int};
use std::rc::Rc;
use std::cell::RefCell;
extern crate libc;

use emscripten_sys::*;

#[derive(Clone)]
pub enum StorageData {
    Loading,
    Success(Vec<u8>),
    Error,
    Done,
}

impl StorageData {
    pub fn is_err(&self) -> bool {
        match self {
            StorageData::Loading => false,
            StorageData::Success(..) => false,
            StorageData::Error => true,
            StorageData::Done => false,
        }
    }

    pub fn get(&self) -> Option<Vec<u8>> {
        match self {
            StorageData::Loading => None,
            StorageData::Success(data) => Some(data.clone()),
            StorageData::Error => None,
            StorageData::Done => None,
        }
    }

    fn consume<T, FS: FnOnce(Vec<u8>) -> T, FE: FnOnce() -> T>(&mut self, success: FS, error: FE) -> Option<T> {
        let ret = match self {
            StorageData::Loading | StorageData::Done => None,
            StorageData::Success(data) => {
                Some(success(data.clone()))
            }
            StorageData::Error => {
                Some(error())
            }
        };

        if ret.is_some() {
            *self = StorageData::Done;
        }
        ret
    }
}

pub struct StorageLoad {
    pub data: Rc<RefCell<StorageData>>,
}

impl StorageLoad {
    pub fn consume<T, FS: FnOnce(Vec<u8>) -> T, FE: FnOnce() -> T>(&mut self, success: FS, error: FE) -> Option<T> {
        self.data.borrow_mut().consume(success, error)
    }
}

extern "C" fn storage_load_callback(user_data: *mut c_void, buf: *mut c_void, len: c_int) {
    unsafe {
        // build Vec from data
        let mut data = Vec::new();
        let slice = std::slice::from_raw_parts(buf as *const u8, len as usize);
        for i in slice {
            data.push(*i);
        }

        // save
        let load = &mut *(user_data as *mut RefCell<StorageData>);
        *load.borrow_mut() = StorageData::Success(data);
        Rc::from_raw(load);

        libc::free(buf);
    }
}

extern "C" fn storage_error(user_data: *mut c_void) {
    unsafe {
        // save
        let load = &mut *(user_data as *mut RefCell<StorageData>);
        *load.borrow_mut() = StorageData::Error;
        Rc::from_raw(load);
    }
}

pub fn load(db: &str, name: &str) -> StorageLoad {
    let data = Rc::new(RefCell::new(StorageData::Loading));
    let db = CString::new(db).unwrap();
    let name = CString::new(name).unwrap();
    let ptr = Rc::into_raw(data.clone());

    unsafe {
        emscripten_idb_async_load(db.as_ptr(), name.as_ptr(), ptr as _, Some(storage_load_callback), Some(storage_error));
    }

    StorageLoad { data }
}

impl StorageLoad {
    pub fn get(&self) -> Option<Vec<u8>> {
        self.data.borrow().get()
    }

    pub fn is_err(&self) -> bool {
        self.data.borrow().is_err()
    }
}

pub fn store(db: &str, name: &str, data: &[u8]) {
    let db = CString::new(db).unwrap();
    let name = CString::new(name).unwrap();
    unsafe {
        emscripten_idb_async_store(db.as_ptr(), name.as_ptr(), data.as_ptr() as _, data.len() as i32, std::ptr::null_mut(), None, None)
    }
}
