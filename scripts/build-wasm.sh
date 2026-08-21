#!/usr/bin/env sh
set -eu

architecture=$(uname -m)
system=$(uname -s)
if [ "$system" != "Linux" ] || [ "$architecture" != "x86_64" ]; then
  echo "release WASM must be built on an x86_64 Linux build host" >&2
  exit 1
fi

build_target_dir=$PWD/target
cargo_home=${CARGO_HOME:-$HOME/.cargo}
RUSTFLAGS="--remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$PWD=/workspace" \
  CARGO_TARGET_DIR=$build_target_dir \
  cargo build --locked --release --target wasm32-unknown-unknown -p semath-wasm
mkdir -p lib/wasm
wasm-bindgen \
  --target web \
  --typescript \
  --remove-name-section \
  --remove-producers-section \
  --out-dir lib/wasm \
  "$build_target_dir/wasm32-unknown-unknown/release/semath_wasm.wasm"

(
  cd lib/wasm
  sha256sum \
    semath_wasm.js \
    semath_wasm.d.ts \
    semath_wasm_bg.wasm \
    semath_wasm_bg.wasm.d.ts > SHA256SUMS
)
