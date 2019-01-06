extern crate cc;

fn main() {
    cc::Build::new()
        .cpp(true)
        .file("src/upload.cpp")
        .compile("libexternalc.a");
}

