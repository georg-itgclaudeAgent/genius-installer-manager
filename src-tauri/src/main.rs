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
fn get_status() -> StatusInfo {
    StatusInfo {
        installed: install::is_installed(),
        installed_version: install::read_installed_version(),
        install_path: paths::extension_install_dir().to_string_lossy().to_string(),
        premiere_running_warning: install::premiere_likely_has_extension_open(),
    }
}

#[tauri::command]
async fn install_from_url(url: String) -> Result<String, String> {
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

    install::extract_zip_to_install_dir(&bytes)?;
    install::set_cep_debug_mode()?;

    let version = install::read_installed_version()
        .unwrap_or_else(|| "(unknown)".to_string());
    Ok(version)
}

#[tauri::command]
fn uninstall_extension() -> Result<(), String> {
    install::uninstall()
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
