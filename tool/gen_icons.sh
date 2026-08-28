#!/usr/bin/env bash
#
# Generate the Flutter app's non-Apple launcher icons.
#
# Apple platforms are deliberately NOT handled here. macOS and iOS consume
# `Assets/AppIcon.icon` directly: Xcode 26's actool compiles that bundle into
# every size *and* every appearance, which is what preserves the dark-mode
# variant (dark plate, white globe) that this flat PNG pipeline cannot express.
# Both Runner targets reference that one file, so there is nothing to generate.
#
# Everything else — Windows, Android, Linux — has no notion of an appearance
# specialisation, so they all get the single purple icon.
#
# Requires ImageMagick. Re-run after editing the icon in Icon Composer:
#   ./tool/gen_icons.sh

set -euo pipefail

cd "$(dirname "$0")/.."

readonly ICON_DIR="Assets/AppIcon.icon"
readonly FLAT="Assets/icon.png"
readonly APP="web-save-app"

# Background fill from icon.json's automatic-gradient (srgb 0.58188,0.21569,1).
readonly PURPLE="#9437FF"

for tool in magick; do
  command -v "$tool" >/dev/null || { echo "need $tool (brew install imagemagick)" >&2; exit 1; }
done
[[ -f "$FLAT" ]] || { echo "missing $FLAT" >&2; exit 1; }
# Fail loudly if the Flutter project moves again, rather than silently writing
# icons into a directory nobody builds.
[[ -f "$APP/pubspec.yaml" ]] || { echo "no Flutter project at $APP/" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# ---------------------------------------------------------------------------
# Transparent foreground, composited from the Icon Composer layers.
#
# Android's adaptive icons mask the artwork to a shape the OEM chooses, so
# they need the glyph WITHOUT the purple plate — the plate is supplied
# separately as a colour. The geometry below is read straight out of
# icon.json so this stays in step with the master: the globe sits at scale
# .83 centred, and the tag at scale .51 offset (227.84, -35.70) points from
# centre on a 1024pt canvas, y-up.
# ---------------------------------------------------------------------------
magick -size 1024x1024 xc:none \
  \( "$ICON_DIR/Assets/internet-globe-icon.png" -resize 850x850 \) \
    -geometry +87+87 -composite \
  \( "$ICON_DIR/Assets/flat-style-blank-tag-icon-in-yellow.png" -resize '595x500!' \) \
    -geometry +442+298 -composite \
  "$work/foreground.png"

# ---------------------------------------------------------------------------
# Windows: one multi-resolution .ico. 16px is the one that actually has to be
# legible — it is what the taskbar and title bar draw.
# ---------------------------------------------------------------------------
magick "$FLAT" -define icon:auto-resize=256,128,64,48,32,24,16 \
  "$APP/windows/runner/resources/app_icon.ico"
echo "windows: app_icon.ico"

# ---------------------------------------------------------------------------
# Android: legacy square icons for pre-API-26, plus adaptive layers for 26+.
#
# An adaptive foreground is a 108dp canvas of which only the central 72dp is
# guaranteed visible — the rest is margin the launcher may crop for parallax.
# So the artwork is drawn at 2/3 of the canvas and centred.
# ---------------------------------------------------------------------------
legacy=(mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192)
for entry in "${legacy[@]}"; do
  density="${entry%%:*}"; size="${entry##*:}"
  magick "$FLAT" -resize "${size}x${size}" \
    "$APP/android/app/src/main/res/mipmap-$density/ic_launcher.png"
done
echo "android: legacy mipmaps"

adaptive=(mdpi:108 hdpi:162 xhdpi:216 xxhdpi:324 xxxhdpi:432)
for entry in "${adaptive[@]}"; do
  density="${entry%%:*}"; canvas="${entry##*:}"
  content=$(( canvas * 2 / 3 ))
  magick "$work/foreground.png" -resize "${content}x${content}" \
    -background none -gravity center -extent "${canvas}x${canvas}" \
    "$APP/android/app/src/main/res/mipmap-$density/ic_launcher_foreground.png"
done
echo "android: adaptive foregrounds"

# ---------------------------------------------------------------------------
# Linux: a single PNG the GTK window points at. GTK scales it for the window
# manager, so one generous size is enough.
# ---------------------------------------------------------------------------
mkdir -p "$APP/linux/runner/resources"
magick "$FLAT" -resize 256x256 "$APP/linux/runner/resources/app_icon.png"
echo "linux: app_icon.png"

echo
echo "Apple targets read Assets/AppIcon.icon directly — nothing generated."
