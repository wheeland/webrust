extern crate cc;

fn main() {
    cc::Build::new()
        .cpp(true)
        .file("src/atmosphere/model.cc")
        .file("src/atmosphere/interface.cc")
        .flag("-std=c++11")
        .include("src/")
        .include("/usr/include/SDL2")
        .define("_REENTRANT", None)
        .compile("libatmosphere.a");

    #[cfg(not(target_os = "emscripten"))] {
        println!("cargo:rustc-link-lib=GLEW");
        println!("cargo:rustc-link-lib=GL");
    }
}
