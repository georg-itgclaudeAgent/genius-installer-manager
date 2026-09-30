use crate::paths;
use std::fs;
use std::io::Cursor;
use std::path::Path;

// ── path-based internals (tested directly) ─────────────────────────

pub(crate) fn read_version_at(dir: &Path) -> Option<String> {
    let manifest = dir.join("CSXS").join("manifest.xml");
    let content = fs::read_to_string(&manifest).ok()?;
    let needle = "ExtensionBundleVersion=\"";
    let start = content.find(needle)? + needle.len();
    let end_offset = content[start..].find('"')?;
    Some(content[start..start + end_offset].to_string())
}

pub(crate) fn extract_zip_into(target: &Path, zip_bytes: &[u8]) -> Result<(), String> {
    // Validate the archive before deleting anything, so a bad download
    // never leaves the user with no extension at all.
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("Failed to read zip: {}", e))?;

    if target.exists() {
        fs::remove_dir_all(target)
            .map_err(|e| format!("Failed to remove existing install: {}", e))?;
    }
    fs::create_dir_all(target).map_err(|e| format!("Failed to create install dir: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry {}: {}", i, e))?;
        let outpath = match entry.enclosed_name() {
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

pub(crate) fn remove_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| format!("Failed to remove install dir: {}", e))?;
    }
    Ok(())
}

// ── id-scoped public API ───────────────────────────────────────────

pub fn read_installed_version(id: &str) -> Option<String> {
    read_version_at(&paths::install_dir(id).ok()?)
}

pub fn is_installed(id: &str) -> bool {
    paths::manifest_path(id).map(|p| p.exists()).unwrap_or(false)
}

pub fn extract_zip_to_install_dir(id: &str, zip_bytes: &[u8]) -> Result<(), String> {
    extract_zip_into(&paths::install_dir(id)?, zip_bytes)
}

pub fn uninstall(id: &str) -> Result<(), String> {
    remove_dir(&paths::install_dir(id)?)
}

/// Best-effort: attempt a tiny write inside the extension's CSXS dir.
pub fn premiere_likely_has_extension_open(id: &str) -> bool {
    let Ok(dir) = paths::install_dir(id) else { return false };
    let csxs = dir.join("CSXS");
    if !csxs.exists() {
        return false; // not installed → not locked
    }
    let probe = csxs.join(".lock-probe");
    let locked = fs::write(&probe, b"").is_err();
    fs::remove_file(&probe).ok();
    locked
}

/// Sanity check: ensure we can write to the CEP extensions dir before downloading.
pub fn check_cep_dir_writable() -> Result<(), String> {
    let dir = paths::cep_extensions_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create CEP extensions dir at {:?}: {}", dir, e))?;
    let probe = dir.join(".pr-extension-manager-write-test");
    fs::write(&probe, b"").map_err(|e| format!("CEP extensions dir is not writable: {}", e))?;
    fs::remove_file(&probe).ok();
    Ok(())
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;
    use std::io::Write;

    fn make_zip(manifest_version: &str) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            w.start_file("CSXS/manifest.xml", opts).unwrap();
            write!(w, r#"<ExtensionManifest ExtensionBundleVersion="{}"/>"#, manifest_version).unwrap();
            w.start_file("client/index.html", opts).unwrap();
            w.write_all(b"<html></html>").unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn extract_then_read_version() {
        let base = tempfile::tempdir().unwrap();
        let dir = paths::install_dir_under(base.path(), "com.attract.genius-cut").unwrap();
        extract_zip_into(&dir, &make_zip("0.1.0")).unwrap();
        assert_eq!(read_version_at(&dir).as_deref(), Some("0.1.0"));
        assert!(dir.join("client/index.html").exists());
    }

    #[test]
    fn reinstall_replaces_previous_files() {
        let base = tempfile::tempdir().unwrap();
        let dir = paths::install_dir_under(base.path(), "com.attract.genius-cut").unwrap();
        extract_zip_into(&dir, &make_zip("0.1.0")).unwrap();
        std::fs::write(dir.join("stale.txt"), b"old").unwrap();
        extract_zip_into(&dir, &make_zip("0.2.0")).unwrap();
        assert_eq!(read_version_at(&dir).as_deref(), Some("0.2.0"));
        assert!(!dir.join("stale.txt").exists());
    }

    #[test]
    fn uninstalling_one_leaves_the_other_intact() {
        let base = tempfile::tempdir().unwrap();
        let pr = paths::install_dir_under(base.path(), "com.attract.pr-extension").unwrap();
        let gc = paths::install_dir_under(base.path(), "com.attract.genius-cut").unwrap();
        extract_zip_into(&pr, &make_zip("1.2.0")).unwrap();
        extract_zip_into(&gc, &make_zip("0.1.0")).unwrap();

        remove_dir(&gc).unwrap();

        assert!(!gc.exists());
        assert_eq!(read_version_at(&pr).as_deref(), Some("1.2.0"));
    }

    #[test]
    fn read_version_missing_dir_is_none() {
        let base = tempfile::tempdir().unwrap();
        assert!(read_version_at(&base.path().join("absent")).is_none());
    }

    #[test]
    fn public_ops_reject_unknown_ids_without_touching_disk() {
        assert!(uninstall("../..").is_err());
        assert!(uninstall("com.attract.nope").is_err());
        assert!(extract_zip_to_install_dir("..", &make_zip("9.9.9")).is_err());
        assert!(!is_installed("../.."));
        assert!(read_installed_version("com.attract.nope").is_none());
    }
}
