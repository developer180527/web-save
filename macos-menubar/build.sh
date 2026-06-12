#!/usr/bin/env bash
# Build the native SwiftUI menubar app against websave-core via UniFFI.
# Output: macos-menubar/dist/WebSave Menubar.app (ad-hoc signed, local use).
set -euo pipefail
cd "$(dirname "$0")"
ROOT=..

echo "==> Building Rust FFI library"
cargo build --release -p websave-ffi --manifest-path "$ROOT/Cargo.toml"

echo "==> Generating Swift bindings"
mkdir -p Generated
cargo run --release -p websave-ffi --bin uniffi-bindgen \
    --manifest-path "$ROOT/Cargo.toml" -- \
    generate --library "$ROOT/target/release/libwebsave_ffi.dylib" \
    --language swift --out-dir Generated

APP="dist/WebSave Menubar.app"
echo "==> Compiling Swift app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp Info.plist "$APP/Contents/Info.plist"
cp "$ROOT/src-tauri/icons/icon.icns" "$APP/Contents/Resources/AppIcon.icns"

swiftc -O \
    Sources/*.swift Generated/websave_ffi.swift \
    -import-objc-header Generated/websave_ffiFFI.h \
    -L "$ROOT/target/release" -lwebsave_ffi \
    -framework AppKit \
    -target "$(uname -m)-apple-macos13.0" \
    -o "$APP/Contents/MacOS/WebSave Menubar"

codesign --force -s - "$APP"
echo "==> Built $APP"
