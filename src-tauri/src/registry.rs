//! The list of extensions the installer offers.
//!
//! It lives on GitHub as `registry.json` in this repo, so adding an app doesn't need a new
//! installer release: the app fetches it on launch and on "Check for updates". If GitHub
//! can't be reached it uses the last good copy, and failing that the built-in list below.
//!
//! Because the list comes from the network, every entry is validated, and for the
//! built-in extensions the repo and tag prefix can't be changed remotely: an edited list
//! can add apps or rename them, never redirect an existing app's downloads.

use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

pub const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/georg-itgclaudeAgent/genius-installer-manager/main/registry.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExtensionSpec {
    /// Bundle id; also the install folder name. Never change an existing one.
    pub id: String,
    pub name: String,
    pub subtitle: String,
    /// Up to three letters shown on the card.
    pub icon: String,
    /// GitHub repo under georg-itgclaudeAgent.
    pub repo: String,
    pub tag_prefix: String,
    /// Optional one-time runtime published in the same repo under its own tag prefix.
    #[serde(default)]
    pub runtime: Option<RuntimeSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RuntimeSpec {
    pub tag_prefix: String,
    /// First extension version (semver) whose backend uses the runtime; older builds skip it.
    pub from_version: String,
}

fn spec(id: &str, name: &str, subtitle: &str, icon: &str, repo: &str, tag_prefix: &str) -> ExtensionSpec {
    ExtensionSpec {
        id: id.into(), name: name.into(), subtitle: subtitle.into(),
        icon: icon.into(), repo: repo.into(), tag_prefix: tag_prefix.into(), runtime: None,
    }
}

pub fn builtin() -> Vec<ExtensionSpec> {
    let mut gc = spec("com.attract.genius-cut", "Genius Cut",
             "For Adobe Premiere Pro · Transcript-driven trimming", "GC", "genius-cut", "v");
    gc.runtime = Some(RuntimeSpec { tag_prefix: "runtime-v".into(), from_version: "0.2.0".into() });
    vec![
        spec("com.attract.pr-extension", "PR Extension",
             "For Adobe Premiere Pro · ElevenLabs + HeyGen + Assets", "PR", "pr-extension", "extension-v"),
        gc,
    ]
}

#[derive(Deserialize)]
struct Doc {
    version: u32,
    extensions: Vec<ExtensionSpec>,
}

fn valid_id(s: &str) -> bool {
    match s.strip_prefix("com.attract.") {
        Some(rest) => !rest.is_empty() && s.len() <= 64
            && rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        None => false,
    }
}

fn valid_repo(s: &str) -> bool {
    !s.is_empty() && s.len() <= 100 && !s.starts_with('.')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn valid_prefix(s: &str) -> bool {
    s.len() <= 20 && s.ends_with('v') && s.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

fn check(e: &ExtensionSpec) -> Result<(), String> {
    let n = |s: &str| s.chars().count();
    if !valid_id(&e.id) { return Err(format!("invalid id {:?}", e.id)); }
    if !valid_repo(&e.repo) { return Err(format!("{}: invalid repo {:?}", e.id, e.repo)); }
    if !valid_prefix(&e.tag_prefix) { return Err(format!("{}: invalid tag_prefix {:?}", e.id, e.tag_prefix)); }
    if n(&e.name) == 0 || n(&e.name) > 60 { return Err(format!("{}: name must be 1-60 characters", e.id)); }
    if n(&e.subtitle) > 120 { return Err(format!("{}: subtitle is longer than 120 characters", e.id)); }
    if n(&e.icon) == 0 || n(&e.icon) > 3 { return Err(format!("{}: icon must be 1-3 characters", e.id)); }
    if let Some(rt) = &e.runtime {
        if !valid_prefix(&rt.tag_prefix) { return Err(format!("{}: invalid runtime tag_prefix {:?}", e.id, rt.tag_prefix)); }
        if rt.tag_prefix.starts_with(&e.tag_prefix) || e.tag_prefix.starts_with(&rt.tag_prefix) {
            return Err(format!("{}: shares a runtime prefix with its own releases", e.id));
        }
        if !crate::runtime::is_semver(&rt.from_version) {
            return Err(format!("{}: invalid runtime from_version {:?}", e.id, rt.from_version));
        }
    }
    Ok(())
}

/// Parse and validate a registry document. All or nothing: one bad entry rejects the lot.
pub fn parse(json: &str) -> Result<Vec<ExtensionSpec>, String> {
    let doc: Doc = serde_json::from_str(json).map_err(|e| format!("registry.json isn't valid: {}", e))?;
    if doc.version != 1 { return Err(format!("registry.json version {} isn't supported", doc.version)); }
    if doc.extensions.is_empty() { return Err("registry.json lists no extensions (empty)".into()); }
    for (i, e) in doc.extensions.iter().enumerate() {
        check(e)?;
        if doc.extensions[..i].iter().any(|p| p.id == e.id) {
            return Err(format!("{} is listed twice", e.id));
        }
        // Two apps sharing a repo must not have one tag prefix inside the other, or one
        // app's releases would be offered as the other's updates.
        if let Some(p) = doc.extensions[..i].iter().find(|p| p.repo == e.repo
            && (p.tag_prefix.starts_with(&e.tag_prefix) || e.tag_prefix.starts_with(&p.tag_prefix))) {
            return Err(format!("{} and {} share repo {} with overlapping tag prefixes", p.id, e.id, e.repo));
        }
    }
    Ok(doc.extensions)
}

/// Remote order first; built-ins the remote list forgot are kept (so nobody is stranded with
/// an installed extension they can't update or uninstall). For built-ins, the remote list may
/// change display text but never the repo or tag prefix.
pub fn merge(remote: Vec<ExtensionSpec>, builtin: Vec<ExtensionSpec>) -> Vec<ExtensionSpec> {
    let mut out: Vec<ExtensionSpec> = remote
        .into_iter()
        .map(|mut r| {
            if let Some(b) = builtin.iter().find(|b| b.id == r.id) {
                r.repo = b.repo.clone();
                r.tag_prefix = b.tag_prefix.clone();
                r.runtime = b.runtime.clone();
            }
            r
        })
        .collect();
    for b in builtin {
        if !out.iter().any(|e| e.id == b.id) { out.push(b); }
    }
    out
}

pub fn find_in<'a>(list: &'a [ExtensionSpec], id: &str) -> Option<&'a ExtensionSpec> {
    list.iter().find(|e| e.id == id)
}

fn cell() -> &'static RwLock<Vec<ExtensionSpec>> {
    static CURRENT: OnceLock<RwLock<Vec<ExtensionSpec>>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(builtin()))
}

