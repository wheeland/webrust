extern crate cc;

fn main() {
    cc::Build::new()
        .cpp(true)
        .file("src/fileload.cpp")
        .file("src/imgdecode.cpp")
        .flag("-std=c++11")
        .compile("libexternalc.a");
}

