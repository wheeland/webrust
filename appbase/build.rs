extern crate cc;

fn main() {
    cc::Build::new()
        .cpp(true)
        .file("src/fileload.cpp")
        .compile("libexternalc.a");
}

