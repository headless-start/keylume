# Contributing

Thanks for helping! Keylume is a small, local-only app; please keep it that way:
no network calls, no telemetry, no accounts. Everyone taking part follows the
[code of conduct](CODE_OF_CONDUCT.md).

## Set up

- Rust (stable) with `clippy` and `rustfmt`, and Node.js 22 (20.19 or later also works).
- Linux / WSL: `sudo apt install libudev-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`
  (only needed for the desktop app; the crates and the web UI build without them).
- `npm ci`, then `npm run build` once: the desktop app embeds the built window, so the whole
  workspace (`--workspace`) only compiles after it.

## Everyday commands

```bash
cargo test --workspace --locked      # protocol, simulator, service, library invariants
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all                      # config in rustfmt.toml (long lines are fine)
npm test                             # UI tests against the in-browser mock backend
npm run dev                          # the UI in a browser, no keyboard needed (this PC only;
                                     # KEYLUME_DEV_HOST=0.0.0.0 npm run dev to reach it from your network)
npm run fixtures                     # regenerate the browser preview's library after changing profiles
                                     # (it's generated, not committed; npm run dev / npm test build it when missing)
npm audit && cargo audit             # known vulnerabilities in dependencies (CI runs both)
```

## Where things live

| Path | What |
|---|---|
| `crates/keylume-proto` | wire protocols (Rongyuan, HID LampArray) and layouts, pure and fully tested |
| `crates/keylume-device` | HID transport, the boards Keylume knows, simulators (timing rules!) |
| `boards/`, `layouts/` | keyboards described as data, and their key positions ([DEVICES.md](docs/DEVICES.md)) |
| `packs/` | the design packs that come with Keylume ([PACKS.md](docs/PACKS.md)) |
| `crates/keylume-profiles` | the generated library: palettes, themes, patterns, effects, skins |
| `crates/keylume-live` | real-time animators (pure; previews use them too) |
| `crates/keylume-core` | device service, store/settings, packs, backups |
| `src-tauri` | desktop shell: commands, tray, hotkeys, captures |
| `src` | React UI |

## Adding profiles

- A **theme** is the easiest: a named set of colours in
  `crates/keylume-profiles/themes/<collection>.json` becomes 41 profiles. Check it with
  `cargo run -q -p keylume-profiles --example themes -- <file>`; names must be your
  own, never a franchise's or brand's (see `docs/PROFILES.md`).
- A **palette** in `palettes.rs` automatically renders through every pattern.
- A **pattern** in `patterns.rs` automatically renders with every palette.
- Tests reject duplicate ids or names, identical pictures and mostly-dark designs.
- **Never change a shipped id**; rename the display name instead. If an id must go,
  add a mapping to `store::replacement` so favourites migrate.
- A **pack** is theme collections in the same format (and optionally designs made in the
  app, in `designs.json`), in `packs/<id>/` with a `pack.json`; `keylume-cli pack build`
  turns it into a `.keylumepack`. Packs of your own designs are easiest made in the app:
  Library → Mine → Make a pack ([PACKS.md](docs/PACKS.md)).

## Hardware

Only the Epomaker TK68 is verified; HID LampArray keyboards are supported to the standard
but not yet tried on a real one. [DEVICES.md](docs/DEVICES.md) says how a keyboard is
described and how a new one gets verified. Please read `docs/PROTOCOL.md` (especially
*Timing*) before sending commands to a real Rongyuan board: writes too close together wedge
the firmware's config channel until a bare GET or a replug. Test against the simulators
first (`keylume-cli --sim …`, or the demo keyboards in Settings).

## Safety rules in the code

- Nothing is written to a device until `keylume-device::identity` has confirmed it's exactly one
  supported keyboard; a restore writes nothing until the whole backup has passed its checks.
- Everything from outside (uploads, packs, files on disk, the window's commands) is
  bounded and validated where it comes in: `Profile::validate`, `AppSettings::validate`,
  `LiveEffect::validate` and `Backup::validate`.
- The window never names a file for the app to read or write: dialogs are opened by the app's
  own commands, and a dropped file is accepted only if the OS reported that drop.

## Pull requests

Work on a branch of your own (`fix/…`, `feat/…`, or a fork), and open a pull request into
`main`: CI runs every check above, and a change is merged once it's green and reviewed. Keep
changes focused, add or update tests, and add a line to the changelog's *Unreleased* section
for anything a user would notice. By
contributing you agree your work is released under the MIT licence. Security problems go
to the private channel in [SECURITY.md](SECURITY.md), not to issues.
