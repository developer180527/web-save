#!/usr/bin/env bash
# Build the native AppKit menubar app against websave-core via UniFFI.
# Output: macos-menubar/dist/WebSave Menubar.app (ad-hoc signed, local use).
#
# The app links the FFI dylib dynamically, so the dylib is COPIED INTO the
# bundle (Contents/Frameworks) and referenced via @rpath — otherwise the
# binary would point at target/release/ and crash the moment that build
# artifact is cleaned.
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
FRAMEWORKS="$APP/Contents/Frameworks"
echo "==> Assembling bundle"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$FRAMEWORKS"
cp Info.plist "$APP/Contents/Info.plist"
cp "$ROOT/src-tauri/icons/icon.icns" "$APP/Contents/Resources/AppIcon.icns"

echo "==> Embedding FFI dylib"
cp "$ROOT/target/release/libwebsave_ffi.dylib" "$FRAMEWORKS/"
# The bundled dylib identifies itself via @rpath; the app records that same
# reference and finds it under Contents/Frameworks at runtime.
install_name_tool -id @rpath/libwebsave_ffi.dylib "$FRAMEWORKS/libwebsave_ffi.dylib"

echo "==> Compiling Swift app"
# Link against the bundled dylib (the only libwebsave_ffi in this -L path),
# and add an rpath so the executable resolves @rpath to Contents/Frameworks.
swiftc -O \
    Sources/*.swift Generated/websave_ffi.swift \
    -import-objc-header Generated/websave_ffiFFI.h \
    -L "$FRAMEWORKS" -lwebsave_ffi \
    -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
    -framework AppKit \
    -target "$(uname -m)-apple-macos13.0" \
    -o "$APP/Contents/MacOS/WebSave Menubar"

echo "==> Signing"
codesign --force -s - "$FRAMEWORKS/libwebsave_ffi.dylib"
codesign --force -s - "$APP/Contents/MacOS/WebSave Menubar"
codesign --force -s - "$APP"
echo "==> Built $APP"
