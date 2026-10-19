# Releasing Vaulty

Releases are built by `.github/workflows/release.yml` when a version tag is pushed. The workflow
uploads everything to a **draft** release; nobody gets it (in the app or on the releases page)
until the draft is published by hand.

## One-time setup

### 1. Update signing key (required)

Every update is signed, and the app refuses anything not signed with this key. Make it on your
own computer, never in CI:

```
npx @tauri-apps/cli@2.12.3 signer generate -w ~/.tauri/vaulty-updater.key
```

Choose a strong password when asked. Then, on GitHub, under **Settings → Secrets and variables →
Actions**:

| Kind | Name | Value |
|---|---|---|
| Secret | `TAURI_SIGNING_PRIVATE_KEY` | contents of `~/.tauri/vaulty-updater.key` |
| Secret | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password you chose |
| Variable | `UPDATER_PUBKEY` | contents of `~/.tauri/vaulty-updater.key.pub` |

**Back the private key and its password up somewhere safe and offline.** If it's lost, copies
already installed can never be updated automatically again; people would have to download a
new version by hand. If it leaks, someone could sign a fake update, so rotate it (new key, new
release, tell users to update by hand).

### 2. Apple signing and notarisation (optional, recommended)

Without it the macOS app is unsigned and people have to allow it under System Settings → Privacy
& Security the first time. With an Apple Developer account:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | your *Developer ID Application* certificate exported as .p12, base64-encoded (`base64 -i cert.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | the .p12 export password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | your Apple ID email |
| `APPLE_PASSWORD` | an app-specific password from appleid.apple.com |
| `APPLE_TEAM_ID` | your team ID |

The workflow switches signing on as soon as `APPLE_CERTIFICATE` exists.

### 3. Windows

Builds are not yet code-signed, so Windows SmartScreen warns on first run. Free signing for open
source projects is available from the SignPath Foundation; Azure Trusted Signing is a low-cost
alternative. Both can be added to the workflow later.

## Cutting a release

1. On `dev`: set the same version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
   `ui/package.json` (the workflow refuses a tag that doesn't match all three), and move the
   *Unreleased* notes in `CHANGELOG.md` under the new version.
2. Merge `dev` into `main` (pull request), then tag `main`:
   ```
   git checkout main && git pull
   git tag -s v0.2.0 -m "Vaulty 0.2.0"
   git push origin v0.2.0
   ```
   A tag containing `-` (e.g. `v0.2.0-beta.1`) becomes a pre-release, which the in-app updater
   ignores.
3. Wait for the **Release** workflow, then open the draft release: check the installers are all
   there, along with `latest.json` and `SHA256SUMS.txt`, paste the changelog notes, and publish.

## Checking a download

Anyone can confirm an installer is the one built here, from this repository:

```
shasum -a 256 -c SHA256SUMS.txt --ignore-missing      # Windows: certutil -hashfile <file> SHA256
gh attestation verify <installer> --repo avmg0/Vaulty
```
