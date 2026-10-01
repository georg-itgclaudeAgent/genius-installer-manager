//! Genius Cut's one-time runtime (embeddable Python + Whisper + NVIDIA libraries), installed
//! next to — not inside — the extension, so extension updates stay small.

use crate::registry::{ExtensionSpec, RuntimeSpec};
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

pub fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 { return None; }
    let n = |p: &str| if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) { p.parse::<u64>().ok() } else { None };
    Some((n(parts[0])?, n(parts[1])?, n(parts[2])?))
}

pub fn is_semver(s: &str) -> bool { parse_semver(s).is_some() }

/// Only extension builds from `from_version` on have a backend that uses the runtime (the
/// 0.1.0 preview runs on sample data). An unknown or unparsable version doesn't need it.
pub fn needed(rt: &RuntimeSpec, extension_version: Option<&str>) -> bool {
    match (extension_version.and_then(parse_semver), parse_semver(&rt.from_version)) {
        (Some(v), Some(from)) => v >= from,
        _ => false,
    }
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

/// Passes writes through while computing their CRC32, so the custom LZMA path below gets the
/// same integrity check the zip crate applies to its own readers.
struct CrcWriter<W: std::io::Write> { inner: W, hasher: crc32fast::Hasher }

impl<W: std::io::Write> CrcWriter<W> {
    fn new(inner: W) -> Self { CrcWriter { inner, hasher: crc32fast::Hasher::new() } }
    /// Flush and compare against the CRC32 recorded in the zip entry.
    fn finish(mut self, expected: u32) -> Result<W, String> {
        std::io::Write::flush(&mut self.inner).map_err(|e| e.to_string())?;
        let got = self.hasher.finalize();
        if got != expected { return Err(format!("CRC mismatch (expected {:08x}, got {:08x})", expected, got)); }
        Ok(self.inner)
    }
}

impl<W: std::io::Write> std::io::Write for CrcWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> { self.inner.flush() }
}

