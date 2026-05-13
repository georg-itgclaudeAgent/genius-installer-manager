use std::path::PathBuf;

pub const EXTENSION_ID: &str = "com.attract.pr-extension";

/// Returns `%APPDATA%/Adobe/CEP/extensions/`
pub fn cep_extensions_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").expect("APPDATA environment variable not set");
    PathBuf::from(appdata)
        .join("Adobe")
        .join("CEP")
        .join("extensions")
}

/// Returns `%APPDATA%/Adobe/CEP/extensions/com.attract.pr-extension/`
pub fn extension_install_dir() -> PathBuf {
    cep_extensions_dir().join(EXTENSION_ID)
}

/// Path to the installed extension's manifest.xml.
pub fn installed_manifest_path() -> PathBuf {
    extension_install_dir().join("CSXS").join("manifest.xml")
}
