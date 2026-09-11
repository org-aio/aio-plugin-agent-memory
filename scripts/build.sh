#!/bin/sh
set -eu
node scripts/check-contract.mjs
node scripts/prepare-graph.mjs
KOTLIN_CLI_NO_WELCOME_BANNER=1 ./kotlin build -m frontend -p wasmJs -v release
KOTLIN_CLI_NO_WELCOME_BANNER=1 ./kotlin build -m backend -p wasmWasi -v release
mkdir -p dist
rm -rf dist/frontend
cp -R build/tasks/_frontend_buildWasmJsAppWasmJsRelease dist/frontend
ADAPTER=${WASI_ADAPTER:-build/wasi_snapshot_preview1.reactor.wasm}
if [ ! -f "$ADAPTER" ]; then
  curl --fail --location --retry 3 https://github.com/bytecodealliance/wasmtime/releases/download/v43.0.2/wasi_snapshot_preview1.reactor.wasm -o "$ADAPTER"
fi
test "$(shasum -a 256 "$ADAPTER" | cut -d ' ' -f 1)" = eb4a4fccf1a2446e2c7606d2d90b7a9ca24d15d9161e363e4b34ce87bc18d173
wasm-tools component embed backend/contract/plugin.wit --world plugin \
  build/artifacts/CompiledWebArtifact/backendwasmWasirelease/kotlin-output/backend.wasm -o build/embedded.wasm
wasm-tools component new build/embedded.wasm --adapt "wasi_snapshot_preview1=$ADAPTER" -o dist/plugin.wasm
wasm-tools validate --features component-model dist/plugin.wasm
