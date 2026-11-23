#!/usr/bin/env bash
# Run on the build machine after `cargo packager -f appimage`, on ARM (aarch64):
#   scripts/fix_appimage_libs.sh /path/to/packager-output.AppImage
# cargo-packager's AppRun sets LD_LIBRARY_PATH to usr/lib and usr/lib/x86_64-linux-gnu
# (and i386), never usr/lib/aarch64-linux-gnu, where the bundled libraries land on
# ARM. This unpacks the AppImage, copies those libraries up into usr/lib, and packs it
# again in place. It is a no-op when there is no aarch64-linux-gnu folder.
set -euxo pipefail

appimage=$(realpath "${1:?usage: scripts/fix_appimage_libs.sh <file.AppImage>}")
machine=$(uname -m)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# appimagetool is pinned to a release (not "continuous") so the build is repeatable.
tool_url=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-${machine}.AppImage
curl -fsSL -o "$work/appimagetool" "$tool_url"
chmod +x "$work/appimagetool"

cd "$work"
cp "$appimage" ./in.AppImage
chmod +x ./in.AppImage
./in.AppImage --appimage-extract > /dev/null

lib_dir=squashfs-root/usr/lib/${machine}-linux-gnu
if [ ! -d "$lib_dir" ]; then
  echo "no $lib_dir in the AppImage: nothing to fix"
  exit 0
fi
cp -a "$lib_dir"/. squashfs-root/usr/lib/

ARCH=$machine APPIMAGE_EXTRACT_AND_RUN=1 ./appimagetool squashfs-root out.AppImage
chmod +x out.AppImage
cp out.AppImage "$appimage"
