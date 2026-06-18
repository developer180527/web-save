#!/usr/bin/env bash
# Embed the menubar companion INSIDE an engine .app bundle as a login-item
# helper. Because it then lives under web-save.app/Contents/, the menubar
# shares the engine's lifecycle: deleting (trashing) the engine app removes
# the menubar with it, and it ships as a single download.
#
# Usage: macos-menubar/embed.sh <path/to/web-save.app>
#   e.g. macos-menubar/embed.sh target/release/bundle/macos/web-save.app
set -euo pipefail
cd "$(dirname "$0")"

ENGINE_APP="${1:?usage: embed.sh <engine .app>}"
# Resolve to an absolute path before we cd around.
ENGINE_APP="$(cd "$(dirname "$ENGINE_APP")" && pwd)/$(basename "$ENGINE_APP")"
if [ ! -d "$ENGINE_APP" ]; then
  echo "error: $ENGINE_APP not found — build the engine first (npm run tauri build)" >&2
  exit 1
fi

echo "==> Building menubar app"
bash build.sh

DEST="$ENGINE_APP/Contents/Library/LoginItems"
echo "==> Embedding into ${DEST#"$ENGINE_APP"/}"
mkdir -p "$DEST"
rm -rf "$DEST/WebSave Menubar.app"
cp -R "dist/WebSave Menubar.app" "$DEST/"

echo "==> Re-signing (inside-out)"
codesign --force -s - "$DEST/WebSave Menubar.app/Contents/Frameworks/libwebsave_ffi.dylib"
codesign --force -s - "$DEST/WebSave Menubar.app"
# Re-seal the parent so its signature covers the newly embedded helper.
codesign --force -s - "$ENGINE_APP"

echo "==> Done. The menubar lives inside the engine bundle and is removed with it."
