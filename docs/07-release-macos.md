# 7. macOS release signing and notarization

[← External plugins](./06-external-plugins.md) · [Docs index](./README.md)

Chronicle can ship **unsigned** release DMGs (free, Gatekeeper workaround required) or **signed + notarized** DMGs (Apple Developer Program, seamless install).

## Unsigned releases (default, $0)

Set repository variable **`MACOS_SIGNING_ENABLED`** to anything other than `true` (or leave it unset). CI builds ad-hoc DMGs without Apple certificates.

### What users see

macOS Gatekeeper may show “Chronicle is damaged” or “unidentified developer” on first open. The app is fine — Apple blocks unsigned downloads by default.

### First-launch workaround (include in release notes)

**Option A — Right-click**

1. Download the DMG → drag Chronicle to Applications
2. Right-click **Chronicle.app** → **Open** → **Open** again

**Option B — Terminal**

```bash
xattr -dr com.apple.quarantine /Applications/Chronicle.app
open /Applications/Chronicle.app
```

After the first successful launch, macOS remembers the choice.

### Build from source (no Gatekeeper)

```bash
git clone https://github.com/aeswibon/chronicle.git
cd chronicle
bun install
cargo build --release -p chronicle-daemon
bun run tauri dev   # or: bun run tauri build
```

---

## Signed releases ($99/year Apple Developer Program)

For DMGs that open with a normal double-click (no Gatekeeper friction), enable signing.

### Requirements

- [Apple Developer Program](https://developer.apple.com/programs/) membership
- **Developer ID Application** certificate (not “Apple Development”)
- GitHub secrets configured (see below)
- Repository variable **`MACOS_SIGNING_ENABLED`** = `true`

### One-time: export your certificate

1. Open **Keychain Access** → **My Certificates**
2. Find **Developer ID Application: Your Name (TEAMID)**
3. Right-click → **Export** → save as `chronicle-codesign.p12` with a strong password
4. Encode for GitHub:

```bash
base64 -i chronicle-codesign.p12 | pbcopy
```

Paste into the `APPLE_CERTIFICATE` secret (single line, no spaces).

### GitHub secrets

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

### Repository variable

Settings → Secrets and variables → Actions → **Variables**:

| Variable | Value |
|----------|--------|
| `MACOS_SIGNING_ENABLED` | `true` |

### Local test before tagging

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: …"
bun run tauri build --target aarch64-apple-darwin
spctl -a -vv src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Chronicle.app
```

`spctl` should report `accepted` and `source=Notarized Developer ID`.

---

## Cut a release

1. Update `CHANGELOG.md` with a `## [x.y.z]` section
2. Commit and push to `master`
3. Tag and push:

```bash
git tag -a v0.1.0 -m "Chronicle v0.1.0"
git push origin v0.1.0
```

The **Release** workflow builds arm64 + x64 DMGs and publishes GitHub Release notes from `CHANGELOG.md` (with unsigned or signed install instructions as appropriate).

## Troubleshooting CI

| Error | Fix |
|-------|-----|
| `failed to import keychain certificate` | Re-export `.p12`; ensure base64 is one line; or use unsigned mode (`MACOS_SIGNING_ENABLED` ≠ `true`) |
| Notarization timeout | Ensure `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` are correct |
| Gatekeeper blocks unsigned DMG | Expected — use right-click Open or `xattr` (see above) |

## Unsigned local dev builds

`bun run tauri dev` and local `tauri build` without signing work fine for development on your own Mac.
