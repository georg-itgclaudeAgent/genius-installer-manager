#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod download;
mod install;
mod legacy;
mod paths;
mod registry;
mod runtime;

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

// ---------- Genius Cut runtime ----------

fn runtime_base(id: &str) -> std::path::PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    std::path::PathBuf::from(local).join("itGenius").join(id.trim_start_matches("com.attract."))
}

fn system32() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())).join("System32")
}

#[derive(Serialize)]
struct RuntimeStatus {
    installed: bool,
    version: Option<String>,
    flavour: &'static str,
    latest: Option<String>,
}

fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 { return None; }
    let n = |p: &str| if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) { p.parse::<u64>().ok() } else { None };
    Some((n(parts[0])?, n(parts[1])?, n(parts[2])?))
}

/// Highest stable `<prefix><semver>` tag among a GitHub releases list.
fn pick_latest_runtime(releases: &[serde_json::Value], prefix: &str) -> Option<String> {
    releases.iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false) && !r["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|r| r["tag_name"].as_str()?.strip_prefix(prefix).map(str::to_string))
        .filter_map(|v| parse_semver(&v).map(|k| (k, v)))
        .max_by_key(|(k, _)| *k)
        .map(|(_, v)| v)
}

async fn latest_runtime_version(spec: &registry::ExtensionSpec) -> Result<Option<String>, String> {
    let rt = spec.runtime.as_ref().ok_or("This app has no runtime.")?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("genius-installer-manager")
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("https://api.github.com/repos/georg-itgclaudeAgent/{}/releases?per_page=50", spec.repo);
    let resp = client.get(&url).header("Accept", "application/vnd.github+json").send().await
        .map_err(|e| format!("Couldn't reach GitHub: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {} for the releases list", resp.status()));
    }
    let releases: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(pick_latest_runtime(&releases, &rt.tag_prefix))
}

fn runtime_spec(id: &str) -> Result<registry::ExtensionSpec, String> {
    let spec = registry::find(id).ok_or_else(|| format!("Unknown extension id: {:?}", id))?;
    if spec.runtime.is_none() { return Err(format!("{} has no runtime.", spec.name)); }
    Ok(spec)
}

#[tauri::command]
async fn runtime_status(id: String) -> Result<RuntimeStatus, String> {
    let spec = runtime_spec(&id)?;
    let flavour = runtime::detect_flavour(&system32());
    let cur = runtime::current(&runtime_base(&id));
    // A failed lookup (offline) shouldn't hide what's installed.
    let latest = latest_runtime_version(&spec).await.unwrap_or(None);
    Ok(RuntimeStatus {
        installed: cur.is_some(),
        version: cur.map(|p| p.version),
        flavour: flavour.as_str(),
        latest,
    })
}

async fn download_to(
    app: &tauri::AppHandle, id: &str, client: &reqwest::Client, url: &str, dest: &std::path::Path,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tauri::Emitter;
    use tokio::io::AsyncWriteExt;
    let resp = client.get(url).send().await.map_err(|e| format!("Download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }
    let total = resp.content_length();
    let mut file = tokio::fs::File::create(dest).await.map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download failed: {}", e))?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= std::time::Duration::from_millis(500) {
            last_emit = std::time::Instant::now();
            let _ = app.emit("runtime-progress", serde_json::json!({ "id": id, "downloaded": downloaded, "total": total }));
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    let _ = app.emit("runtime-progress", serde_json::json!({ "id": id, "downloaded": downloaded, "total": total }));
    Ok(())
}

#[tauri::command]
async fn ensure_runtime(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let spec = runtime_spec(&id)?;
    let tag_prefix = spec.runtime.as_ref().map(|r| r.tag_prefix.clone()).unwrap_or_default();
    let version = latest_runtime_version(&spec).await?
        .ok_or_else(|| "No runtime has been released yet.".to_string())?;
    let base = runtime_base(&id);
    if let Some(cur) = runtime::current(&base) {
        if cur.version == version { return Ok(version); }
    }
    let flavour = runtime::detect_flavour(&system32());
    let name = runtime::asset_name(flavour, &version);
    let zip_url = format!(
        "https://github.com/georg-itgclaudeAgent/{}/releases/download/{}{}/{}", spec.repo, tag_prefix, version, name);
    let sha_url = format!("{}.sha256", zip_url);
    for u in [&zip_url, &sha_url] {
        if !runtime::is_allowed_runtime_url(u, &spec) {
            return Err(format!("Refusing to download from an untrusted location: {}", u));
        }
    }
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;

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

    // The checksum is small; keep it in memory.
    let sha_resp = client.get(&sha_url).send().await.map_err(|e| format!("Download failed: {}", e))?;
    if !sha_resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", sha_resp.status()));
    }
    let sha_text = sha_resp.text().await.map_err(|e| e.to_string())?;
    let expected = sha_text.split_whitespace().next().unwrap_or("").to_string();

    let part = base.join("download.part");
    let result: Result<(), String> = async {
        download_to(&app, &id, &client, &zip_url, &part).await?;
        let (b, v, p) = (base.clone(), version.clone(), part.clone());
        tauri::async_runtime::spawn_blocking(move || runtime::install_zip(&b, &v, flavour, &p, &expected))
            .await
            .map_err(|e| e.to_string())??;
        Ok(())
    }.await;
    let _ = std::fs::remove_file(&part);
    result?;
    Ok(version)
}

#[tauri::command]
fn remove_runtime(id: String) -> Result<(), String> {
    runtime_spec(&id)?;
    runtime::remove_all(&runtime_base(&id))
}

#[tauri::command]
fn uninstall_extension(id: String) -> Result<(), String> {
    install::uninstall(&id)?;
    if registry::find(&id).map_or(false, |s| s.runtime.is_some()) {
        runtime::remove_all(&runtime_base(&id))?;
    }
    Ok(())
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
            uninstall_extension,
            runtime_status,
            ensure_runtime,
            remove_runtime
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_highest_stable_runtime_tag() {
        let r = vec![
            json!({"tag_name": "runtime-v1.0.0"}),
            json!({"tag_name": "runtime-v1.10.0"}),
            json!({"tag_name": "runtime-v2.0.0", "prerelease": true}),
            json!({"tag_name": "runtime-v3.0.0", "draft": true}),
            json!({"tag_name": "v9.0.0"}),
            json!({"tag_name": "runtime-vbad"}),
        ];
        assert_eq!(pick_latest_runtime(&r, "runtime-v"), Some("1.10.0".into()));
        assert_eq!(pick_latest_runtime(&r[4..], "runtime-v"), None);
    }
}
