# Releasing Keylume

How a version is checked, built, signed and published. Nothing here needs the internet at
runtime: releases are downloaded by people, never fetched by the app.

## Every release

Changes reach `main` through pull requests, each with CI green. A release is cut from `main`:

1. All checks clean (see [CONTRIBUTING.md](../CONTRIBUTING.md)), including clippy for the
   Windows target, `npm audit` and `cargo audit`.
2. The new version, following [semantic versioning](https://semver.org) (a patch for fixes, a
   minor version for new features), in `Cargo.toml` (workspace), `package.json` (and the root
   of `package-lock.json`) and `src-tauri/tauri.conf.json`; `cargo check` updates
   `Cargo.lock`. An entry in [CHANGELOG.md](../CHANGELOG.md).
3. A commit `vX.Y.Z: short summary` and a tag `vX.Y.Z`; pushing the tag starts the release
   build.

## Building the installer

- **On a Windows PC:** `npx tauri build`.
- **From Linux or WSL:** `XWIN_ACCEPT_LICENSE=1 npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc`.
- **On GitHub:** *Actions → Release build* (run it by hand, or push a `v*` tag). It makes two
  artifacts: `keylume-windows-<tag>` (the installer, `SHA256SUMS.txt`,
  `THIRD-PARTY-NOTICES.txt`) and `keylume-linux-<tag>` (the `.deb`, the AppImage,
  `SHA256SUMS-linux.txt`, `THIRD-PARTY-NOTICES-linux.txt`). Artifacts are kept 30 days: a
  draft release keeps them for good (below).

Each build first writes `src-tauri/THIRD-PARTY-NOTICES.txt` (`tools/third_party_notices.mjs`):
the licences of every crate and npm package the app ships, for the target being built. The
installer puts it next to the app.

## Signing

Unsigned installers work, but Windows SmartScreen warns about them ("Windows protected your
PC") until they've been downloaded often enough. Signing is set up in the repository's
**secrets**, never in files:

- **A certificate you have as a `.pfx`:** add `WINDOWS_CERTIFICATE` (the file, base64-encoded)
  and `WINDOWS_CERTIFICATE_PASSWORD`. The release workflow then signs the app and the installer
  with `tools/sign.ps1`, with a timestamp.
- **Certificates issued since mid-2023** keep their key in hardware or a cloud service and can't
  be exported as a `.pfx`. Use the provider's signing tool, or Microsoft's Trusted Signing, by
  changing the command in `tools/sign.ps1` (the workflow's `signCommand` calls it for each file).
- Open-source projects can also apply for free signing (SignPath offers it).

Without secrets, the workflow still builds, and says the build is unsigned.

## Publishing

1. Download the workflow's artifact and try the installer on a clean Windows user account.
2. Check the signature (right-click → Properties → Digital Signatures) when it's signed.
3. Make a GitHub release from the tag: attach every file of both artifacts, and paste the
   changelog entry and the checksums into the notes. (It can wait as a draft until it's
   been tried.)

People can check a download with `certutil -hashfile Keylume_X.Y.Z_x64-setup.exe SHA256` on
Windows, or `sha256sum -c SHA256SUMS.txt` elsewhere.

## Design packs

Packs are built and signed on the publisher's own computer with the publisher key, which never
goes into the repository ([PACKS.md](PACKS.md)). The packs that come with Keylume are built
into the app from `packs/`; packs sold or shared separately are `.keylumepack` files made
with `keylume-cli pack build`.

## Repository settings

Set once on GitHub, and worth checking now and then:

- **Private vulnerability reporting** on (SECURITY.md and the code of conduct send reports
  there), with **Dependabot alerts** and **security updates** (Settings → Code security).
- `main` protected: changes land by pull request, and CI must pass.
- Paid pack sources stay out of this repository; `packs/` holds only the packs that come with
  Keylume.
