#!/usr/bin/env bash
set -e

echo "==> Adding wasm32-wasip1 target for plugins..."
rustup target add wasm32-wasip1

echo "==> Building Rust core (native)..."
cargo build --release -p messenger_core

echo "==> Building storage_memory plugin (wasm)..."
cargo build --release -p storage_memory --target wasm32-wasip1

echo "==> Building crypto_aes plugin (wasm)..."
cargo build --release -p crypto_aes --target wasm32-wasip1

echo "==> Copying .wasm files to Flutter assets..."
mkdir -p flutter_app/assets/plugins

cp target/wasm32-wasip1/release/storage_memory.wasm flutter_app/assets/plugins/
cp target/wasm32-wasip1/release/crypto_aes.wasm flutter_app/assets/plugins/

cp plugins/storage_memory/manifest.toml flutter_app/assets/plugins/storage_memory.manifest.toml
cp plugins/crypto_aes/manifest.toml flutter_app/assets/plugins/crypto_aes.manifest.toml

echo "==> Copying native core lib to Flutter..."
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    LIB_SRC="target/release/libmessenger_core.so"
    LIB_DST="flutter_app/linux/libmessenger_core.so"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    LIB_SRC="target/release/libmessenger_core.dylib"
    LIB_DST="flutter_app/macos/libmessenger_core.dylib"
fi

mkdir -p "$(dirname $LIB_DST)"
cp "$LIB_SRC" "$LIB_DST"

echo "==> Done. Run: cd flutter_app && flutter run"
