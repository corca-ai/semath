#!/usr/bin/env bash
set -euo pipefail

build_host=${SEMATH_BUILD_HOST:-${1:-}}
if [[ -z "$build_host" ]]; then
  echo "usage: SEMATH_BUILD_HOST=<x86_64-linux-host> scripts/build-wasm-remote.sh" >&2
  exit 2
fi

remote_dir=$(ssh -- "$build_host" mktemp -d /tmp/semath-build.XXXXXX)
if [[ "$remote_dir" != /tmp/semath-build.* ]]; then
  echo "remote host returned an unexpected build directory" >&2
  exit 1
fi

cleanup() {
  ssh -- "$build_host" rm -rf -- "$remote_dir"
}
trap cleanup EXIT

rsync --archive --delete \
  --exclude .git \
  --exclude .artifacts \
  --exclude node_modules \
  --exclude target \
  ./ "$build_host:$remote_dir/"
ssh -- "$build_host" "cd '$remote_dir' && bash -lc scripts/build-wasm.sh"
rsync --archive --ignore-times --delete "$build_host:$remote_dir/lib/wasm/" lib/wasm/
