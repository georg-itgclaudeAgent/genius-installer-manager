#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
fn get_status(id: String) -> StatusInfo {
    StatusInfo {
        installed: install::is_installed(&id),
        installed_version: install::read_installed_version(&id),
        install_path: paths::install_dir(&id).map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        premiere_running_warning: install::premiere_likely_has_extension_open(&id),
    }
}

#[tauri::command]
async fn install_from_url(id: String, url: String) -> Result<String, String> {
    install::check_cep_dir_writable()?;

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?
        .to_vec();

    install::extract_zip_to_install_dir(&id, &bytes)?;
    install::set_cep_debug_mode()?;

    let version = install::read_installed_version(&id)
        .unwrap_or_else(|| "(unknown)".to_string());
    Ok(version)
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
            get_status,
            install_from_url,
            uninstall_extension
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
