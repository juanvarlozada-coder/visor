#!/usr/bin/env bash
# Genera dist/Visor-x86_64.AppImage con linuxdeploy + plugin GTK4.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="${VISOR_TOOLS_DIR:-/tmp/visor-appimage-tools}"
DIST="$ROOT/dist"
WORK="$TOOLS/work"
APPDIR="$WORK/AppDir"

mkdir -p "$TOOLS" "$DIST" "$WORK"

fetch() {
    curl -fsSL "$1" -o "$2"
    chmod +x "$2"
}

[[ -x "$TOOLS/linuxdeploy" ]] || fetch \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" \
    "$TOOLS/linuxdeploy"
[[ -x "$TOOLS/appimagetool" ]] || fetch \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
    "$TOOLS/appimagetool"

cp "$ROOT/packaging/linuxdeploy-plugin-gtk" "$TOOLS/linuxdeploy-plugin-gtk"
chmod +x "$TOOLS/linuxdeploy-plugin-gtk"

cd "$ROOT"
cargo build --release

rm -rf "$APPDIR"
PATH="$TOOLS:$PATH" \
APPIMAGE_EXTRACT_AND_RUN=1 \
DEPLOY_GTK_VERSION=4 \
ARCH=x86_64 \
    "$TOOLS/linuxdeploy" \
        --appdir "$APPDIR" \
        --executable "$ROOT/target/release/visor" \
        --desktop-file "$ROOT/packaging/visor.desktop" \
        --icon-file "$ROOT/packaging/visor.png" \
        --plugin gtk

# libadwaita elige el tema por su cuenta: forzar GTK_THEME rompe sus estilos.
HOOK="$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"
if [[ -f "$HOOK" ]]; then
    sed -i '/^export GTK_THEME=/d' "$HOOK"
fi

# Iconos simbólicos usados por la interfaz, copiados a hicolor para que el
# AppImage no dependa del tema de iconos del sistema anfitrión.
ICONS="$APPDIR/usr/share/icons/hicolor/scalable/actions"
SOURCE_ICONS="${VISOR_ICON_THEME:-/usr/share/icons/Adwaita/symbolic}"
mkdir -p "$ICONS" "$APPDIR/usr/share/icons/hicolor/scalable/mimetypes"
for icon in \
    document-open go-previous go-next open-menu \
    zoom-in zoom-out zoom-fit-best zoom-original \
    edit-undo edit-redo view-fullscreen \
    window-close window-maximize window-minimize window-restore; do
    cp "$SOURCE_ICONS/actions/$icon-symbolic.svg" "$ICONS/" 2>/dev/null || true
done
cp "$SOURCE_ICONS/ui/window-close-symbolic.svg" "$ICONS/" 2>/dev/null || true
cp "$SOURCE_ICONS/ui/window-maximize-symbolic.svg" "$ICONS/" 2>/dev/null || true
cp "$SOURCE_ICONS/ui/window-minimize-symbolic.svg" "$ICONS/" 2>/dev/null || true
cp "$SOURCE_ICONS/ui/window-restore-symbolic.svg" "$ICONS/" 2>/dev/null || true
cp "$SOURCE_ICONS/mimetypes/image-x-generic-symbolic.svg" \
    "$APPDIR/usr/share/icons/hicolor/scalable/mimetypes/" 2>/dev/null || true
cp "$SOURCE_ICONS/mimetypes/application-x-addon-symbolic.svg" \
    "$APPDIR/usr/share/icons/hicolor/scalable/mimetypes/" 2>/dev/null || true
cp "$SOURCE_ICONS/places/user-trash-symbolic.svg" "$ICONS/" 2>/dev/null || true
cp "$SOURCE_ICONS/status/dialog-warning-symbolic.svg" "$ICONS/" 2>/dev/null || true

rm -f "$DIST"/*.AppImage
PATH="$TOOLS:$PATH" \
APPIMAGE_EXTRACT_AND_RUN=1 \
ARCH=x86_64 \
VERSION="${VISOR_VERSION:-0.3.0}" \
    "$TOOLS/appimagetool" "$APPDIR" "$DIST/Visor-x86_64.AppImage"

ls -lh "$DIST"
