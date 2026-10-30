#!/usr/bin/env bash
#
# Wrap a built WebSave.app in a drag-to-install DMG.
#
#   macos/packaging/make_dmg.sh <path/to/WebSave.app> <out.dmg>
#
# Used by .github/workflows/flutter-release.yml and runnable locally. Needs
# `create-dmg` (brew install create-dmg).
#
# The window geometry here and the artwork in background*.png are drawn for
# each other: the icons land on the glow (left) and the arrow's tip (right).
# Move one and the other has to move with it.

set -euo pipefail

app="${1:?usage: make_dmg.sh <WebSave.app> <out.dmg>}"
out="${2:?usage: make_dmg.sh <WebSave.app> <out.dmg>}"
here="$(cd "$(dirname "$0")" && pwd)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Finder only picks the retina image out of a multi-resolution TIFF; a lone
# @2x PNG is just drawn at twice the size. Stitched here rather than
# committed, because an uncompressed TIFF of a soft gradient is ~10MB in git
# and the DMG compresses it anyway.
tiffutil -cathidpicheck "$here/background.png" "$here/background@2x.png" \
  -out "$work/background.tiff" >/dev/null

mkdir "$work/src"
cp -R "$app" "$work/src/"
name="$(basename "$app")"

rm -f "$out"
create-dmg \
  --volname "WebSave" \
  --volicon "$app/Contents/Resources/AppIcon.icns" \
  --background "$work/background.tiff" \
  --window-pos 200 120 \
  --window-size 640 400 \
  --icon-size 112 \
  --text-size 13 \
  --icon "$name" 160 185 \
  --hide-extension "$name" \
  --app-drop-link 480 185 \
  --no-internet-enable \
  "$out" "$work/src/"
