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
6. Nothing else: installed copies find the release through `releases/latest/download/latest.json`.

## Old "PR Extension Manager" 0.1.0 installs

Those were signed with a different key (retired on 2026-10-01; its private key had been
exposed). A 0.1.0 install can't verify updates signed with the current key, so it won't
update itself: install `GeniusInstallerManager-Setup-<version>.exe` by hand once from the
release page. The new app removes the old "PR Extension Manager" copy on first start, and
updates itself from then on. Don't upload new `latest.json` files to `pr-extension`'s
`manager-v0.1.0` release: 0.1.0 would download them and fail the signature check.

## Signing key

The updater key pair is stored in GCP Secret Manager, project `agent-georg`:

| Secret | What |
|---|---|
| `GENIUS_INSTALLER_SIGNING_KEY` | Private key (the file `tauri signer generate` writes) |
| `GENIUS_INSTALLER_SIGNING_KEY_PASSWORD` | Its password |
| `GENIUS_INSTALLER_SIGNING_PUBKEY` | Public key; must equal `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` |

The public key is built into every installed copy. **Never change it** unless you accept that
existing installs will stop updating themselves and need one manual install.

The repo's Actions secrets are filled from Secret Manager without the values ever being shown:

```bash
gcloud secrets versions access latest --secret=GENIUS_INSTALLER_SIGNING_KEY --project=agent-georg   | gh secret set TAURI_PRIVATE_KEY -R georg-itgclaudeAgent/genius-installer-manager
gcloud secrets versions access latest --secret=GENIUS_INSTALLER_SIGNING_KEY_PASSWORD --project=agent-georg   | gh secret set TAURI_KEY_PASSWORD -R georg-itgclaudeAgent/genius-installer-manager
```
