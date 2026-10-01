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

Edit [`registry.json`](registry.json) on `main` and add an entry: a unique bundle id
(`com.attract.<name>`), display name, subtitle, 1-3 letter icon, repo and tag prefix.
Every installed copy picks it up on its next launch, or when someone presses **Check for
updates**, and shows it with a **NEW APP** badge. No installer release is needed.

The extension's repo must be public, under `georg-itgclaudeAgent`, and publish releases
tagged `<tag_prefix>X.Y.Z` with a `.zip` whose `CSXS/manifest.xml` declares the same
`ExtensionBundleId`. The installer refuses a zip built for a different extension.

Safety: entries are validated, an invalid `registry.json` is ignored (the last good copy is
used), and for apps built into the installer the repo and tag prefix can't be changed
remotely. `cargo test` checks `registry.json` too, so run it before pushing an edit.

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