/// Unpack every entry into `dest`. Entries using LZMA (zip method 14, what the runtime is built
/// with) are decoded here because the zip crate's own LZMA reader mis-parses the 4-byte zip
/// LZMA header that precedes the stream and fails on real archives. Everything else goes through
/// the zip crate. Entry names are checked with `enclosed_name`, so zip-slip entries are skipped.
fn extract_all(archive: &mut zip::ZipArchive<std::fs::File>, dest: &Path) -> Result<(), String> {
    for i in 0..archive.len() {
        let (name, is_dir, method, size, crc) = {
            let f = archive.by_index_raw(i).map_err(|e| e.to_string())?;
            (f.enclosed_name(), f.is_dir(), f.compression(), f.size(), f.crc32())
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
            let mut checked = CrcWriter::new(&mut out);
            lzma_rs::lzma_decompress(&mut input, &mut checked).map_err(|e| format!("{}: {:?}", out_path.display(), e))?;
            checked.finish(crc).map_err(|e| format!("{}: {}", out_path.display(), e))?;
        } else {
            let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
        }
        std::io::Write::flush(&mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Bases with an install in progress; a sweep skips them so it never retires a version that
/// is about to become current.
static BUSY: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

struct BusyGuard(PathBuf);

impl BusyGuard {
    fn try_claim(base: &Path) -> Option<BusyGuard> {
        let mut busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        if busy.iter().any(|b| b == base) { return None; }
        busy.push(base.to_path_buf());
        Some(BusyGuard(base.to_path_buf()))
    }
    fn claim(base: &Path) -> BusyGuard {
        loop {
            if let Some(g) = BusyGuard::try_claim(base) { return g; }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        let mut busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        busy.retain(|b| b != &self.0);
    }
}

/// The version runtime.json names, whether or not its python.exe still exists.
fn pointed_version(base: &Path) -> Option<String> {
    let text = std::fs::read_to_string(base.join("runtime.json")).ok()?;
    serde_json::from_str::<Pointer>(&text).ok().map(|p| p.version)
}

/// Move a version folder aside, then delete it. Renaming a folder that holds an open file
/// (a running python.exe) fails cleanly on Windows, so an in-use runtime is left whole rather
/// than half-deleted under the running backend; a later sweep retries it.
fn retire(root: &Path, dir: &Path) -> bool {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let trash = root.join(format!(".trash-{}", name));
    let _ = std::fs::remove_dir_all(&trash);
    if std::fs::rename(dir, &trash).is_err() { return false; }
    let _ = std::fs::remove_dir_all(&trash);
    true
}

fn sweep_claimed(base: &Path) {
    let root = base.join("runtime");
    let keep = pointed_version(base);
    let Ok(entries) = std::fs::read_dir(&root) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() { continue; }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with(".trash-") {
            let _ = std::fs::remove_dir_all(&p);
        } else if !name.starts_with(".staging-") && Some(&name) != keep.as_ref() {
            retire(&root, &p);
        }
    }
}

/// Best-effort cleanup of old versions and leftover `.trash-*` folders. Never touches the
/// version runtime.json points at, and skips a base while an install is running there.
pub fn sweep_old(base: &Path) {
    if let Some(_g) = BusyGuard::try_claim(base) { sweep_claimed(base); }
}

pub fn install_zip(base: &Path, version: &str, flavour: Flavour, zip_path: &Path, expected_sha256: &str)
    -> Result<PathBuf, String> {
    if !is_semver(version) { return Err(format!("Invalid runtime version {:?}", version)); }
    let _busy = BusyGuard::claim(base);
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
    if target.exists() && !retire(&root, &target) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Runtime {} is in use. Close Premiere and try again.", version));
    }
    std::fs::rename(&staging, &target).map_err(|e| e.to_string())?;
    let pointer = Pointer { version: version.into(), flavour: flavour.as_str().into(),
                            python: target.join("python.exe").to_string_lossy().into() };
    let tmp = base.join("runtime.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&pointer).unwrap()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, base.join("runtime.json")).map_err(|e| e.to_string())?;
    // Best effort: an old version still in use (python.exe running) stays whole and is
    // cleaned up by a later sweep.
    sweep_claimed(base);
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
    #[cfg(windows)]
    fn an_old_version_in_use_is_left_intact_then_swept_once_released() {
        // Review Focus 3 / I2: the old python.exe is still running. A real lock (no
        // FILE_SHARE_DELETE) is what a running backend holds; the pointer must move, and the
        // old folder must stay whole (not half-deleted under the running process).
        use std::os::windows::fs::OpenOptionsExt;
        let base = tempfile::tempdir().unwrap();
        let (z1, s1) = fake_runtime_zip(base.path(), "a.zip");
        install_zip(base.path(), "1.0.0", Flavour::Cpu, &z1, &s1).unwrap();
        let old = base.path().join("runtime/1.0.0");
        let sibling = old.join("Lib/site-packages/faster_whisper/__init__.py");
        let lock = std::fs::OpenOptions::new().read(true).share_mode(1 /* FILE_SHARE_READ */)
            .open(old.join("python.exe")).unwrap();
        let (z2, s2) = fake_runtime_zip(base.path(), "b.zip");
        assert!(install_zip(base.path(), "1.1.0", Flavour::Cpu, &z2, &s2).is_ok());
        assert_eq!(current(base.path()).unwrap().version, "1.1.0");
        assert!(old.join("python.exe").is_file() && sibling.is_file(), "old runtime was half-deleted");
        drop(lock);
        sweep_old(base.path());
        assert!(!old.exists(), "old version not swept after release");
        assert!(base.path().join("runtime/1.1.0/python.exe").is_file());
    }

    #[test]
    fn sweep_removes_trash_and_stale_versions_but_never_the_current_one() {
        let base = tempfile::tempdir().unwrap();
        let (z, s) = fake_runtime_zip(base.path(), "a.zip");
        install_zip(base.path(), "1.0.0", Flavour::Cpu, &z, &s).unwrap();
        let root = base.path().join("runtime");
        std::fs::create_dir_all(root.join(".trash-0.9.0/x")).unwrap();
        std::fs::create_dir_all(root.join("0.9.0/Lib")).unwrap();
        std::fs::create_dir_all(root.join(".staging-1.1.0")).unwrap();
        sweep_old(base.path());
        assert!(!root.join(".trash-0.9.0").exists() && !root.join("0.9.0").exists());
        assert!(root.join("1.0.0/python.exe").is_file(), "swept the current version");
        assert!(root.join(".staging-1.1.0").exists(), "swept an install in progress");
        // A stale pointer (python.exe gone) still protects the version it names.
        std::fs::remove_file(root.join("1.0.0/python.exe")).unwrap();
        sweep_old(base.path());
        assert!(root.join("1.0.0").exists());
    }

    #[test]
    fn crc_writer_passes_data_through_and_checks_the_entry_crc() {
        use std::io::Write as _;
        let data = b"MZ fake lzma payload".repeat(10);
        let mut w = CrcWriter::new(Vec::new());
        w.write_all(&data).unwrap();
        assert_eq!(w.finish(crc32fast::hash(&data)).unwrap(), data);
        let mut w = CrcWriter::new(Vec::new());
        w.write_all(&data).unwrap();
        assert!(w.finish(crc32fast::hash(&data) ^ 1).unwrap_err().contains("CRC mismatch"));
    }

    #[test]
    fn runtime_is_needed_only_from_its_first_extension_version() {
        let rt = RuntimeSpec { tag_prefix: "runtime-v".into(), from_version: "0.2.0".into() };
        assert!(!needed(&rt, Some("0.1.0")), "the 0.1.0 preview has no backend");
        assert!(needed(&rt, Some("0.2.0")) && needed(&rt, Some("0.10.0")) && needed(&rt, Some("1.0.0")));
        assert!(!needed(&rt, None) && !needed(&rt, Some("junk")));
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
