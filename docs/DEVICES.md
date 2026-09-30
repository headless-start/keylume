# Keyboards

How Keylume supports a keyboard, which ones work today, and how another one gets added.

## What works today

| Keyboard | Driver | Support |
|---|---|---|
| Epomaker TK68 (firmware 3.04) | Rongyuan | **Verified** on the real keyboard |
| Any keyboard that implements **HID LampArray**, the standard behind Windows Dynamic Lighting | LampArray | **Experimental**: built to the published standard and tested against a simulated keyboard, not yet on a real one |

Keylume drives one keyboard at a time: a verified board first, otherwise the first LampArray
keyboard it finds.

## How Keylume adapts to a keyboard

Nothing in the app names a model. A keyboard is described by data, and everything follows
from that description:

- **Its shape.** Keylume draws the keyboard it's connected to, and redraws the whole library
  for it. Designs are drawn from key positions, so a 60 % board, a TKL and a full-size board
  each get every design in their own shape (the keypad of a full-size board lights up; keys
  a board doesn't have are left out). The last keyboard's layout is remembered
  (`layout.json` in the data folder), so the library opens drawn for it.
- **What it can do** (its *features*): the app only offers those.

| Feature | Without it |
|---|---|
| `perKey` | No per-key designs, spells or Paint keys |
| `effects` | Only the animations listed are offered (on a LampArray board, Keylume draws them) |
| `hostDriven` | Keylume drives the lights frame by frame while it runs; nothing is stored on the keyboard |
| `sideLight` | No Side light tab |
| `live` | No live effects |
| `keymap`, `macros` | No Keys page |
| `onboardProfiles`, `pictureLayers` | Fewer choices in Settings |
| `settings` | No report rate, debounce or sleep settings |
| `backup` | No backup, restore or factory reset |
| `readBack` | After connecting, what the keys show is unknown until Keylume sets it |

The Library hides designs a keyboard can't show (for example, animations that answer
typing on a LampArray keyboard).

## Boards described by a file (`boards/*.json`)

A board file says how to recognise the keyboard, which driver talks to it, its layout, and
its features. The TK68's:

```json
{
  "id": "epomaker-tk68",
  "name": "Epomaker TK68",
  "maker": "Epomaker",
  "driver": "rongyuan",
  "support": "verified",
  "layout": "epomaker-tk68",
  "usb": { "vid": "05AC", "pid": "024F", "interface": 0, "usagePage": "0001", "usage": "0006",
           "manufacturer": "ROYUAN", "products": ["Acrylic68"], "featureReport": 64 },
  "features": { "perKey": true, "pictureLayers": 3, "effects": ["off", "static", "…"], "…": "…" }
}
```

Before anything is written, a device must match every `usb` field: the ids, the
interface and collection, the manufacturer and product strings, and a HID report
descriptor declaring the protocol's feature report. The ids alone prove nothing: the TK68
uses the ids of an Apple keyboard, and 52 other models on the same platform share them
([FIRMWARE.md](FIRMWARE.md), *Other boards on the platform*). More than one match is
refused. `keylume-cli list` shows each device's verdict without sending anything to it.

The layout (`layouts/<id>.json`) gives every key's id, position, size, light slot and HID
usage. Key ids are Keylume's own and the same on every board (`esc`, `a`, `lshift`,
`kp7`…; the full list is `KEYS` in `crates/keylume-proto/src/standard.rs`), so a design
lights the same keys everywhere. `"finish"` says what the keyboard itself looks like,
`"dark"` (the default) or `"white"` (a white or clear case with white keycaps, like the
TK68's acrylic body): Home draws the keyboard that way. `"fnLayer"` is what each key does
with Fn by default: Keys → Fn layer → Reset puts it back, and Settings offers to write it.
Standard layouts get Keylume's standard Fn layer (`standard::standard_fn`): on a board without
an F-row, Fn + 1 … = are F1 … F12; everywhere, Fn + Z X C are previous / play-pause / next,
Fn + , . / mute / volume down / volume up, and Fn + [ ] screen brightness. A board file's
layout adds its own functions beside these (the TK68 keeps its lighting controls on the
arrows, device keys on Q–T, and Insert, Print Screen, Home, End and Scroll Lock on J–O).

## The drivers

### Rongyuan (the TK68's family)

Pictures are stored in the keyboard's flash (about 1.5 s each), animations run in its
firmware, live effects use two real-time channels (one colour for the whole board, or 32
music bars), and the keyboard keeps key maps, macros, settings and a side light. The wire
format is in [PROTOCOL.md](PROTOCOL.md). Epomaker's own app knows 179 models on this
platform from many brands; each needs its own layout and an owner to verify it before
Keylume ships it (see *Adding a board*).

### HID LampArray (any maker)

A LampArray keyboard describes itself: how many lamps it has, where each one is, and which
key sits above it. Keylume reads that description and builds the layout from it (standard
key sizes for keys it knows, the reported positions for everything), so a new model needs
no file. Lamps that aren't under a key (edges, logos, underglow) take the colour of the
nearest key.

- **Keylume drives every lamp.** Pictures show at once. Animations are drawn by Keylume,
  frame by frame, except the ones that answer typing (Keylume doesn't watch your keys).
  Music bars are drawn across the keys. Spells show a picture every 0.7 s.
- **Nothing is stored on the keyboard.** When Keylume closes it hands the lamps back, and
  the keyboard shows its own lighting again. Keylume keeps running in the tray, so your
  lighting stays while you use the computer.
- **Finding one.** Any HID collection on the Lighting and Illumination page (0x59) whose
  attributes say it's a keyboard. Opening it reads its lamp descriptions (the standard asks
  for each with a request report); no light changes until you pick a design.
- **Windows.** Windows Dynamic Lighting may be driving the same keyboard. If the colours
  flicker or switch back, turn Dynamic Lighting off for that keyboard (Settings →
  Personalization → Dynamic Lighting). This hasn't been checked on a real keyboard yet.
- **Linux.** Opening a keyboard needs a udev rule. When one is missing, Keylume shows the
  exact line to save in `/etc/udev/rules.d/`. (The `.deb` installs the TK68's rule.)

## Demo keyboards

With no keyboard plugged in, **Settings → Demo keyboard** plays a simulated one: the TK68,
or a full-size LampArray keyboard with an underglow strip. The whole app works against them,
so you can see it adapt without the hardware (and tests use the same simulators).

## Adding a board

**A LampArray keyboard** needs nothing: plug it in. If something looks wrong, please report
what `keylume-cli list` prints and what you saw.

**A Rongyuan board** needs a board file and a layout, and an owner to verify them:

1. Read-only first: `keylume-cli list` shows the device's USB strings and descriptor
   verdict. Nothing is sent to it.
2. Write the board file with `"support": "experimental"` and the layout (key positions and
   the light slot of each key). The research notes in [FIRMWARE.md](FIRMWARE.md) say where
   each model's details can be found.
3. Back up the keyboard in its vendor app, then check one step at a time: read the
   firmware version and the current effect, then one static colour, then one per-key
   picture. Keep the recovery path from [FIRMWARE.md](FIRMWARE.md) at hand.
4. When everything checks out, it becomes `verified` and ships with Keylume.

Keylume never guesses at commands on a keyboard it hasn't verified.

## Not supported yet

- Keyboards with their own closed protocols (Razer, Corsair, Logitech, SteelSeries…),
  unless they implement LampArray, as many recent ones do.
- VIA / QMK keyboards (next on the roadmap).
- Mice, headsets and other lighting devices (the LampArray driver only takes keyboards).
