# 7. macOS release signing and notarization

[← External plugins](./06-external-plugins.md) · [Docs index](./README.md)

Unsigned macOS apps downloaded from the internet are blocked by **Gatekeeper** (“Chronicle is damaged and can’t be opened” or “unidentified developer”). Chronicle release DMGs must be **Developer ID signed** and **notarized** before users can open them normally.

## Requirements

- [Apple Developer Program](https://developer.apple.com/programs/) membership ($99/year)
- A **Developer ID Application** certificate (not “Apple Development” or “Mac App Distribution”)
- GitHub repository secrets configured (see below)
- Repository variable **`MACOS_SIGNING_ENABLED`** = `true`

## One-time: export your certificate

1. Open **Keychain Access** → **My Certificates**
2. Find **Developer ID Application: Your Name (TEAMID)**
3. Right-click → **Export** → save as `chronicle-codesign.p12` with a strong password
4. Encode for GitHub:

```bash
base64 -i chronicle-codesign.p12 | pbcopy
```

Paste into the `APPLE_CERTIFICATE` secret (single line, no spaces).

## GitHub secrets

| Secret | Value |
|--------|--------|
| `APPLE_CERTIFICATE` | Base64 of the `.p12` file |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full name, e.g. `Developer ID Application: Jane Doe (AB12CD34EF)` |
| `APPLE_ID` | Apple ID email |
| `APPLE_PASSWORD` | [App-specific password](https://appleid.apple.com/account/manage) |
| `APPLE_TEAM_ID` | 10-character team ID from [Membership details](https://developer.apple.com/account) |
| `KEYCHAIN_PASSWORD` | Any random string (CI temporary keychain only) |

Find signing identity locally:

```bash
security find-identity -v -p codesigning
```

## Repository variable

Settings → Secrets and variables → Actions → **Variables**:

| Variable | Value |
|----------|--------|
| `MACOS_SIGNING_ENABLED` | `true` |

Release workflow refuses to build if this is not `true`, so unsigned DMGs are never published by mistake.

## Local test before tagging

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: …"
# Import .p12 into your login keychain first, or use the same env vars as CI
bun run tauri build --target aarch64-apple-darwin
spctl -a -vv src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Chronicle.app
```

`spctl` should report `accepted` and `source=Notarized Developer ID`.

## Cut a release

1. Update `CHANGELOG.md` with a `## [x.y.z]` section
2. Commit and push to `master`
3. Tag and push:

```bash
git tag -a v0.1.0 -m "Chronicle v0.1.0"
git push origin v0.1.0
```

The **Release** workflow builds arm64 + x64 DMGs, signs, notarizes, and publishes GitHub Release notes from `CHANGELOG.md`.

## Troubleshooting CI

| Error | Fix |
|-------|-----|
| `failed to import keychain certificate` | Re-export `.p12`; ensure base64 is one line; check `APPLE_CERTIFICATE_PASSWORD` |
| `MACOS_SIGNING_ENABLED is not true` | Set the repository variable |
| Notarization timeout | Ensure `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` are correct; app-specific password must have notarization access |
| Gatekeeper still blocks after download | Build must complete notarization + stapling; re-run release after fixing secrets |

## Unsigned local dev builds

`bun run tauri dev` and local `tauri build` without signing produce ad-hoc binaries fine for development. Only **GitHub Release DMGs** require full signing.
