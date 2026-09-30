# Release Process

This repo only ever contains installer releases, so GitHub's "Latest" release is always
an installer release. That is what the app's self-updater reads:
`releases/latest/download/latest.json`.

## Cutting a release

1. Ensure `npm run tauri build` succeeds locally.
2. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `package.json`,
   and `MANAGER_VERSION` in `src/App.tsx`.
3. Commit: `chore: bump to v0.3.0`.
4. Push, then tag and push the tag:
   ```
   git tag v0.3.0
   git push origin v0.3.0
   ```
5. `.github/workflows/release.yml` runs on Windows: builds the app, signs the updater
   payload, and creates a release (marked Latest) with
   `GeniusInstallerManager-Setup-<version>.exe`, its `.sig`, and `latest.json`.
6. **Until old installs are retired, bridge the release** (next section).

## Updates for old "PR Extension Manager" 0.1.0 installs

Those installs check `pr-extension`'s Latest release for `latest.json`. After each installer
release, upload the new `latest.json` onto the `manager-v0.1.0` release in `pr-extension`,
and keep that release marked Latest there:

```bash
gh release download v0.3.0 -p latest.json -R georg-itgclaudeAgent/genius-installer-manager --clobber
gh release upload manager-v0.1.0 latest.json --clobber -R georg-itgclaudeAgent/pr-extension
```

The `url` inside `latest.json` points at this repo's exe, and the signature verifies with
the unchanged key. So an old install updates straight to the renamed app, which then
removes the old "PR Extension Manager" copy on first start.

## Signing keys

The updater key pair was created once with `tauri signer generate`. The public key is
`plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. **Never change it**: existing
installs only accept updates signed by the matching private key.

The repo needs two Actions secrets:

```bash
gh secret set TAURI_PRIVATE_KEY -R georg-itgclaudeAgent/genius-installer-manager < ~/.tauri/pr-extension-manager.key
gh secret set TAURI_KEY_PASSWORD -R georg-itgclaudeAgent/genius-installer-manager   # prompts; nothing echoed
```
