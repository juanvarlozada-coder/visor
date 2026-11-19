use crate::plugins::manifest::Manifest;
use crate::plugins::runtime::Official;
use crate::plugins::store;

const SMOOTH_ZOOM_MANIFEST: &str = include_str!("../../plugins/smooth-zoom/plugin.json");
const SMOOTH_ZOOM_SCRIPT: &str = include_str!("../../plugins/smooth-zoom/main.lua");
const CAROUSEL_MANIFEST: &str = include_str!("../../plugins/carousel/plugin.json");
const CAROUSEL_SCRIPT: &str = include_str!("../../plugins/carousel/main.lua");

/// Plugins oficiales incluidos en el binario para funcionar sin conexión.
pub fn all() -> Vec<Official> {
    [
        (SMOOTH_ZOOM_MANIFEST, SMOOTH_ZOOM_SCRIPT),
        (CAROUSEL_MANIFEST, CAROUSEL_SCRIPT),
    ]
    .into_iter()
    .map(|(manifest, script)| Official {
        manifest: Manifest::parse(manifest).expect("manifiesto oficial inválido"),
        script,
    })
    .collect()
}

/// Escribe un plugin oficial en el directorio de datos del usuario.
pub fn install(id: &str) -> Result<Manifest, String> {
    let official = all()
        .into_iter()
        .find(|plugin| plugin.manifest.id == id)
        .ok_or_else(|| "plugin oficial desconocido".to_string())?;

    store::install_embedded(&official.manifest, official.script)?;
    Ok(official.manifest)
}
