use crate::plugins::manifest::{Manifest, is_safe_component};
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const USER_AGENT: &str = "visor-plugin-store (+https://github.com/juanvarlozada-coder/visor)";

pub struct Installed {
    pub manifest: Manifest,
    pub script: String,
}

#[derive(Debug, PartialEq)]
pub struct RepoRef {
    pub owner: String,
    pub repo: String,
    pub branch: Option<String>,
    pub path: String,
}

#[derive(Serialize, Deserialize, Default)]
struct State {
    enabled: Vec<String>,
}

pub fn plugins_dir() -> PathBuf {
    glib::user_data_dir().join("visor").join("plugins")
}

fn state_path() -> PathBuf {
    glib::user_config_dir().join("visor").join("plugins.json")
}

pub fn load_installed() -> Vec<Installed> {
    let Ok(entries) = std::fs::read_dir(plugins_dir()) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter_map(|entry| load_plugin_dir(&entry.path()))
        .collect()
}

pub fn load_plugin_dir(dir: &Path) -> Option<Installed> {
    let manifest_text = std::fs::read_to_string(dir.join("plugin.json")).ok()?;
    let manifest = Manifest::parse(&manifest_text).ok()?;
    let script = std::fs::read_to_string(dir.join(&manifest.entry)).ok()?;
    Some(Installed { manifest, script })
}

pub fn find(id: &str) -> Option<Installed> {
    load_plugin_dir(&plugins_dir().join(id))
}

pub fn is_installed(id: &str) -> bool {
    plugins_dir().join(id).join("plugin.json").is_file()
}

pub fn write(manifest: &Manifest, script: &str) -> Result<(), String> {
    manifest.validate()?;
    let dir = plugins_dir().join(&manifest.id);
    std::fs::create_dir_all(&dir).map_err(|err| format!("no se pudo crear {dir:?}: {err}"))?;

    let json = serde_json::to_string_pretty(manifest).map_err(|err| err.to_string())?;
    std::fs::write(dir.join("plugin.json"), json).map_err(|err| err.to_string())?;
    std::fs::write(dir.join(&manifest.entry), script).map_err(|err| err.to_string())?;
    Ok(())
}

pub fn remove(id: &str) -> Result<(), String> {
    if !is_safe_component(id) {
        return Err("id inválido".into());
    }
    let dir = plugins_dir().join(id);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|err| format!("no se pudo eliminar: {err}"))?;
    }
    let mut enabled = load_enabled();
    enabled.retain(|item| item != id);
    save_enabled(&enabled)?;
    Ok(())
}

pub fn load_enabled() -> Vec<String> {
    std::fs::read_to_string(state_path())
        .ok()
        .and_then(|text| serde_json::from_str::<State>(&text).ok())
        .map(|state| state.enabled)
        .unwrap_or_default()
}

pub fn save_enabled(enabled: &[String]) -> Result<(), String> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let state = State {
        enabled: enabled.to_vec(),
    };
    let json = serde_json::to_string_pretty(&state).map_err(|err| err.to_string())?;
    std::fs::write(path, json).map_err(|err| err.to_string())
}

pub fn install_embedded(manifest: &Manifest, script: &str) -> Result<(), String> {
    write(manifest, script)
}

pub fn install_from_github(input: &str) -> Result<Manifest, String> {
    let repo = parse_github(input)?;
    let branch = match repo.branch.clone() {
        Some(branch) => branch,
        None => default_branch(&repo.owner, &repo.repo)?,
    };

    let base = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}",
        repo.owner,
        repo.repo,
        branch,
        repo.path.trim_matches('/')
    );
    let manifest_text = fetch(&format!("{base}/plugin.json"))?;
    let manifest = Manifest::parse(&manifest_text)?;
    let script = fetch(&format!("{base}/{}", manifest.entry))?;
    write(&manifest, &script)?;
    Ok(manifest)
}

pub fn parse_github(input: &str) -> Result<RepoRef, String> {
    let raw = input.trim().trim_end_matches('/');
    if raw.is_empty() {
        return Err("URL vacía".into());
    }

    let raw = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
        .unwrap_or(raw);
    let raw = raw
        .strip_prefix("www.github.com/")
        .or_else(|| raw.strip_prefix("github.com/"))
        .unwrap_or(raw);
    let raw = raw.split(['?', '#']).next().unwrap_or(raw);

    let parts: Vec<&str> = raw.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() < 2 {
        return Err("formato esperado: usuario/repo".into());
    }

    let owner = parts[0];
    let repo = parts[1].trim_end_matches(".git");
    if !is_safe_component(owner) || !is_safe_component(repo) {
        return Err("usuario o repositorio inválidos".into());
    }

    let (branch, path) = match parts.get(2) {
        None => (None, String::new()),
        Some(&"tree") if parts.len() >= 5 => {
            let branch = parts[3];
            let path = parts[4..].join("/");
            if !is_safe_component(branch) || !path.split('/').all(is_safe_component) {
                return Err("rama o ruta inválidas".into());
            }
            (Some(branch.to_string()), path)
        }
        Some(_) => return Err("solo se admiten URLs de repositorio o de carpeta".into()),
    };

    Ok(RepoRef {
        owner: owner.to_string(),
        repo: repo.to_string(),
        branch,
        path,
    })
}

fn default_branch(owner: &str, repo: &str) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}");
    let body = fetch(&url)?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|err| format!("respuesta inválida: {err}"))?;
    json.get("default_branch")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| "no se pudo determinar la rama del repositorio".into())
}

fn fetch(url: &str) -> Result<String, String> {
    let mut response = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json, text/plain, */*")
        .config()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .call()
        .map_err(|err| format!("{url}: {err}"))?;

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("{url}: {err}"))?;

    if body.len() > 512 * 1024 {
        return Err("respuesta demasiado grande".into());
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acepta_formatos_de_github() {
        let short = parse_github("juanvarlozada-coder/visor").unwrap();
        assert_eq!(short.owner, "juanvarlozada-coder");
        assert_eq!(short.repo, "visor");
        assert_eq!(short.branch, None);
        assert_eq!(short.path, "");

        let url = parse_github("https://github.com/acme/plugins/tree/main/zoom/").unwrap();
        assert_eq!(url.branch.as_deref(), Some("main"));
        assert_eq!(url.path, "zoom");
    }

    #[test]
    fn rechaza_rutas_peligrosas() {
        assert!(parse_github("../etc").is_err());
        assert!(parse_github("acme/plugins/tree/main/../../x").is_err());
        assert!(parse_github("").is_err());
    }
}
