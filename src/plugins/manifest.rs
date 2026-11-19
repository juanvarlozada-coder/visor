use serde::{Deserialize, Serialize};

/// Versión de la API de plugins que entiende esta compilación del visor.
pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_api")]
    pub api: u32,
    #[serde(default = "default_entry")]
    pub entry: String,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self, String> {
        let manifest: Self =
            serde_json::from_str(text).map_err(|err| format!("plugin.json inválido: {err}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !is_safe_component(&self.id) {
            return Err("id de plugin inválido".into());
        }
        if self.api != API_VERSION {
            return Err(format!(
                "requiere la API {}, esta versión del visor usa la {API_VERSION}",
                self.api
            ));
        }
        if !is_safe_component(&self.entry) || !self.entry.ends_with(".lua") {
            return Err("entry debe ser un archivo .lua simple".into());
        }
        Ok(())
    }
}

fn default_api() -> u32 {
    API_VERSION
}

fn default_entry() -> String {
    "main.lua".into()
}

/// Rechaza rutas y nombres con separadores o componentes especiales.
pub fn is_safe_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
