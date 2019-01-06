#!/bin/sh

#embuilder.py build sdl2

export EMMAKEN_CFLAGS="\
    -s USE_SDL=2 \
    -s USE_WEBGL2=1 \
    -Os \
    -s TOTAL_MEMORY=1073741824 \
    -s EXPORTED_FUNCTIONS='[\"_UploadFinished\", \"_main\", \"_malloc\"]' \
    -s EXTRA_EXPORTED_RUNTIME_METHODS='[\"ccall\", \"cwrap\"]' \
"

export RUST_BACKTRACE=0
#export EMMAKEN_CFLAGS="-s USE_SDL=2 -s DEMANGLE_SUPPORT=1 -s ASSERTIONS=2"
export RUSTFLAGS='-C panic=abort -C lto -C opt-level=z -C panic=abort -C overflow-checks=no -C debuginfo=0 -C debug-assertions=no'

cargo build --target asmjs-unknown-emscripten --release

php -S 0.0.0.0:8080
