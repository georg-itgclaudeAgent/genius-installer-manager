use crate::paths;
use std::fs;
use std::io::Cursor;

/// Read the installed extension's version from its manifest.xml.
/// Returns None if the extension isn't installed or the manifest can't be parsed.
pub fn read_installed_version() -> Option<String> {
    let manifest = paths::installed_manifest_path();
    if !manifest.exists() {
        return None;
    }
    let content = fs::read_to_string(&manifest).ok()?;
    let needle = "ExtensionBundleVersion=\"";
    let start = content.find(needle)? + needle.len();
    let end_offset = content[start..].find('"')?;
    Some(content[start..start + end_offset].to_string())
}

pub fn is_installed() -> bool {
    paths::installed_manifest_path().exists()
}

/// Set HKCU\Software\Adobe\CSXS.12\PlayerDebugMode = "1" so unsigned CEP extensions load.
#[cfg(windows)]
pub fn set_cep_debug_mode() -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey("Software\\Adobe\\CSXS.12")
        .map_err(|e| format!("Failed to open registry key: {}", e))?;
    key.set_value("PlayerDebugMode", &"1")
        .map_err(|e| format!("Failed to write PlayerDebugMode: {}", e))?;
    Ok(())
}

#[cfg(not(windows))]
pub fn set_cep_debug_mode() -> Result<(), String> {
    // No-op on non-Windows builds. Manager only supports Windows for v1.
    Ok(())
}

/// Extract a zip archive (in-memory bytes) into the extension install directory,
/// replacing whatever was there.
pub fn extract_zip_to_install_dir(zip_bytes: &[u8]) -> Result<(), String> {
    let target = paths::extension_install_dir();
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|e| format!("Failed to remove existing install: {}", e))?;
    }
    fs::create_dir_all(&target)
        .map_err(|e| format!("Failed to create install dir: {}", e))?;

    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("Failed to read zip: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry {}: {}", i, e))?;
        let outpath: std::path::PathBuf = match entry.enclosed_name() {
            Some(p) => target.join(p),
            None => continue, // skip suspicious entries
        };
        if entry.is_dir() {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create dir {:?}: {}", outpath, e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent {:?}: {}", parent, e))?;
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file {:?}: {}", outpath, e))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Failed to write file {:?}: {}", outpath, e))?;
        }
    }

    Ok(())
}

/// Remove the extension install directory.
pub fn uninstall() -> Result<(), String> {
    let target = paths::extension_install_dir();
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|e| format!("Failed to remove install dir: {}", e))?;
    }
    Ok(())
}

/// Sanity check: ensure we can write to the CEP extensions dir before downloading.
/// Returns Err with a helpful message if the dir doesn't exist or isn't writable.
pub fn check_cep_dir_writable() -> Result<(), String> {
    let dir = paths::cep_extensions_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create CEP extensions dir at {:?}: {}", dir, e))?;
    let probe = dir.join(".pr-extension-manager-write-test");
    fs::write(&probe, b"")
        .map_err(|e| format!("CEP extensions dir is not writable: {}", e))?;
    fs::remove_file(&probe).ok();
    Ok(())
}

/// Detect whether Premiere likely has the extension's files locked (because Premiere is running).
/// Best-effort: attempt a tiny write to the install dir's CSXS subdir.
pub fn premiere_likely_has_extension_open() -> bool {
    let probe = paths::extension_install_dir().join("CSXS").join(".lock-probe");
    if probe.parent().map_or(false, |p| !p.exists()) {
        return false; // dir doesn't exist → not installed → not locked
    }
    let _ = fs::create_dir_all(probe.parent().unwrap());
    fs::write(&probe, b"").is_err()
}
