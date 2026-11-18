use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::path::{Path, PathBuf};

pub const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico",
];

pub struct LoadedImage {
    pub texture: gdk::Texture,
    pub width: i32,
    pub height: i32,
    pub format: String,
}

pub fn load(path: &Path) -> Result<LoadedImage, String> {
    let data = std::fs::read(path).map_err(|err| err.to_string())?;
    let decoded = image::load_from_memory(&data).map_err(|err| err.to_string())?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    let bytes = glib::Bytes::from_owned(rgba.into_raw());
    let texture = gdk::MemoryTexture::new(
        width as i32,
        height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        (width * 4) as usize,
    );

    Ok(LoadedImage {
        texture: texture.upcast(),
        width: width as i32,
        height: height as i32,
        format: format_name(path),
    })
}

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn first_image(dir: &Path) -> Option<PathBuf> {
    let mut files = image_files(dir);
    files.sort();
    files.into_iter().next()
}

/// Imágenes de la carpeta del archivo, ordenadas por nombre, y el índice del archivo.
pub fn siblings(path: &Path) -> (Vec<PathBuf>, usize) {
    let Some(parent) = path.parent() else {
        return (vec![path.to_path_buf()], 0);
    };

    let mut files = image_files(parent);
    files.sort_by_key(|file| {
        file.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });

    if files.is_empty() {
        return (vec![path.to_path_buf()], 0);
    }

    let index = files.iter().position(|file| file == path).unwrap_or(0);
    (files, index)
}

fn image_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && is_image(path))
                .collect()
        })
        .unwrap_or_default()
}

fn format_name(path: &Path) -> String {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "JPEG".into(),
        Some("tif" | "tiff") => "TIFF".into(),
        Some("png") => "PNG".into(),
        Some("gif") => "GIF".into(),
        Some("webp") => "WebP".into(),
        Some("bmp") => "BMP".into(),
        Some("ico") => "ICO".into(),
        _ => "IMG".into(),
    }
}
