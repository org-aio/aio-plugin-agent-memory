#!/bin/sh
set -eu
node scripts/check-contract.mjs
OUTPUT=backend/src/site/addzero/aio/memory/bindings
"${WIT_BINDGEN:-wit-bindgen}" kotlin \
  --kotlin-package-name site.addzero.aio.memory.bindings \
  --kotlin-imports site.addzero.aio.memory.transport.PluginRootFunctionsExportsImpl \
  --declaration-visibility internal \
  --cabi-realloc-freeing-strategy free-all \
  --out-dir "$OUTPUT" \
  backend/contract/plugin.wit
find "$OUTPUT" -name '*.kt' -exec perl -pi -e 's/[ \t]+$//g' {} +
find "$OUTPUT" -name '*.kt' -exec perl -0pi -e 's/\n+\z/\n/' {} +