pub fn current() -> Vec<ExtensionSpec> {
    cell().read().map(|l| l.clone()).unwrap_or_else(|_| builtin())
}

pub fn set_current(list: Vec<ExtensionSpec>) {
    if let Ok(mut l) = cell().write() { *l = list; }
}

pub fn find(id: &str) -> Option<ExtensionSpec> {
    find_in(&current(), id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{
      "version": 1,
      "extensions": [
        {"id": "com.attract.pr-extension", "name": "PR Extension", "subtitle": "For Adobe Premiere Pro", "icon": "PR", "repo": "pr-extension", "tag_prefix": "extension-v"},
        {"id": "com.attract.genius-cut", "name": "Genius Cut", "subtitle": "Transcript-driven trimming", "icon": "GC", "repo": "genius-cut", "tag_prefix": "v"},
        {"id": "com.attract.new-app", "name": "New App", "subtitle": "Something new", "icon": "NA", "repo": "new-app", "tag_prefix": "v"}
      ]
    }"#;

    #[test]
    fn builtin_has_both_shipped_extensions_with_their_repos() {
        let b = builtin();
        assert_eq!(find_in(&b, "com.attract.pr-extension").unwrap().repo, "pr-extension");
        assert_eq!(find_in(&b, "com.attract.genius-cut").unwrap().tag_prefix, "v");
    }

    #[test]
    fn parses_a_valid_registry_in_order() {
        let list = parse(GOOD).unwrap();
        let ids: Vec<&str> = list.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["com.attract.pr-extension", "com.attract.genius-cut", "com.attract.new-app"]);
    }

    #[test]
    fn rejects_entries_that_could_point_somewhere_unsafe() {
        let bad = [
            r#""id": "../../evil""#,
            r#""id": "com.evil.thing""#,
            r#""id": "com.attract.UPPER""#,
            r#""repo": "../other""#,
            r#""repo": "someone-else/repo""#,
            r#""tag_prefix": "../v""#,
            r#""name": """#,
            r#""icon": "TOOLONG""#,
        ];
        for field in bad {
            let key = field.split(':').next().unwrap();
            let entry = r#"{"id": "com.attract.x", "name": "X", "subtitle": "s", "icon": "X", "repo": "x", "tag_prefix": "v"}"#;
            // replace just that field in an otherwise valid entry
            let re = regex_lite_replace(entry, key, field);
            let doc = format!(r#"{{"version": 1, "extensions": [{}]}}"#, re);
            assert!(parse(&doc).is_err(), "accepted {}", field);
        }
    }

    #[test]
    fn rejects_duplicate_ids_unknown_versions_and_junk() {
        let dup = r#"{"version": 1, "extensions": [
          {"id": "com.attract.a", "name": "A", "subtitle": "", "icon": "A", "repo": "a", "tag_prefix": "v"},
          {"id": "com.attract.a", "name": "A2", "subtitle": "", "icon": "A", "repo": "a2", "tag_prefix": "v"}]}"#;
        assert!(parse(dup).unwrap_err().contains("twice"));
        assert!(parse(r#"{"version": 2, "extensions": []}"#).unwrap_err().contains("version"));
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"version": 1, "extensions": []}"#).unwrap_err().contains("empty"));
    }

    #[test]
    fn rejects_overlapping_tag_prefixes_in_a_shared_repo() {
        let doc = r#"{"version": 1, "extensions": [
          {"id": "com.attract.a", "name": "A", "subtitle": "", "icon": "A", "repo": "shared", "tag_prefix": "v"},
          {"id": "com.attract.b", "name": "B", "subtitle": "", "icon": "B", "repo": "shared", "tag_prefix": "v"}]}"#;
        assert!(parse(doc).unwrap_err().contains("overlapping"));
    }

    #[test]
    fn merge_keeps_remote_order_and_never_drops_a_builtin() {
        // A remote list that forgets an extension must not strand people who have it installed.
        let remote = parse(r#"{"version": 1, "extensions": [
          {"id": "com.attract.new-app", "name": "New App", "subtitle": "", "icon": "NA", "repo": "new-app", "tag_prefix": "v"},
          {"id": "com.attract.genius-cut", "name": "Genius Cut (renamed)", "subtitle": "", "icon": "GC", "repo": "genius-cut", "tag_prefix": "v"}]}"#).unwrap();
        let merged = merge(remote, builtin());
        let ids: Vec<&str> = merged.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["com.attract.new-app", "com.attract.genius-cut", "com.attract.pr-extension"]);
        assert_eq!(merged[1].name, "Genius Cut (renamed)"); // remote wins for display text
        // The remote entry has no runtime; the built-in's (incl. from_version) is kept.
        assert_eq!(merged[1].runtime.as_ref().unwrap().from_version, "0.2.0");
    }

    #[test]
    fn the_repo_and_prefix_of_a_builtin_can_not_be_changed_remotely() {
        // Otherwise an edited list could redirect PR Extension installs to another repo.
        let remote = parse(r#"{"version": 1, "extensions": [
          {"id": "com.attract.pr-extension", "name": "PR Extension", "subtitle": "", "icon": "PR", "repo": "evil-fork", "tag_prefix": "v"}]}"#).unwrap();
        let merged = merge(remote, builtin());
        let pr = find_in(&merged, "com.attract.pr-extension").unwrap();
        assert_eq!((pr.repo.as_str(), pr.tag_prefix.as_str()), ("pr-extension", "extension-v"));
    }

    #[test]
    fn the_published_registry_json_is_valid_and_matches_the_builtins() {
        // registry.json at the repo root is what every installed copy fetches.
        let list = parse(include_str!("../../registry.json")).expect("registry.json must parse");
        for b in builtin() {
            let r = find_in(&list, &b.id).unwrap_or_else(|| panic!("registry.json is missing {}", b.id));
            assert_eq!((r.repo.as_str(), r.tag_prefix.as_str()), (b.repo.as_str(), b.tag_prefix.as_str()));
            assert_eq!(r.runtime, b.runtime, "{}: runtime differs from the built-in", b.id);
        }
    }

    #[test]
    fn runtime_field_is_optional_and_validated() {
        let with = r#"{"version": 1, "extensions": [
          {"id": "com.attract.genius-cut", "name": "Genius Cut", "subtitle": "", "icon": "GC", "repo": "genius-cut", "tag_prefix": "v",
           "runtime": {"tag_prefix": "runtime-v", "from_version": "0.2.0"}}]}"#;
        let rt = parse(with).unwrap()[0].runtime.clone().unwrap();
        assert_eq!((rt.tag_prefix.as_str(), rt.from_version.as_str()), ("runtime-v", "0.2.0"));
        assert!(parse(&with.replace(r#", "from_version": "0.2.0""#, "")).is_err(), "accepted a runtime without from_version");
        for bad in ["0.2", "v0.2.0", "0.2.0-beta", ""] {
            assert!(parse(&with.replace("0.2.0", bad)).unwrap_err().contains("from_version"), "accepted from_version {:?}", bad);
        }
        let bad = with.replace("runtime-v", "../v");
        assert!(parse(&bad).is_err());
        let clash = with.replace(r#""tag_prefix": "runtime-v""#, r#""tag_prefix": "v""#);
        assert!(parse(&clash).unwrap_err().contains("runtime"));
        assert!(parse(GOOD).unwrap()[0].runtime.is_none());
    }

    #[test]
    fn find_uses_the_current_list() {
        assert!(find("com.attract.pr-extension").is_some());
        assert!(find("com.attract.nope").is_none());
    }

    /// Test helper: swap one `"key": value` in a flat JSON object for `replacement`.
    fn regex_lite_replace(entry: &str, key: &str, replacement: &str) -> String {
        let start = entry.find(key).unwrap();
        let after = &entry[start..];
        let end = after.find(',').or_else(|| after.find('}')).unwrap();
        format!("{}{}{}", &entry[..start], replacement, &after[end..])
    }
}
