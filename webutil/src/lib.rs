extern crate crypto;
extern crate rand;

#[cfg(target_os = "emscripten")]
extern crate emscripten_sys;

#[cfg(not(target_os = "emscripten"))]
extern crate curl;

pub mod httpclient;
pub mod curve25519;