#!/usr/bin/env bash
# Build the WebSave iPad app for the iOS SIMULATOR and install it on the
# booted device. Links websave-core through UniFFI (static lib), same pattern
# as the macOS menubar app. No Xcode project and no Developer account needed
# for the simulator.
#
# Usage: ios-app/build.sh          # build + install + launch on booted sim
set -euo pipefail
cd "$(dirname "$0")"
ROOT=..
TARGET=aarch64-apple-ios-sim
BUNDLE_ID=com.venugopal.web-save.ios
SDK=$(xcrun --sdk iphonesimulator --show-sdk-path)

echo "==> Building Rust core for $TARGET"
cargo build --release -p websave-ffi --target "$TARGET" \
    --manifest-path "$ROOT/Cargo.toml"

echo "==> Generating Swift bindings"
mkdir -p Generated
cargo run --release -p websave-ffi --bin uniffi-bindgen \
    --manifest-path "$ROOT/Cargo.toml" -- \
    generate --library "$ROOT/target/release/libwebsave_ffi.dylib" \
    --language swift --out-dir Generated

APP="dist/WebSave.app"
echo "==> Assembling app bundle"
rm -rf "$APP"
mkdir -p "$APP"
cp Info.plist "$APP/Info.plist"

echo "==> Compiling SwiftUI app"
# Link the Rust staticlib directly; add the system libs its deps pull in
# (SQLite/C++ from rusqlite, Security/resolv from the platform).
swiftc -O \
    Sources/*.swift Generated/websave_ffi.swift \
    -import-objc-header Generated/websave_ffiFFI.h \
    -sdk "$SDK" \
    -target arm64-apple-ios17.0-simulator \
    -L "$ROOT/target/$TARGET/release" -lwebsave_ffi \
    -lc++ -lresolv \
    -framework Security -framework SystemConfiguration \
    -framework SwiftUI -framework UIKit \
    -o "$APP/WebSave"

echo "==> Installing on the booted simulator"
xcrun simctl install booted "$APP"
xcrun simctl launch booted "$BUNDLE_ID"
echo "==> Launched $BUNDLE_ID"
