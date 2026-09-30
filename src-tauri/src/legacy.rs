//! Before the rename, the app installed to `%LOCALAPPDATA%/PR Extension Manager`.
//! Tauri's NSIS installer keys the install folder and the Apps-list entry on the
//! display name, so updating to "Genius Installer Manager" installs a second copy.
//! On first start the new app silently runs the old copy's uninstaller.

use std::path::{Path, PathBuf};

pub const LEGACY_PRODUCT_DIR: &str = "PR Extension Manager";

/// The old install's uninstaller, if one exists and we are not running from it.
pub fn legacy_uninstaller(local_app_data: &Path, current_exe: &Path) -> Option<PathBuf> {
    let dir = local_app_data.join(LEGACY_PRODUCT_DIR);
    let uninstaller = dir.join("uninstall.exe");
    if !uninstaller.is_file() || current_exe.starts_with(&dir) {
        return None;
    }
    Some(uninstaller)
}

/// Release builds only — a `tauri dev` run must never uninstall the developer's real copy.
pub fn remove_legacy_install() {
    if cfg!(debug_assertions) || !cfg!(windows) {
        return;
    }
    let (Ok(lad), Ok(exe)) = (std::env::var("LOCALAPPDATA"), std::env::current_exe()) else { return };
    if let Some(u) = legacy_uninstaller(Path::new(&lad), &exe) {
        // Silent NSIS uninstall: removes the old app, its shortcut and its Apps entry,
        // never the CEP extensions it installed.
        let _ = std::process::Command::new(u).arg("/S").spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_install(base: &Path) -> PathBuf {
        let dir = base.join(LEGACY_PRODUCT_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("uninstall.exe"), b"").unwrap();
        dir
    }

    #[test]
    fn finds_the_old_uninstaller_when_running_from_the_new_install() {
        let lad = tempfile::tempdir().unwrap();
        let old = legacy_install(lad.path());
        let exe = lad.path().join("Genius Installer Manager").join("genius-installer-manager.exe");
        assert_eq!(legacy_uninstaller(lad.path(), &exe), Some(old.join("uninstall.exe")));
    }

    #[test]
    fn never_uninstalls_the_folder_it_is_running_from() {
        let lad = tempfile::tempdir().unwrap();
        let old = legacy_install(lad.path());
        let exe = old.join("pr-extension-manager.exe");
        assert_eq!(legacy_uninstaller(lad.path(), &exe), None);
    }

    #[test]
    fn nothing_to_do_on_a_fresh_machine() {
        let lad = tempfile::tempdir().unwrap();
        let exe = lad.path().join("Genius Installer Manager").join("genius-installer-manager.exe");
        assert_eq!(legacy_uninstaller(lad.path(), &exe), None);
    }
}
