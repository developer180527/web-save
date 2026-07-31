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

# Match the Xcode project's minimum, or every object in the archive is stamped
# with the SDK's version and the linker warns once per object file:
#   "was built for newer 'iOS' version (26.5) than being linked (13.0)"
# Noisy rather than fatal, but it also means the library would refuse to run
# on anything older than the machine that built it.
export IPHONEOS_DEPLOYMENT_TARGET=13.0

for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    echo "==> $target"
    cargo build -p websave_bridge --release --target "$target"
done
echo "==> done"
ls -la target/aarch64-apple-ios{,-sim}/release/libwebsave_bridge.a
