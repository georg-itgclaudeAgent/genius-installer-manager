//! Genius Cut's one-time runtime (embeddable Python + Whisper + NVIDIA libraries), installed
//! next to — not inside — the extension, so extension updates stay small.

use crate::registry::ExtensionSpec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const OWNER: &str = "georg-itgclaudeAgent";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flavour { Cuda, Cpu }

impl Flavour {
    pub fn as_str(&self) -> &'static str { match self { Flavour::Cuda => "cuda", Flavour::Cpu => "cpu" } }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Pointer { pub version: String, pub flavour: String, pub python: String }

/// NVIDIA's driver installs nvcuda.dll in System32; without it the GPU libraries are dead weight.
pub fn detect_flavour(system32: &Path) -> Flavour {
    if system32.join("nvcuda.dll").is_file() { Flavour::Cuda } else { Flavour::Cpu }
}

pub fn asset_name(f: Flavour, version: &str) -> String { format!("genius-cut-runtime-{}-{}.zip", f.as_str(), version) }

fn is_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

pub fn is_allowed_runtime_url(url: &str, spec: &ExtensionSpec) -> bool {
    let Some(rt) = &spec.runtime else { return false };
    let Ok(u) = reqwest::Url::parse(url) else { return false };
    if u.scheme() != "https" || u.host_str() != Some("github.com") || u.query().is_some() { return false; }
    let segs: Vec<&str> = match u.path_segments() { Some(s) => s.collect(), None => return false };
    let [owner, repo, "releases", "download", tag, file] = segs.as_slice() else { return false };
    let Some(ver) = tag.strip_prefix(rt.tag_prefix.as_str()) else { return false };
    let base = file.strip_suffix(".sha256").unwrap_or(file);
    *owner == OWNER && *repo == spec.repo && is_semver(ver)
        && (base == asset_name(Flavour::Cuda, ver) || base == asset_name(Flavour::Cpu, ver))
}

fn sha256_file(p: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", h.finalize()))
}

pub fn current(base: &Path) -> Option<Pointer> {
    let text = std::fs::read_to_string(base.join("runtime.json")).ok()?;
    let p: Pointer = serde_json::from_str(&text).ok()?;
    Path::new(&p.python).is_file().then_some(p)
}

/// Unpack every entry into `dest`. Entries using LZMA (zip method 14, what the runtime is built
/// with) are decoded here because the zip crate's own LZMA reader mis-parses the 4-byte zip
/// LZMA header that precedes the stream and fails on real archives. Everything else goes through
/// the zip crate. Entry names are checked with `enclosed_name`, so zip-slip entries are skipped.
fn extract_all(archive: &mut zip::ZipArchive<std::fs::File>, dest: &Path) -> Result<(), String> {
    for i in 0..archive.len() {
        let (name, is_dir, method, size) = {
            let f = archive.by_index_raw(i).map_err(|e| e.to_string())?;
            (f.enclosed_name(), f.is_dir(), f.compression(), f.size())
        };
        let Some(rel) = name else { continue };
        let out_path = dest.join(rel);
        if is_dir { std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?; continue; }
        if let Some(parent) = out_path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        let mut out = std::io::BufWriter::new(std::fs::File::create(&out_path).map_err(|e| e.to_string())?);
        if method == zip::CompressionMethod::Lzma {
            let mut raw = archive.by_index_raw(i).map_err(|e| e.to_string())?;
            let mut hdr = [0u8; 4]; // LZMA SDK version (2) + properties size (2, little endian)
            std::io::Read::read_exact(&mut raw, &mut hdr).map_err(|e| e.to_string())?;
            let props_len = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
            if props_len != 5 { return Err(format!("{}: unexpected LZMA properties", out_path.display())); }
            let mut props = [0u8; 5];
            std::io::Read::read_exact(&mut raw, &mut props).map_err(|e| e.to_string())?;
            // Rebuild the classic .lzma header (props + uncompressed size) the decoder expects.
            let mut head = props.to_vec();
            head.extend_from_slice(&size.to_le_bytes());
            let mut input = std::io::BufReader::new(std::io::Read::chain(std::io::Cursor::new(head), raw));
            lzma_rs::lzma_decompress(&mut input, &mut out).map_err(|e| format!("{}: {:?}", out_path.display(), e))?;
        } else {
            let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
        }
        std::io::Write::flush(&mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn install_zip(base: &Path, version: &str, flavour: Flavour, zip_path: &Path, expected_sha256: &str)
    -> Result<PathBuf, String> {
    if !is_semver(version) { return Err(format!("Invalid runtime version {:?}", version)); }
    let got = sha256_file(zip_path)?;
    if !got.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err("The runtime download failed its checksum, so nothing was installed. Try again.".into());
    }
    let root = base.join("runtime");
    let staging = root.join(format!(".staging-{}", version));
    let target = root.join(version);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Couldn't read the runtime: {}", e))?;
    extract_all(&mut archive, &staging).map_err(|e| { let _ = std::fs::remove_dir_all(&staging); format!("Couldn't unpack the runtime: {}", e) })?;
    let python = staging.join("python.exe");
    if !python.is_file() { let _ = std::fs::remove_dir_all(&staging); return Err("The runtime has no python.exe.".into()); }
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&staging, &target).map_err(|e| e.to_string())?;
    let pointer = Pointer { version: version.into(), flavour: flavour.as_str().into(),
                            python: target.join("python.exe").to_string_lossy().into() };
    let tmp = base.join("runtime.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&pointer).unwrap()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, base.join("runtime.json")).map_err(|e| e.to_string())?;
    // Best effort: an old version still in use (python.exe running) is cleaned up next time.
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            let p = e.path();
            if p != target && p.is_dir() { let _ = std::fs::remove_dir_all(&p); }
        }
    }
    Ok(target.join("python.exe"))
}

pub fn remove_all(base: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(base.join("runtime.json"));
    let root = base.join("runtime");
    if root.exists() { std::fs::remove_dir_all(&root).map_err(|e| format!("Couldn't remove the runtime: {}", e))?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Write;

    fn fake_runtime_zip(dir: &Path, name: &str) -> (PathBuf, String) {
        let p = dir.join(name);
        let f = std::fs::File::create(&p).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        w.start_file("python.exe", o).unwrap(); w.write_all(b"MZ fake").unwrap();
        w.start_file("Lib/site-packages/faster_whisper/__init__.py", o).unwrap(); w.write_all(b"").unwrap();
        w.finish().unwrap();
        let sha = format!("{:x}", Sha256::digest(std::fs::read(&p).unwrap()));
        (p, sha)
    }

    #[test]
    fn detects_cuda_only_when_the_nvidia_driver_dll_is_present() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(detect_flavour(d.path()), Flavour::Cpu);
        std::fs::write(d.path().join("nvcuda.dll"), b"").unwrap();
        assert_eq!(detect_flavour(d.path()), Flavour::Cuda);
    }

    #[test]
    fn asset_names_and_urls() {
        assert_eq!(asset_name(Flavour::Cuda, "1.0.0"), "genius-cut-runtime-cuda-1.0.0.zip");
        let spec = crate::registry::find("com.attract.genius-cut").unwrap();
        let ok = "https://github.com/georg-itgclaudeAgent/genius-cut/releases/download/runtime-v1.0.0/genius-cut-runtime-cuda-1.0.0.zip";
        assert!(is_allowed_runtime_url(ok, &spec));
        assert!(is_allowed_runtime_url(&format!("{}.sha256", ok), &spec));
        for bad in [
            "https://github.com/georg-itgclaudeAgent/genius-cut/releases/download/v1.0.0/genius-cut-runtime-cuda-1.0.0.zip",
            "https://github.com/georg-itgclaudeAgent/pr-extension/releases/download/runtime-v1.0.0/genius-cut-runtime-cuda-1.0.0.zip",
            "https://github.com/someone/genius-cut/releases/download/runtime-v1.0.0/genius-cut-runtime-cuda-1.0.0.zip",
            "https://github.com/georg-itgclaudeAgent/genius-cut/releases/download/runtime-v1.0.0/evil.exe",
        ] { assert!(!is_allowed_runtime_url(bad, &spec), "allowed {}", bad); }
    }

    #[test]
    fn installs_verifies_points_and_cleans_up_old_versions() {
        let base = tempfile::tempdir().unwrap();
        let (z1, s1) = fake_runtime_zip(base.path(), "a.zip");
        install_zip(base.path(), "1.0.0", Flavour::Cpu, &z1, &s1).unwrap();
        let (z2, s2) = fake_runtime_zip(base.path(), "b.zip");
        let py = install_zip(base.path(), "1.1.0", Flavour::Cpu, &z2, &s2).unwrap();
        assert!(py.ends_with("runtime/1.1.0/python.exe") || py.ends_with("runtime\\1.1.0\\python.exe"));
        let p = current(base.path()).unwrap();
        assert_eq!((p.version.as_str(), p.flavour.as_str()), ("1.1.0", "cpu"));
        assert!(!base.path().join("runtime/1.0.0").exists(), "old version kept");
    }

    #[test]
    fn a_checksum_mismatch_is_refused_and_changes_nothing() {
        // Review Focus 2.
        let base = tempfile::tempdir().unwrap();
        let (z, _) = fake_runtime_zip(base.path(), "a.zip");
        let err = install_zip(base.path(), "1.0.0", Flavour::Cpu, &z, "00").unwrap_err();
        assert!(err.contains("checksum"));
        assert!(current(base.path()).is_none());
        assert!(!base.path().join("runtime/1.0.0").exists());
    }

    #[test]
    fn a_leftover_half_extracted_staging_folder_is_replaced() {
        // Review Focus 1: an earlier attempt died mid-extract.
        let base = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(base.path().join("runtime/.staging-1.0.0/junk")).unwrap();
        let (z, s) = fake_runtime_zip(base.path(), "a.zip");
        install_zip(base.path(), "1.0.0", Flavour::Cpu, &z, &s).unwrap();
        assert!(!base.path().join("runtime/1.0.0/junk").exists());
        assert!(!base.path().join("runtime/.staging-1.0.0").exists());
    }

    #[test]
    fn an_old_version_that_cannot_be_deleted_does_not_fail_the_upgrade() {
        // Review Focus 3: the old python.exe is still running. Simulated with a read-only file
        // the cleanup can't remove; the pointer must still move to the new version.
        let base = tempfile::tempdir().unwrap();
        let (z1, s1) = fake_runtime_zip(base.path(), "a.zip");
        install_zip(base.path(), "1.0.0", Flavour::Cpu, &z1, &s1).unwrap();
        let locked = base.path().join("runtime/1.0.0/python.exe");
        let mut perm = std::fs::metadata(&locked).unwrap().permissions(); perm.set_readonly(true);
        std::fs::set_permissions(&locked, perm).unwrap();
        let (z2, s2) = fake_runtime_zip(base.path(), "b.zip");
        assert!(install_zip(base.path(), "1.1.0", Flavour::Cpu, &z2, &s2).is_ok());
        assert_eq!(current(base.path()).unwrap().version, "1.1.0");
    }

    #[test]
    fn remove_all_deletes_runtime_and_pointer() {
        let base = tempfile::tempdir().unwrap();
        let (z, s) = fake_runtime_zip(base.path(), "a.zip");
        install_zip(base.path(), "1.0.0", Flavour::Cpu, &z, &s).unwrap();
        remove_all(base.path()).unwrap();
        assert!(current(base.path()).is_none() && !base.path().join("runtime").exists());
    }

    #[test]
    fn extracts_a_real_lzma_compressed_runtime_zip() {
        // The shipped runtime is built with Python's zipfile.ZIP_LZMA (the zip crate can only
        // read LZMA, not write it), so the fixture is a tiny zip generated that way.
        let base = tempfile::tempdir().unwrap();
        let z = base.path().join("lzma.zip");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/lzma-runtime.zip"), &z).unwrap();
        let sha = format!("{:x}", Sha256::digest(std::fs::read(&z).unwrap()));
        let py = install_zip(base.path(), "1.0.0", Flavour::Cuda, &z, &sha).unwrap();
        assert!(py.is_file());
        assert_eq!(std::fs::read(&py).unwrap(), b"MZ fake lzma ".repeat(50));
        
        assert!(base.path().join("runtime/1.0.0/Lib/site-packages/faster_whisper/data.txt").is_file());
        assert_eq!(current(base.path()).unwrap().flavour, "cuda");
    }

    /// Manual benchmark: `GC_RUNTIME_BENCH_ZIP=<big lzma zip> cargo test --release -- --ignored --nocapture bench`
    #[test]
    #[ignore]
    fn bench_install_of_a_large_lzma_zip() {
        let Ok(zp) = std::env::var("GC_RUNTIME_BENCH_ZIP") else { eprintln!("GC_RUNTIME_BENCH_ZIP unset, skipping"); return };
        let zp = PathBuf::from(zp);
        let base = tempfile::tempdir().unwrap();
        let sha = sha256_file(&zp).unwrap();
        let t = std::time::Instant::now();
        install_zip(base.path(), "1.0.0", Flavour::Cuda, &zp, &sha).unwrap();
        eprintln!("install_zip took {:.1}s", t.elapsed().as_secs_f64());
    }
}
