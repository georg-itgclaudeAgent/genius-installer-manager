use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Copy, Debug)]
pub struct ExtensionSpec {
    /// Bundle id; also the install folder name. Never change an existing one.
    pub id: &'static str,
    pub name: &'static str,
    pub subtitle: &'static str,
    /// Two-letter tile shown on the card.
    pub icon: &'static str,
    /// GitHub repo under georg-itgclaudeAgent.
    pub repo: &'static str,
    pub tag_prefix: &'static str,
}

pub const EXTENSIONS: &[ExtensionSpec] = &[
    ExtensionSpec {
        id: "com.attract.pr-extension",
        name: "PR Extension",
        subtitle: "For Adobe Premiere Pro · ElevenLabs + HeyGen + Assets",
        icon: "PR",
        repo: "pr-extension",
        tag_prefix: "extension-v",
    },
    ExtensionSpec {
        id: "com.attract.genius-cut",
        name: "Genius Cut",
        subtitle: "For Adobe Premiere Pro · Transcript-driven trimming",
        icon: "GC",
        repo: "pr-extension",
        tag_prefix: "geniuscut-v",
    },
];

pub fn find(id: &str) -> Option<&'static ExtensionSpec> {
    EXTENSIONS.iter().find(|e| e.id == id)
}

/// Returns `%APPDATA%/Adobe/CEP/extensions/`
pub fn cep_extensions_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").expect("APPDATA environment variable not set");
    PathBuf::from(appdata).join("Adobe").join("CEP").join("extensions")
}

/// Resolve `<base>/<id>` — only for ids in the registry. Because only registry
/// ids are accepted, traversal strings can never reach the filesystem.
pub fn install_dir_under(base: &Path, id: &str) -> Result<PathBuf, String> {
    let spec = find(id).ok_or_else(|| format!("Unknown extension id: {:?}", id))?;
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
    fn registry_contains_both_extensions_with_exact_ids() {
        let ids: Vec<&str> = EXTENSIONS.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec!["com.attract.pr-extension", "com.attract.genius-cut"]);
    }

    #[test]
    fn tag_prefixes_are_distinct_and_not_prefixes_of_each_other() {
        for a in EXTENSIONS {
            for b in EXTENSIONS {
                if a.id != b.id {
                    assert!(!a.tag_prefix.starts_with(b.tag_prefix),
                        "{} bleeds into {}", a.tag_prefix, b.tag_prefix);
                }
            }
        }
    }

    #[test]
    fn find_known_and_unknown() {
        assert_eq!(find("com.attract.genius-cut").unwrap().name, "Genius Cut");
        assert!(find("com.attract.nope").is_none());
    }

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
