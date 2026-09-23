#!/bin/sh
set -eu
cargo fetch --locked
dx build --package az-memory-frontend --platform web --release --locked --offline
node scripts/package-frontend.mjs
TARGET=${AIO_RUST_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
if [ "${1:-}" = "--process" ]; then
  TARGET=x86_64-unknown-linux-gnu
  cargo zigbuild --locked --release --target "$TARGET.2.17" -p az-memory-server
else
  cargo build --locked --release --target "$TARGET" -p az-memory-server
fi
mkdir -p dist
cp "target/$TARGET/release/az-memory-server" dist/memory-server
