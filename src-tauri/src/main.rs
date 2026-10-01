#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod download;
mod install;
mod legacy;
mod paths;
mod registry;

use serde::Serialize;

#[derive(Serialize)]
struct StatusInfo {
    installed: bool,
    installed_version: Option<String>,
    install_path: String,
    premiere_running_warning: bool,
}

#[tauri::command]
fn list_extensions() -> Vec<registry::ExtensionSpec> {
    registry::current()
}

#[derive(Serialize)]
struct RegistryStatus {
    extensions: Vec<registry::ExtensionSpec>,
    /// "github" (fresh), "cache" (last good copy) or "built-in".
    source: &'static str,
    error: Option<String>,
}

fn registry_cache(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    use tauri::Manager;
    app.path().app_data_dir().ok().map(|d| d.join("registry-cache.json"))
}

async fn fetch_registry() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(registry::REGISTRY_URL).send().await.map_err(|e| format!("Couldn't reach GitHub: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {} for registry.json", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

/// Fetch the app list from GitHub (falling back to the cached copy, then the built-in list)
/// and make it the list every other command validates against.
#[tauri::command]
async fn refresh_registry(app: tauri::AppHandle) -> RegistryStatus {
    let cache = registry_cache(&app);
    let fresh = match fetch_registry().await {
        Ok(text) => registry::parse(&text).map(|list| (list, text)),
        Err(e) => Err(e),
    };
    match fresh {
        Ok((list, text)) => {
            if let Some(path) = &cache {
                if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
                let _ = std::fs::write(path, text);
            }
            let merged = registry::merge(list, registry::builtin());
            registry::set_current(merged.clone());
            RegistryStatus { extensions: merged, source: "github", error: None }
        }
        Err(e) => {
            let cached = cache
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|t| registry::parse(&t).ok());
            let (list, source) = match cached {
                Some(l) => (registry::merge(l, registry::builtin()), "cache"),
                None => (registry::builtin(), "built-in"),
            };
            registry::set_current(list.clone());
            RegistryStatus { extensions: list, source, error: Some(e) }
        }
    }
}

#[tauri::command]
fn get_status(id: String) -> Result<StatusInfo, String> {
    let dir = paths::install_dir(&id)?;
    Ok(StatusInfo {
        installed: install::is_installed(&id),
        installed_version: install::read_installed_version(&id),
        install_path: dir.to_string_lossy().to_string(),
        premiere_running_warning: install::premiere_likely_has_extension_open(&id),
    })
}

#[tauri::command]
async fn install_from_url(id: String, url: String) -> Result<String, String> {
    // Reject unknown ids before any network or disk work.
    let spec = registry::find(&id).ok_or_else(|| format!("Unknown extension id: {:?}", id))?;
    if !download::is_allowed_download_url(&url, &spec) {
        return Err(format!("Refusing to download from an untrusted location: {}", url));
    }
    install::check_cep_dir_writable()?;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("too many redirects")
            } else if download::is_allowed_redirect(attempt.url()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|e| format!("Download failed: {}", e))?;
    let resp = client.get(&url).send().await.map_err(|e| format!("Download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await
        .map_err(|e| format!("Failed to read response body: {}", e))?
        .to_vec();

    install::extract_zip_to_install_dir(&id, &bytes)?;
    install::set_cep_debug_mode()?;

    Ok(install::read_installed_version(&id).unwrap_or_else(|| "(unknown)".to_string()))
}

#[tauri::command]
fn uninstall_extension(id: String) -> Result<(), String> {
    install::uninstall(&id)
}

fn main() {
    legacy::remove_legacy_install();

    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_extensions,
            refresh_registry,
            get_status,
            install_from_url,
            uninstall_extension
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
