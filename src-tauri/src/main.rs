#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod download;
mod install;
mod paths;

use serde::Serialize;

#[derive(Serialize)]
struct StatusInfo {
    installed: bool,
    installed_version: Option<String>,
    install_path: String,
    premiere_running_warning: bool,
}

#[tauri::command]
fn list_extensions() -> Vec<paths::ExtensionSpec> {
    paths::EXTENSIONS.to_vec()
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
    let spec = paths::find(&id).ok_or_else(|| format!("Unknown extension id: {:?}", id))?;
    if !download::is_allowed_download_url(&url, spec) {
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
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_extensions,
            get_status,
            install_from_url,
            uninstall_extension
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
