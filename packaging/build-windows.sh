#!/usr/bin/env bash
# Genera dist/Visor-<version>-windows-x64.zip usando el runtime GTK4 de gvsbuild.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="0.1.0"
CRATE_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
VERSION="${VISOR_VERSION:-$CRATE_VERSION}"
CACHE="${VISOR_WINDOWS_CACHE:-/tmp/opencode}"
GTK_DIR="${GTK4_WINDOWS_DIR:-$CACHE/gtk4-win}"
GTK_ZIP="${GTK4_WINDOWS_ZIP:-$CACHE/gtk4-win64.zip}"
GTK_URL="https://github.com/wingtk/gvsbuild/releases/download/2026.10.0/GTK4_Gvsbuild_2026.10.0_x64.zip"
TARGET="x86_64-pc-windows-msvc"
BUNDLE="$ROOT/dist/Visor-$VERSION-windows-x64"

if [[ ! -f "$GTK_DIR/lib/pkgconfig/gtk4.pc" ]]; then
    [[ -f "$GTK_ZIP" ]] || curl -fL "$GTK_URL" -o "$GTK_ZIP"
    rm -rf "$GTK_DIR"
    mkdir -p "$GTK_DIR"
    unzip -q "$GTK_ZIP" -d "$GTK_DIR"
    sed -i "s#^prefix=.*#prefix=$GTK_DIR#" "$GTK_DIR"/lib/pkgconfig/*.pc
fi

cd "$ROOT"

RUSTFLAGS="-C target-feature=+crt-static" \
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_LIBDIR="$GTK_DIR/lib/pkgconfig" \
PKG_CONFIG_PATH="$GTK_DIR/lib/pkgconfig" \
    cargo xwin build --release --target "$TARGET"

EXE="$ROOT/target/$TARGET/release/visor.exe"
[[ -f "$EXE" ]] || { echo "No se generó $EXE"; exit 1; }

rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/bin" \
    "$BUNDLE/share/glib-2.0" \
    "$BUNDLE/share/icons" \
    "$BUNDLE/lib/gdk-pixbuf-2.0/2.10.0/loaders"

cp "$EXE" "$BUNDLE/bin/visor.exe"
python3 "$ROOT/packaging/windows-dlls.py" "$EXE" "$GTK_DIR/bin" "$BUNDLE/bin"

cp -r "$GTK_DIR/share/glib-2.0/schemas" "$BUNDLE/share/glib-2.0/"
cp -r "$GTK_DIR/share/icons/Adwaita" "$BUNDLE/share/icons/"
cp "$GTK_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders/pixbufloader_svg.dll" \
    "$BUNDLE/lib/gdk-pixbuf-2.0/2.10.0/loaders/"
cp "$GTK_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache" \
    "$BUNDLE/lib/gdk-pixbuf-2.0/2.10.0/"

rm -f "$ROOT/dist/Visor-$VERSION-windows-x64.zip"
python3 -c "import shutil; shutil.make_archive('$ROOT/dist/Visor-$VERSION-windows-x64', 'zip', '$ROOT/dist', '$(basename "$BUNDLE")')"

echo "Listo:"
du -sh "$BUNDLE" "$ROOT/dist/Visor-$VERSION-windows-x64.zip"
