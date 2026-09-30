# Genius Installer Manager

A small Windows app that installs, updates and uninstalls itGenius extensions for
Adobe Premiere Pro. It checks each extension's own GitHub repo for new releases.

| Extension | Repo | What it does |
|---|---|---|
| PR Extension | [pr-extension](https://github.com/georg-itgclaudeAgent/pr-extension) | ElevenLabs + HeyGen + Assets |
| Genius Cut | [genius-cut](https://github.com/georg-itgclaudeAgent/genius-cut) | Transcript-driven trimming (coming soon) |

## For users

Download the latest `GeniusInstallerManager-Setup-*.exe` from
[Releases](https://github.com/georg-itgclaudeAgent/genius-installer-manager/releases/latest)
and run it once. The app updates itself from then on.

## Adding an extension

Add an entry to `EXTENSIONS` in `src-tauri/src/paths.rs`: a unique bundle id, display
name, subtitle, two-letter icon, repo and tag prefix. The extension's repo must be public
and publish a release with a `.zip` whose `CSXS/manifest.xml` declares the same
`ExtensionBundleId`. The installer refuses a zip built for a different extension.

## Development

```bash
npm ci
npm test              # vitest
npm run tauri dev     # run the app
cd src-tauri && cargo test
```

Debug builds never remove the legacy "PR Extension Manager" install, so `tauri dev` is
safe on a machine that still has it.

## Releasing

See [docs/release-process.md](docs/release-process.md).
