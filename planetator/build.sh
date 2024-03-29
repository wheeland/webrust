#!/bin/bash

# source emscripten env vars
OLD=`pwd`
cd ~/software/web/emsdk
. ~/software/web/emsdk/emsdk_env.sh
cd "$OLD"

# build SDL2 for emscripten
embuilder.py build sdl2

export EMMAKEN_CFLAGS="\
    -s USE_SDL=2 \
    -s USE_WEBGL2=1 \
    -s ASSERTIONS=1 \
    -s DEMANGLE_SUPPORT=1 \
    -Os \
    -s TOTAL_MEMORY=1073741824 \
    -s EXPORTED_FUNCTIONS='[\"_UploadData\", \"_UploadFinished\", \"_DecodeSetImageData\", \"_DecodeSetImageDone\", \"_main\", \"_malloc\"]' \
    -s EXTRA_EXPORTED_RUNTIME_METHODS='[\"ccall\", \"cwrap\"]' \
    --bind \
"

export RUST_BACKTRACE=1

# set to debuginfo=1 to profile Rust code inside the browsers!
export RUSTFLAGS='-C panic=abort -C lto -C opt-level=z -C overflow-checks=no -C debuginfo=0 -C debug-assertions=no'

cargo build --target wasm32-unknown-emscripten --release

php -S 0.0.0.0:8080
