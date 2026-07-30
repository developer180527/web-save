#!/usr/bin/env bash
# Build the Rust bridge for iOS, both slices.
#
# Xcode links whichever one matches the SDK it is building for (see
# ios/Flutter/Rust.xcconfig), so both have to exist before `flutter run -d
# <device>`. Release profile for both: a debug Rust build is several times
# larger and noticeably slower to link, and none of this is code you step
# through from Xcode anyway.
set -euo pipefail
cd "$(dirname "$0")/.."

for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    echo "==> $target"
    cargo build -p websave-bridge --release --target "$target"
done
echo "==> done"
ls -la target/aarch64-apple-ios{,-sim}/release/libwebsave_bridge.a
