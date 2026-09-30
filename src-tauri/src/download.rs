/// Only GitHub release assets from our own account may be installed.
/// `browser_download_url` is github.com/<owner>/...; GitHub then 302s to
/// objects.githubusercontent.com, which reqwest follows.
pub fn is_allowed_download_url(url: &str) -> bool {
    const OWNER_PREFIX: &str = "https://github.com/georg-itgclaudeAgent/";
    const CDN_PREFIX: &str = "https://objects.githubusercontent.com/";
    url.starts_with(OWNER_PREFIX) || url.starts_with(CDN_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_github_release_assets() {
        assert!(is_allowed_download_url(
            "https://github.com/georg-itgclaudeAgent/pr-extension/releases/download/geniuscut-v0.1.0/genius-cut.zip"));
        assert!(is_allowed_download_url(
            "https://objects.githubusercontent.com/github-production-release-asset-2e65be/123/abc"));
    }

    #[test]
    fn refuses_everything_else() {
        for bad in [
            "http://github.com/georg-itgclaudeAgent/pr-extension/releases/download/x.zip", // not https
            "https://github.com.evil.example/x.zip",
            "https://evil.example/github.com/x.zip",
            "https://github.com/someone-else/pr-extension/releases/download/x.zip",
            "file:///C:/Windows/system32",
            "",
        ] {
            assert!(!is_allowed_download_url(bad), "allowed {:?}", bad);
        }
    }
}
