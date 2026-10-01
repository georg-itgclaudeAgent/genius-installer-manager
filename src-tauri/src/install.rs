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

/// Extract `zip_bytes` into `dir`, which must not exist yet.
fn extract_zip_into(dir: &Path, zip_bytes: &[u8]) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("Failed to read zip: {}", e))?;
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create install dir: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read zip entry {}: {}", i, e))?;
        let outpath = match entry.enclosed_name() {
            Some(p) => dir.join(p),
            None => continue, // skip suspicious entries (zip-slip)
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

fn read_bundle_id_at(dir: &Path) -> Option<String> {
    let content = fs::read_to_string(dir.join("CSXS").join("manifest.xml")).ok()?;
    let needle = "ExtensionBundleId=\"";
    let start = content.find(needle)? + needle.len();
    let end_offset = content[start..].find('"')?;
    Some(content[start..start + end_offset].to_string())
}

/// Install a release zip for `id` under `base` without ever leaving a
/// half-written or half-deleted extension behind:
/// 1. extract into `.<id>.staging` and check its manifest names this bundle id;
/// 2. rename the live folder to `.<id>.old` — if Windows refuses (Premiere holds
///    the files open) we stop here and the old install is untouched;
/// 3. rename staging into place, then delete `.old` best-effort.
pub(crate) fn install_zip_under(base: &Path, id: &str, zip_bytes: &[u8]) -> Result<(), String> {
    let target = paths::install_dir_under(base, id)?;
    let staging = base.join(format!(".{}.staging", id));
    let old = base.join(format!(".{}.old", id));
    let _ = remove_dir(&staging);
    let _ = remove_dir(&old);

    let staged = extract_zip_into(&staging, zip_bytes).and_then(|_| {
        match read_bundle_id_at(&staging) {
            Some(b) if b == id => Ok(()),
            Some(b) => Err(format!("This download is for {}, not {}. Nothing was changed.", b, id)),
            None => Err("The download has no readable CSXS/manifest.xml. Nothing was changed.".to_string()),
        }
    });
    if let Err(e) = staged {
        let _ = remove_dir(&staging);
        return Err(e);
    }

    if target.exists() {
        if let Err(e) = fs::rename(&target, &old) {
            let _ = remove_dir(&staging);
            return Err(format!(
                "Couldn't replace the installed files ({}). Close Premiere Pro and try again. Nothing was changed.", e));
        }
    }
    if let Err(e) = fs::rename(&staging, &target) {
        // Put the previous install back so the user is never left with nothing.
        if old.exists() { let _ = fs::rename(&old, &target); }
        let _ = remove_dir(&staging);
        return Err(format!("Failed to move the new version into place: {}", e));
    }
    let _ = remove_dir(&old);
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
    install_zip_under(&paths::cep_extensions_dir(), id, zip_bytes)
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
    let probe = dir.join(".genius-installer-manager-write-test");
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

    const GC: &str = "com.attract.genius-cut";
    const PR: &str = "com.attract.pr-extension";

    fn make_zip(bundle_id: &str, manifest_version: &str) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            w.start_file("CSXS/manifest.xml", opts).unwrap();
            write!(w, r#"<ExtensionManifest ExtensionBundleId="{}" ExtensionBundleVersion="{}"/>"#,
                bundle_id, manifest_version).unwrap();
            w.start_file("client/index.html", opts).unwrap();
            w.write_all(b"<html></html>").unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    /// Zip whose central directory is intact but whose last entry fails its CRC,
    /// so it parses fine and only fails part-way through extraction.
    fn make_corrupt_zip(bundle_id: &str) -> Vec<u8> {
        let mut z = make_zip(bundle_id, "9.9.9");
        let at = z.windows(13).position(|w| w == b"<html></html>").unwrap();
        z[at] = b'X';
        z
    }

    fn leftovers(base: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(base).unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".staging") || n.contains(".old"))
            .collect()
    }

    #[test]
    fn install_then_read_version() {
        let base = tempfile::tempdir().unwrap();
        install_zip_under(base.path(), GC, &make_zip(GC, "0.1.0")).unwrap();
        let dir = paths::install_dir_under(base.path(), GC).unwrap();
        assert_eq!(read_version_at(&dir).as_deref(), Some("0.1.0"));
        assert!(dir.join("client/index.html").exists());
        assert!(leftovers(base.path()).is_empty());
    }

    #[test]
    fn reinstall_replaces_previous_files() {
        let base = tempfile::tempdir().unwrap();
        install_zip_under(base.path(), GC, &make_zip(GC, "0.1.0")).unwrap();
        let dir = paths::install_dir_under(base.path(), GC).unwrap();
        std::fs::write(dir.join("stale.txt"), b"old").unwrap();
        install_zip_under(base.path(), GC, &make_zip(GC, "0.2.0")).unwrap();
        assert_eq!(read_version_at(&dir).as_deref(), Some("0.2.0"));
        assert!(!dir.join("stale.txt").exists());
        assert!(leftovers(base.path()).is_empty());
    }

    #[test]
    fn failed_extraction_leaves_the_old_install_untouched() {
        let base = tempfile::tempdir().unwrap();
        install_zip_under(base.path(), GC, &make_zip(GC, "0.1.0")).unwrap();
        let dir = paths::install_dir_under(base.path(), GC).unwrap();

        assert!(install_zip_under(base.path(), GC, &make_corrupt_zip(GC)).is_err());

        assert_eq!(read_version_at(&dir).as_deref(), Some("0.1.0"));
        assert!(dir.join("client/index.html").exists());
        assert!(leftovers(base.path()).is_empty());
    }

    #[test]
    fn refuses_a_zip_built_for_a_different_extension() {
        let base = tempfile::tempdir().unwrap();
        install_zip_under(base.path(), PR, &make_zip(PR, "1.2.0")).unwrap();

        let err = install_zip_under(base.path(), PR, &make_zip(GC, "0.1.0")).unwrap_err();

        assert!(err.contains("com.attract.genius-cut"), "{}", err);
        let pr = paths::install_dir_under(base.path(), PR).unwrap();
        assert_eq!(read_version_at(&pr).as_deref(), Some("1.2.0"));
        assert!(leftovers(base.path()).is_empty());
    }

    #[test]
    fn uninstalling_one_leaves_the_other_intact() {
        let base = tempfile::tempdir().unwrap();
        install_zip_under(base.path(), PR, &make_zip(PR, "1.2.0")).unwrap();
        install_zip_under(base.path(), GC, &make_zip(GC, "0.1.0")).unwrap();
        let pr = paths::install_dir_under(base.path(), PR).unwrap();
        let gc = paths::install_dir_under(base.path(), GC).unwrap();

        remove_dir(&gc).unwrap();

        assert!(!gc.exists());
        assert_eq!(read_version_at(&pr).as_deref(), Some("1.2.0"));
    }

    /// Manual check of a real release: GENIUSCUT_RELEASE_ZIP=<path> cargo test -- --ignored real_release
    #[test]
    #[ignore]
    fn real_release_zip_installs_through_the_same_path_as_the_app() {
        let zip = std::env::var("GENIUSCUT_RELEASE_ZIP").expect("set GENIUSCUT_RELEASE_ZIP");
        let id = std::env::var("GENIUSCUT_RELEASE_ID").unwrap_or_else(|_| GC.to_string());
        let bytes = std::fs::read(&zip).unwrap();
        let base = tempfile::tempdir().unwrap();
        install_zip_under(base.path(), &id, &bytes).expect("the installer must accept this release");
        let dir = paths::install_dir_under(base.path(), &id).unwrap();
        println!("installed version {:?}", read_version_at(&dir));
        assert!(dir.join("CSXS/manifest.xml").exists() && dir.join("client/dist/index.html").exists());
        assert!(leftovers(base.path()).is_empty());
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
        assert!(extract_zip_to_install_dir("..", &make_zip(GC, "9.9.9")).is_err());
        assert!(!is_installed("../.."));
        assert!(read_installed_version("com.attract.nope").is_none());
    }
}
