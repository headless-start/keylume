# Roadmap

Where Keylume is going. Goals, not promises or dates; [CHANGELOG.md](../CHANGELOG.md) says
what has shipped.

## Vision

One local app for the lighting and keys of your peripherals: the biggest free lighting
catalogue, your own designs, and deep control of each device, down to its firmware. It
started with one keyboard and grows to many boards, then other devices. Whatever it grows
into, it stays local: no account and no cloud.

## Principles

- **Local only.** No accounts, no cloud, no telemetry, no update checks, no network
  connections, and no link to any game or game store.
- **Designs are data.** The built-in library is generated (palettes × patterns, effects ×
  colours, curated live effects, themes and flags from JSON), so one new palette or pattern
  grows it everywhere. Built-in ids never change once shipped (favourites depend on them);
  retired ids migrate (`keylume-core::store::replacement`).
- **The keyboard's limits are respected.** Per-key pictures need a ~1.5 s flash commit and
  wear the flash, so animation runs in the firmware's effects or in the real-time channels,
  never by rewriting pictures in a loop.
- **Tested without hardware.** Protocol, simulators, the device service, library invariants
  and UI flows are covered, and clippy is clean with `-D warnings`.

## Next

### The catalogue
- [ ] More games, themes and patterns that suit them
- [ ] More live effects that react to what you do (typing heat, per-app colours)
- [ ] Per-app designs: switch when a given program is focused

### Getting started
- [ ] A first-run welcome: find the keyboard, pick a starter design
- [ ] A signed Windows installer (the release workflow is ready for a certificate) and a
      winget manifest
- [ ] A user guide and FAQ (for example, why live effects light the whole board at once)
- [ ] A screen-reader walk-through (contrast, keyboard access and reduced motion are done)

### More keyboards
- [ ] Try a real HID LampArray keyboard (Windows and Linux), then drop "experimental"
- [ ] Other Rongyuan ("yc300") boards share the TK68's protocol: Epomaker's EP108, Skyline 87,
      TH-21 and TH96 first. Each needs a board file, its layout and an owner to verify it
      ([DEVICES.md](DEVICES.md), [FIRMWARE.md](FIRMWARE.md))
- [ ] VIA / QMK keyboards (the keyboard's own effects, brightness and colour)
- [ ] Linux: the music visualiser and screen colour effects (Windows only today)

### The firmware track
- [ ] Identify the TK68's microcontroller, bootloader and update path ([FIRMWARE.md](FIRMWARE.md))
- [ ] Map the stock firmware's remaining commands: simulator first, then careful checks on
      real hardware
- [ ] A decoder for USB captures of vendor traffic (clean-room protocol work)
- [ ] Decide whether custom or open firmware is realistic; nothing is flashed without a
      proven recovery path

### Packs
- [ ] A regular line-up of packs, each going deep on one idea where the free catalogue goes
      broad. Nothing moves from the free catalogue into a pack.

### Later
- Other devices (mice, headsets, light strips) in the same app
- A small plugin format for live effects
