use crate::registry;
use std::path::{Path, PathBuf};

/// Returns `%APPDATA%/Adobe/CEP/extensions/`
pub fn cep_extensions_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").expect("APPDATA environment variable not set");
    PathBuf::from(appdata).join("Adobe").join("CEP").join("extensions")
}

/// Resolve `<base>/<id>` — only for ids in the registry. Because only registry
/// ids are accepted, traversal strings can never reach the filesystem.
pub fn install_dir_under(base: &Path, id: &str) -> Result<PathBuf, String> {
    let spec = registry::find(id).ok_or_else(|| format!("Unknown extension id: {:?}", id))?;
    Ok(base.join(spec.id))
}

pub fn install_dir(id: &str) -> Result<PathBuf, String> {
    install_dir_under(&cep_extensions_dir(), id)
}

pub fn manifest_path(id: &str) -> Result<PathBuf, String> {
    Ok(install_dir(id)?.join("CSXS").join("manifest.xml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn install_dir_under_joins_the_id() {
        let d = install_dir_under(Path::new("C:/base"), "com.attract.genius-cut").unwrap();
        assert_eq!(d, Path::new("C:/base").join("com.attract.genius-cut"));
    }

    #[test]
    fn install_dir_rejects_traversal_and_unknown_ids() {
        for bad in ["", "..", "../..", "com.attract.pr-extension/../x", "com.attract.nope"] {
            assert!(install_dir_under(Path::new("C:/base"), bad).is_err(), "accepted {:?}", bad);
        }
    }
}
