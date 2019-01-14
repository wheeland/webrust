use std::ffi::CString;
use std::os::raw::{c_void, c_int};
use std::rc::Rc;
use std::cell::RefCell;
extern crate libc;

#[cfg(target_os = "emscripten")]
use emscripten_sys::*;

#[derive(Clone)]
pub enum StorageData {
    Loading,
    Success(Vec<u8>),
    Error,
}

impl StorageData {
    pub fn is_err(&self) -> bool {
        match self {
            StorageData::Loading => false,
            StorageData::Success(data) => false,
            StorageData::Error => true,
        }
    }

    pub fn get(&self) -> Option<Vec<u8>> {
        match self {
            StorageData::Loading => None,
            StorageData::Success(data) => Some(data.clone()),
            StorageData::Error => None,
        }
    }
}

pub struct StorageLoad {
    pub data: Rc<RefCell<StorageData>>,
}

extern "C" fn storage_load_callback(user_data: *mut c_void, buf: *mut c_void, len: c_int) {
    unsafe {
        // build Vec from data
        let mut data = Vec::new();
        let mut slice = std::slice::from_raw_parts(buf as *const u8, len as usize);
        for i in slice {
            data.push(*i);
        }

        // save
        let mut load = &mut *(user_data as *mut RefCell<StorageData>);
        *load.borrow_mut() = StorageData::Success(data);
        Rc::from_raw(load);

        libc::free(buf);
    }
}

extern "C" fn storage_error(user_data: *mut c_void) {
    unsafe {
        // save
        let mut load = &mut *(user_data as *mut RefCell<StorageData>);
        *load.borrow_mut() = StorageData::Error;
        Rc::from_raw(load);
    }
}

pub fn load(db: &str, name: &str) -> StorageLoad {
    let data = Rc::new(RefCell::new(StorageData::Loading));

    #[cfg(target_os = "emscripten")]
    {
        let db = CString::new(db).unwrap();
        let name = CString::new(name).unwrap();

        let ptr = Rc::into_raw(data.clone());

        unsafe {
            emscripten_idb_async_load(db.as_ptr(), name.as_ptr(), ptr as _, Some(storage_load_callback), Some(storage_error));
        }
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
    #[cfg(target_os = "emscripten")]
    {
        let db = CString::new(db).unwrap();
        let name = CString::new(name).unwrap();
        let mut error = 0;
        unsafe {
            emscripten_idb_async_store(db.as_ptr(), name.as_ptr(), data.as_ptr() as _, data.len() as i32, std::ptr::null_mut(), None, None)
        }
    }
}
