# 7. macOS release signing

[← External plugins](./06-external-plugins.md) · [Docs index](./README.md)

Official [GitHub Releases](https://github.com/aeswibon/chronicle/releases) ship **signed and notarized** DMGs when repository variable `MACOS_SIGNING_ENABLED` is `true`.

## Signed releases (default for this repo)

Users install with a normal double-click — no Gatekeeper workaround.

### Requirements

- [Apple Developer Program](https://developer.apple.com/programs/) membership
- **Developer ID Application** certificate (not “Apple Development”)
- App-specific password for notarization

### GitHub configuration

| Type | Name |
|------|------|
| Variable | `MACOS_SIGNING_ENABLED` = `true` |
| Secret | `APPLE_CERTIFICATE` (base64 `.p12`) |
| Secret | `APPLE_CERTIFICATE_PASSWORD` |
| Secret | `APPLE_SIGNING_IDENTITY` |
| Secret | `APPLE_ID` |
| Secret | `APPLE_PASSWORD` (app-specific) |
| Secret | `APPLE_TEAM_ID` |
| Secret | `KEYCHAIN_PASSWORD` |

Export certificate:

```bash
base64 -i chronicle-codesign.p12 | pbcopy   # paste into APPLE_CERTIFICATE
security find-identity -v -p codesigning    # copy identity for APPLE_SIGNING_IDENTITY
```

Push a version tag (`v*`) to trigger the [Release workflow](../.github/workflows/release.yml). Notes are generated from `CHANGELOG.md`.

## Unsigned releases (forks)

Set `MACOS_SIGNING_ENABLED` to anything other than `true` (or leave unset). CI builds ad-hoc DMGs.

**First launch workaround:**

```bash
xattr -dr com.apple.quarantine /Applications/Chronicle.app
```

Or right-click **Chronicle.app → Open** once.

## Local builds

`bun run tauri dev` and unsigned `tauri build` work for development. Ad-hoc signatures fail `spctl` — expected without Developer ID env vars:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: …"
export APPLE_ID=…
export APPLE_TEAM_ID=…
export APPLE_PASSWORD=…
bun run tauri build
```

## Troubleshooting

| Issue | Fix |
|-------|-----|
| `failed to import keychain certificate` | Re-export `.p12`; ensure base64 is one line |
| Notarization timeout | Retry; check Apple status page |
| Gatekeeper on unsigned DMG | Use `xattr` or right-click Open (see above) |
