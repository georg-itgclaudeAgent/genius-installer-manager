use crate::registry::ExtensionSpec;
use reqwest::Url;

const OWNER: &str = "georg-itgclaudeAgent";

/// A download is allowed only if, after WHATWG normalisation (which collapses
/// `..` and `%2e%2e`), it is exactly
/// `https://github.com/<OWNER>/<spec.repo>/releases/download/<tag>/<file>.zip`
/// with `<tag>` on this extension's own channel. That ties the URL to the id,
/// so one extension's zip can never be extracted into another's folder.
pub fn is_allowed_download_url(url: &str, spec: &ExtensionSpec) -> bool {
    let Ok(u) = Url::parse(url) else { return false };
    if u.scheme() != "https" || u.host_str() != Some("github.com") || u.port().is_some() {
        return false;
    }
    if !u.username().is_empty() || u.password().is_some() || u.query().is_some() || u.fragment().is_some() {
        return false;
    }
    let Some(segs) = u.path_segments() else { return false };
    let segs: Vec<&str> = segs.collect();
    matches!(segs.as_slice(),
        [owner, repo, "releases", "download", tag, file]
            if *owner == OWNER
            && *repo == spec.repo.as_str()
            && tag.starts_with(spec.tag_prefix.as_str())
            && file.ends_with(".zip"))
}

/// GitHub serves release assets by redirecting from github.com to a
/// *.githubusercontent.com CDN host. Any other hop is refused.
pub fn is_allowed_redirect(url: &Url) -> bool {
    url.scheme() == "https"
        && match url.host_str() {
            Some("github.com") => true,
            Some(h) => h.ends_with(".githubusercontent.com"),
            None => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;

    fn gc() -> registry::ExtensionSpec { registry::find("com.attract.genius-cut").unwrap() }
    fn pr() -> registry::ExtensionSpec { registry::find("com.attract.pr-extension").unwrap() }

    const GC_ZIP: &str =
        "https://github.com/georg-itgclaudeAgent/genius-cut/releases/download/v0.1.0/genius-cut-0.1.0.zip";

    #[test]
    fn allows_the_published_genius_cut_0_1_0_asset() {
        assert!(is_allowed_download_url(
            "https://github.com/georg-itgclaudeAgent/genius-cut/releases/download/v0.1.0/genius-cut-0.1.0.zip", &gc()));
    }

    #[test]
    fn refuses_genius_cut_zip_from_the_wrong_repo() {
        assert!(!is_allowed_download_url(
            "https://github.com/georg-itgclaudeAgent/pr-extension/releases/download/v0.1.0/genius-cut-0.1.0.zip",
            &gc()));
    }

    #[test]
    fn allows_this_extensions_own_release_asset() {
        assert!(is_allowed_download_url(GC_ZIP, &gc()));
        assert!(is_allowed_download_url(
            "https://github.com/georg-itgclaudeAgent/pr-extension/releases/download/extension-v1.2.0/pr-extension-1.2.0.zip",
            &pr()));
    }

    #[test]
    fn refuses_another_extensions_release() {
        // Genius Cut zip must never be extracted into the PR Extension folder.
        assert!(!is_allowed_download_url(GC_ZIP, &pr()));
    }

    #[test]
    fn refuses_dot_segment_and_encoded_traversal() {
        for bad in [
            "https://github.com/georg-itgclaudeAgent/../someone-else/genius-cut/releases/download/v0.1.0/x.zip",
            "https://github.com/georg-itgclaudeAgent/%2e%2e/evil/genius-cut/releases/download/v0.1.0/x.zip",
            "https://github.com/georg-itgclaudeAgent/genius-cut/releases/download/v0.1.0/../../../../evil/x.zip",
        ] {
            assert!(!is_allowed_download_url(bad, &gc()), "allowed {:?}", bad);
        }
    }

    #[test]
    fn refuses_everything_else() {
        for bad in [
            "http://github.com/georg-itgclaudeAgent/genius-cut/releases/download/v0.1.0/x.zip",
            "https://github.com.evil.example/georg-itgclaudeAgent/genius-cut/releases/download/v0.1.0/x.zip",
            "https://evil.example/github.com/georg-itgclaudeAgent/genius-cut/releases/download/v0.1.0/x.zip",
            "https://github.com/someone-else/genius-cut/releases/download/v0.1.0/x.zip",
            "https://github.com/georg-itgclaudeAgent/other-repo/releases/download/v0.1.0/x.zip",
            "https://github.com/georg-itgclaudeAgent/genius-cut/releases/download/v0.1.0/x.exe",
            "https://objects.githubusercontent.com/github-production-release-asset-2e65be/123/abc",
            "file:///C:/Windows/system32",
            "",
        ] {
            assert!(!is_allowed_download_url(bad, &gc()), "allowed {:?}", bad);
        }
    }

    #[test]
    fn redirects_only_to_github_hosts_over_https() {
        let ok = |s: &str| is_allowed_redirect(&reqwest::Url::parse(s).unwrap());
        assert!(ok("https://release-assets.githubusercontent.com/github-production-release-asset/1/2"));
        assert!(ok("https://objects.githubusercontent.com/github-production-release-asset-2e65be/1"));
        assert!(ok("https://github.com/georg-itgclaudeAgent/pr-extension/releases/download/x/y.zip"));
        assert!(!ok("http://release-assets.githubusercontent.com/x"));
        assert!(!ok("https://githubusercontent.com.evil.example/x"));
        assert!(!ok("https://evil.example/x"));
    }
}
