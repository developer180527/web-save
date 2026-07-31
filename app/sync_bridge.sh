#!/usr/bin/env bash
#
# Regenerate the bridge bindings and rebuild every Rust binary that depends
# on them, in the one order that leaves nothing stale.
#
# The bindings carry a content hash that both sides check at startup. There
# are three separate Rust binaries behind that one Dart API — a macOS dylib,
# an iOS device staticlib and an iOS simulator staticlib — and `cargo build`
# refreshes exactly one of them. Change bridge/src/api/ and rebuild only the
# host, and the next iOS run dies in RustLib.init with a hash mismatch that
# names neither the file you edited nor the target you forgot.
#
# Codegen has to come first: it rewrites bridge/src/frb_generated.rs as well
# as the Dart, so building before generating compiles the previous API.
#
# Usage: ./sync_bridge.sh [--ios-only|--host-only]

set -euo pipefail
cd "$(dirname "$0")/.."

want_host=1
want_ios=1
case "${1:-}" in
  --ios-only) want_host=0 ;;
  --host-only) want_ios=0 ;;
  "") ;;
  *) echo "usage: $0 [--ios-only|--host-only]" >&2; exit 2 ;;
esac

echo "==> generating bindings"
flutter_rust_bridge_codegen generate \
  --rust-input crate::api \
  --rust-root bridge \
  --dart-output app/lib/src/rust

if [ "$want_host" = 1 ]; then
  echo "==> building host dylib (macOS app)"
  cargo build -p websave_bridge
fi

if [ "$want_ios" = 1 ]; then
  echo "==> building iOS staticlibs (device + simulator)"
  ./app/build_rust_ios.sh
fi

echo
echo "==> all bridge binaries match the generated bindings"
