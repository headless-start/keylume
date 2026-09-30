# TK68 / Rongyuan "yc300" firmware: what it would take to go deeper

This is the "brain" track: what's underneath the HID protocol in `docs/PROTOCOL.md`,
and what it would take for Keylume to go deeper (update, recover, or eventually run
open firmware on) the Epomaker TK68 and its Rongyuan "yc300"-family siblings.

Clean-room notes only: findings from the vendor app are written in our own words from
reading its already-unpacked JavaScript (no `app.asar` on this install; nothing was
copied into this repo). Web sources are cited inline and marked as guesses where
unverified. **No command in this file was sent to a real keyboard** — see the
Experiments section for what an owner would need to do to try it safely.

## Firmware version (where we are)

- **The TK68 here runs firmware `0x0304`**, read with command `0x80` (reply bytes 1-2,
  little-endian). Keylume's sidebar shows it in decimal as "Firmware 772". Read as
  major/minor it's most likely **3.04** (a guess: we haven't seen how the vendor app
  labels it).
- Everything in `docs/PROTOCOL.md` was verified on this version. Other boards and other
  firmware versions may differ.
- Whether a newer version exists is unknown. The vendor app asks Epomaker's servers for
  updates per device; Keylume never checks (local-only), so updating stays a job for the
  vendor app, see Experiment 3.

What could come later (not urgent, in order of effort):
1. Label the version the way the vendor does (e.g. "3.04"), once confirmed.
2. A small table of which firmware each supported board was verified on, and a gentle
   note in the app when a board reports a version we haven't tested.
3. Keep a record of the firmware version in backups (`keylume-cli` backup), so a restore
   can say when it comes from a different version.
4. Firmware updates or rollback from Keylume itself: only after the chip, its recovery
   path and the update format are known (sections 2-4). Not planned before then.

## 1. What we know

- **Identity.** VID `0x05AC` / PID `0x024F` (decimal 1452/591), confirmed in the
  vendor app's own device table: entry `yc300_acr68pile`, `displayName: "TK68"`,
  `company: "EPOMAKER"`, `featureReportByteLength: 65`, `layer: 3`, `fnLayer: 1` —
  matches `docs/PROTOCOL.md` exactly (3 onboard profiles + 1 Fn layer).
  - `0x05AC:0x024F` is officially **Apple's** vendor ID and the product ID of an
    Apple Aluminium Keyboard / Magic Keyboard with Numeric Keypad (model A1243)
    ([DeviceHunt](https://devicehunt.com/view/type/usb/vendor/05AC/device/024F),
    [USB ID Repository](https://usb-ids.gowdy.us/read/UD/05ac/024f)). Why the board
    reports it isn't documented; a guess is compatibility with macOS's built-in keyboard
    handling. Keylume matches on these ids plus the HID interface (interface 0, usage
    page `0x01`, usage `0x06`), so a genuine Apple keyboard with the same ids would be
    taken for a TK68; checking the product string too is a cheap follow-up.
- **"yc300" is a platform codename, not a chip number.** It's Rongyuan's (容圆科技,
  [rongyuan.tech](https://rongyuan.tech/aboutus.html)) own name for a firmware
  generation. The same vendor app's device table lists many parallel/successive
  generations side by side: `yc300_*` (our board, plus RS6, 5108S, SK6, PC75S, EP108,
  …), `yc3121_*` (hall-effect/OLED boards like `yc3121_tk68_soc_hall` — a *different*,
  newer SKU despite the similar name), `yc500_*`, `ry5088_*`.
- **Rongyuan's public MCU/SoC lineup** (rongyuan.tech "about us" page) is: `YC3121-R`,
  `YC3123-R`, `YC3018`, `PAN1086`, `PAN1080`, `RY6063`, `RY6010`, `PAN1082`, `PAN1010`.
  Nothing is named "YC300" — so whatever's actually in the TK68 either predates this
  list, was dropped from their current marketing, or "yc300" simply doesn't map 1:1 to
  a same-numbered chip. `YC3121` is a real, separate chip: a Bluetooth 5.0 SoC from
  Yichip Microelectronics ([JLCPCB part page](https://jlcpcb.com/partdetail/YICHIP-YC3121D/C2916800)).
- **The strongest lead we have**: an independent, GPL-3.0 reverse-engineering project,
  [`monsgeek-akko-linux`](https://github.com/AuriSolCorues/monsgeek-akko-linux),
  documents that RongYuan-firmware magnetic-switch keyboards (MonsGeek M1 V5 HE, Akko
  MOD007B-HE — the `RY5088`/`YC3121` chipset family) use an **Artery AT32F405**
  (STM32-compatible ARM Cortex-M4) as the main keyboard MCU, with a **PAN1080**
  (Panchip BLE SoC) for wireless. The project documents SWD debug pinouts, a ROM DFU
  recovery procedure, Flash Access Protection notes, and ships an "unbrick" tool — proof
  this MCU family is real, documented silicon with a known hardware recovery path, and
  that a third party has already run custom firmware patches on it.
  - **This is not proof for the TK68.** The MonsGeek/Akko board is newer, wireless, and
    uses magnetic (Hall-effect) switches — a different, higher-end product than our
    wired-only TK68. The TK68 could share the same AT32F4 chip, use a smaller/cheaper
    AT32 part, or use something unrelated. Treat "AT32F4-ish" as a hypothesis, not a fact.
- **The vendor app's main-firmware update path** (EPOMAKER Driver v2.1.92, installed
  unpacked — no `app.asar`, so `resources/app/main_dist/main.js` and
  `resources/app/dist/static/js/main_c38c7efc.js` are plain webpack-bundled, minified
  JS). The TK68's device-handler class doesn't override the base `upgrade()` method, so
  this generic flow is what would run for our board. Reconstructed in our own words:
  1. **Enter bootloader**: send a 64-byte HID feature report, `cmd = 0x7F`
     (the app's own constant is `FEA_CMD_SET_BOOTLOATER` — sic, their typo), payload
     `55 AA 55 AA 00 00`, CK7 checksum. The checksum byte the app sends (`0x82`) is
     exactly what `docs/PROTOCOL.md`'s CK7 formula (`0xFF - sum(b[0..6])`) produces for
     these bytes — a nice cross-check that this reconstruction and our existing protocol
     notes agree.
  2. The board disconnects and **re-enumerates as a separate bootloader HID device**,
     PID `0x4001` (16385), under VID `0x3151` (12625 — Rongyuan's yc300/yc3121-family
     VID) or, for some older/other boards, VID `0x0461` (1121). The bootloader interface
     may present as the normal keyboard collection (usage page `0x01`, usage `0x06`) or
     as an additional vendor-defined interface (usage page `0xFF01`) — the app's own
     device filter accepts either. It polls for up to ~50 s for this device to appear.
  3. It pings the bootloader with `0xBA 0xFF` (checksum NONE) and expects back
     `0xAB 0xFF` — a different acknowledgement convention than the config channel's
     `cmd | 0x80`, consistent with this really being separate bootloader-resident code,
     not just a mode flag inside the normal firmware.
  4. The firmware image file has its **first 64 KB stripped** before flashing
     (`payload = file.slice(65536)`) — likely a vendor packaging/container header,
     though it could also reflect a reserved bootloader region; unconfirmed which.
     (Other, newer device classes in the same app strip only 20 KB for "usb"-method
     updates, so this offset is per-platform, not a universal constant.)
  5. The remaining payload is sent as raw 64-byte HID feature-report writes, checksum
     type **NONE**, with no page-number header byte (unlike the documented paged writes
     for pictures/macros/key maps in `docs/PROTOCOL.md`).
  6. A "start" command (`0xBA 0xC0` + page count + payload length) opens the transfer;
     a "finish" command (`0xBA 0xC2` + page count + a plain 32-bit sum of every payload
     byte + payload length) closes it — this checksum is a simple additive sum, not the
     CK7/CK8 style used elsewhere. Both are read back with a GET.
  7. **Firmware images are not bundled** in the installed app — they're downloaded on
     demand from Epomaker's cloud (`/get_fw_version?dev_id=…`) as a zip containing
     `firmwareFile.bin` (the "usb"/main-MCU method used by our board),
     `firmwareRFFile.bin`, `firmwareOledFile.bin`, `firmwareMledFile.bin`,
     `firmwareNordicFile.bin`, `firmwareFlashFile.bin` depending on method. The app's UI
     copy warns that picking the wrong firmware for your exact model needs factory
     repair, and its recovery flow for a failed update still asks the user to enter a
     "device ID" to re-fetch firmware from that same cloud service — **there is no fully
     offline vendor recovery tool bundled locally.**
- **Existing QMK support for "Epomaker" is on unrelated hardware.** `qmk/qmk_firmware`'s
  `keyboards/epomaker/` directory contains only
  [`tide65`](https://github.com/qmk/qmk_firmware/blob/master/keyboards/epomaker/tide65/keyboard.json),
  whose `keyboard.json` declares `"processor": "WB32FQ95"` (a WestberryTech WB32 ARM
  Cortex-M3) and `"manufacturer": "HS"` — a different ODM, different chip family,
  different bootloader (`wb32-dfu`) than anything Rongyuan/yc300-related. Epomaker's
  TH40/Luma40/Galaxy68 are marketed with QMK/VIA branding too, but we found no
  confirmation either way whether they're Rongyuan-based — treat as unknown. **None of
  this covers the TK68 or the yc300 family.**
- Keylume's own protocol coverage is already broad: `crates/keylume-proto` implements
  every command documented in `docs/PROTOCOL.md` (LED, side strip, key map/Fn map,
  macros, user picture, debounce, report rate, sleep timers, keyboard options, profile
  switch). There isn't much "shallow protocol" headroom left to mine.

## Other boards on the platform

Read-only pass over the vendor app's own device table (EPOMAKER Driver v2.1.92,
installed unpacked, no `app.asar`) to see how many other keyboards it drives and how
they relate to the TK68. The table isn't a separate data file — it's ~1,550 JS object
literals baked into the renderer bundle, `resources/app/dist/static/js/main_c38c7efc.js`.
Extracted with a throwaway script into a summary JSON (path at the end of this section);
nothing vendor-authored was copied into this repo.

**EPOMAKER Driver is a white-labelled, multi-brand OEM tool, not Epomaker-exclusive
software.** Across every keyboard-shaped entry in the table there are **1,551 model
codenames from roughly 290 distinct brand strings** (akko, AJAZZ, MonsGeek, Keydous,
HEXGEARS, Fantech, YUNZII, Darmoshark, ABKO, 腹灵/Fuling, and so on), all served by a
handful of shared chip-platform drivers. Epomaker is one customer of whoever built this
app — some device-table entries use `company:"rongyuan"` for what look like bare
reference-design boards, e.g. `YZWCommon`'s display name is "ROYUAN108", the same
"ROYUAN" string as the TK68's own manufacturer string (`docs/PROTOCOL.md`).

### Families / chip platforms

The device table is split into separate arrays in the bundle, one (or one merged pair)
per platform generation, matching the model-name prefix before the first `_`:

| Family (name prefix) | Keyboards | Brands | Epomaker models found | Protocol status |
|---|---|---|---|---|
| **`yc300`** — the TK68's family | 179 | 63 | 5: TK68, EP108, Skyline 87, TH-21, th96 | **Verified** (this board) |
| `yc200` | 45 | 25 | 0 | Unverified guess — co-listed in the exact same array as `yc300`, same field shape, never captured |
| `yc3016` / `yc3016a` | 27 | 15 | 0 | Unverified |
| `yc400` (+ a few stray `yc300`/`yc200`/`yzw` names) | 11 | 7 | 0 | Unverified |
| **`yc3121` + `yc500`** (one merged array) | 843 | 192 | 29 models: RT100 (8 regional/SKU variants), TH-21/66/68/80/80SE/80X/80ISO/96/98, EP21/64/68/75pro/84/84pro/84Plus, DynaTab 75/75X, KF850 UK/KF850-P UK, HE75 Mag, Aura75, DS87, RT65/80, Cypher 81/96, Shadow-X | Unverified. `yc3121` alone has roughly 150 Hall-effect (`_hall`) SKUs, so this isn't purely "the newer OLED family" — it spans normal mechanical and magnetic switches both |
| **`ry5088` + `pan1086` + `yc3123`** (one merged array) | 241 | 58 | 7 models: HE68 Lite, HE68/HE75/HE65 Mag, HE65 Mec, Epomaker M65, Epomaker Cypher 81 | Unverified — the family `monsgeek-akko-linux` (main Sources, below) documents an AT32F405 for, but on a *different* brand's board |
| `yzw` | 166 | 73 | 9 models (12 table rows, some as regional SKUs): TH100, EP87, EP84, EP68, TH66/68/80/98, Eclair 75 | Unverified |
| **`k68`** — a distinct platform that also squats on the TK68's exact USB ids | 33 | 20 | 0 | Unverified, see "Identity collisions" below |
| `bk100` | 6 | 2 (MonsGeek, akko) | 0 | Unverified |

Two more arrays exist in the same table but aren't keyboards: a 227-model, 40-brand
mouse table and a 16-model wireless-dongle ("dangle") table. Out of scope for this pass.

Only **`yc300` is verified** — the only family checked against real traffic from an
actual board (ours, `docs/PROTOCOL.md`). Every other family is a same-shape guess.

### The TK68's family (`yc300`): what's shared, what differs

All 179 `yc300` entries declare `featureReportByteLength: 65` (64-byte feature report
plus the report-id byte Windows exposes — matches `docs/PROTOCOL.md`'s report shape
exactly) and are instantiated by a `switch (name) { case "yc300_...": return new
<PerModelClass>(e) }` factory in the same bundle: every model gets its own small handler
class. Per this doc's *Firmware version* note, the TK68's class doesn't override the
shared `upgrade()` method — good evidence a common base class carries the shared command
set, with per-model classes overriding only what's different. What the table itself
shows varying, per model:

| Field | Meaning | Values across the 179 `yc300` models |
|---|---|---|
| `layer` (→ onboard profiles / picture layers) | key-map & picture slots stored on the board | 3 (49 models, incl. TK68), 4 (106, mostly akko), 8 (2: `yc300_dz61`, `yc300_hs_k61`), unspecified (22) |
| `fnLayer` | separate Fn key-map layers | 1 (125, incl. TK68), 2 (51, mostly akko), unspecified (3) |
| `logoLayout` | vendor UI shows a side-light tab | present on 15 of 179 |
| `company` | brand | 63 distinct; akko alone is 49 of the 179 |

Key count and matrix size (slot count) are **not in this table at all** — see "Where
per-model layout and behaviour data live" below. `docs/PROTOCOL.md`'s 128-slot,
column-major table is specific to the TK68's own physical layout; a 61-, 75-, 87- or
108-key sibling would need its own slot count and matrix, unrelated to this table.

The TK68 itself is a live counter-example for trusting `logoLayout`: its own entry lacks
it, yet the real board answers the side-strip command (0x08/0x88) anyway
(`docs/PROTOCOL.md`), and Keylume's own `boards/epomaker-tk68.json` sets
`"sideLight": true` on that basis. So `logoLayout` is a weak hint, not proof, in either
direction — only a real 0x88 read on the specific model settles it.

A compact sample (the full 179-row list with vid/pid/brand/profiles/Fn-layers/side-light
hint is in the JSON, path below):

| Model (vendor name) | Display name | Maker | USB ids | Profiles | Fn layers |
|---|---|---|---|---|---|
| `yc300_acr68pile` | TK68 | EPOMAKER | `05AC:024F` | 3 | 1 |
| `yc300_ep108` | EP108 | EPOMAKER | `3151:4003`, `25A7:2420` | 3 | 1 |
| `yc300_skyline87` | Skyline 87 | EPOMAKER | `3151:4003`, `25A7:2420` | 3 | 1 |
| `yc300_th21_dm` | TH-21 | EPOMAKER | `0461:4003` | 3 | 1 |
| `yc300_th96` | th96 WAP | EPOMAKER | `0461:4003`, `25A7:2420` | 3 | 1 |
| `yc300_5108s_dm` | 5108S | akko | `0461:4003` | 4 | 2 |
| `yc300_pc75s_dm` | PC75S | akko | `0461:4003` | 4 | 2 |
| `yc300_rs6` | RS6 | RS6 | `25A7:2420`, `3151:4003` | 4 | 1 |
| `yc300_sk6` | SK6 | 比乐 (Bile) | **`05AC:024F`** | 3 | 1 |
| `yc300_abko_ar75` | ABKO AR75 | ABKO | **`05AC:024F`** | 4 | 1 |
| `yc300_ac067_dm` | AC067 | AJAZZ | **`05AC:024F`** | 4 | 1 |

(RS6, 5108S, SK6 and PC75S were already named as examples in this doc's *What we know*
section; EP108 is now confirmed as Epomaker's own, the other four are cross-brand
siblings, not Epomaker's.)

### Identity collisions (why matching only on VID:PID is not enough)

Reusing Apple's `05AC:024F` isn't a one-off TK68 quirk: **52 of the 179 `yc300` models
declare the exact same ids**, across at least a dozen brands (akko, ABKO, AJAZZ, 比乐,
腹灵, DAGK, Sky, XIBERIA, protoarc, DAXA, Fantech, STOGA, 叠韵创新, Qeeke, acer, ICHUAN…).
Worse, the **`k68` family is a different platform entirely** (a different array, different
handler classes in the bundle) that *also* reuses `05AC:024F` on 12 of its 33 models
(ARDORGAMING "Hunter", 比乐 "SK1", rongyuan's own "TK568" reference board, several 腹灵
boards, a bare `dk2017`…) — so the same USB ids alone can mean at least two
protocol-incompatible keyboard families from over a dozen brands, on top of the small
risk of a real Apple keyboard. Separately, **the display name "TK68" isn't unique
either**: `yc3121_tk68_soc_hall` is a Hall-effect board from brand `gamakay2`, ids
`3151:4011`/`3151:4015`, 8 onboard profiles — an unrelated product on a newer platform
that happens to share Epomaker's model name.

This is exactly why `keylume-device::identity` already requires interface + usage page/
usage + manufacturer + product string + an exact HID report-descriptor match (one
64-byte feature report, no report ids) before Keylume writes anything
(`crates/keylume-device/src/identity.rs`, `docs/PROTOCOL.md`) — ids alone were already
known to be insufficient for telling a TK68 from a real Apple keyboard; this pass shows
they're insufficient for telling it from dozens of *other vendors'* keyboards too, on at
least two different chip platforms.

### Other brands

Epomaker has products in four of these platform groupings — `yc300`, `yc3121`+`yc500`,
`ry5088`+`pan1086`+`yc3123`, and `yzw` — and none found in `yc200`, `yc3016`/`yc3016a`,
`yc400`, `k68` or `bk100`. Supporting more Epomaker models later has real,
table-confirmed candidates well beyond the `yc300` family, though only `yc300` is
protocol-verified today. Akko is the single biggest brand within `yc300` itself (49 of
179 models) — of the many brands sharing this platform, Akko looks like the deepest
catalogue after Epomaker's own selection.

### Where per-model layout and behaviour data live

Two structures in the same renderer bundle, found by searching for the TK68's model name
(`yc300_acr68pile`), neither of them the `layout:` *field* in the device-table entries
above (that field is a **lighting-effects capability descriptor** — which effects exist,
their speed/brightness ranges, whether "dazzle"/rainbow applies — not a physical layout,
a naming false-friend worth flagging so nobody else wastes time on it):

1. A name-keyed lookup object, e.g. `{..., yc300_acr68pile: {layout: _k.layout, delt:
   {deltX: _k.deltX, deltY: _k.deltY}}, ...}`, where `_k` is a per-model imported
   sub-module. This is almost certainly the on-screen keyboard picture's physical key
   geometry (`delt` reads like a rendering offset). The string `defaultMatrix` — the same
   term Keylume's own `layouts/epomaker-tk68.json` uses for its 128-slot table — occurs
   1,509 times in the same bundle, consistent with each per-model sub-module carrying its
   own matrix table in that shape.
2. A `switch (name) { case "yc300_acr68pile": return new $ye(e); ... }` factory
   instantiating one handler class per model — where model-specific protocol behaviour
   (e.g. the `upgrade()` override question from this doc's *What we know* section) would
   live, if a model has any.

Neither was parsed in depth: per-model geometry and matrix tables for ~180 `yc300`
models is a lot of vendor-authored data, and doing so would go well past "reading a
device table." Nothing from either structure was transcribed. **A clean-room board
definition for any sibling model must be independently re-derived by someone who owns
that physical board** — checking real key positions and capturing its actual HID
picture/key-map writes to confirm slot order — the same way `layouts/epomaker-tk68.json`
was built for the TK68, not by trusting or adapting anything read out of the vendor's
bundle.

### Adding a model: the board-definition fields

Keylume already has the generic shape this needs (`crates/keylume-device/src/boards.rs`,
one file bundled today: `boards/epomaker-tk68.json`). A new model needs:

| `BoardDef` field | Where a vendor-table fact helps | Must be re-verified on real hardware |
|---|---|---|
| `usb.{vid,pid,interface,usagePage,usage}` | table gives `vid`/`pid`/`usage`/`usagePage` directly | interface number, and — given the collisions above — **always** the strings below too |
| `usb.manufacturer` / `usb.products` | not in this table at all | read from the real device (`keylume-cli list`) |
| `usb.featureReport` | table's `featureReportByteLength` minus 1 (65→64) | confirm via the HID report descriptor, not just the table |
| `layout` (→ `layouts/<id>.json`) | table gives no key count or matrix | built from scratch per model: physical key positions + matrix slot order, checked against real picture/key-map reads |
| `features.onboardProfiles` / `pictureLayers` | table's `layer` is a strong hint | confirm the picture command's layer nibble actually goes that high |
| `features.sideLight` | table's `logoLayout` is a weak hint (the TK68 itself contradicts it) | only a real 0x88 read settles it |
| `features.macros`, `.keymap`, `.settings`, `.backup`, `.readBack`, `.effects` | assume "same as TK68" for a same-family model, since the shared base class suggests a common command set | confirm each command actually works before shipping |
| (no current field) Fn-layer count | table's `fnLayer` is sometimes 2, not 1 | **`Features` has no field for this yet** — a model with `fnLayer: 2` would need a small schema addition before it could be supported correctly |

### Safe verification steps for a model's owner

Only the owner of a candidate board, with the board in hand, should do this:

1. **Backup first**: `keylume-cli backup` (pictures, all onboard key-map profiles, Fn
   layer, macros) before anything else, on whatever firmware the board currently runs.
2. **Identify, read-only**: `keylume-cli list` — confirms vid/pid/interface/usage, reads
   (never writes) the manufacturer/product strings and the HID report descriptor, and
   says in words whether it matches an existing board file or none.
3. Draft a `boards/<candidate>.json` with `"support": "experimental"` from the fields
   above, and a matching `layouts/<candidate>.json` built from the owner's own
   measurements of their physical keyboard, not from anything in this section.
4. Verify read commands only at first (firmware version `0x80`, LED register `0x87`, a
   picture layer `0x8C`) before ever attempting a write, respecting the timing table in
   `docs/PROTOCOL.md`.
5. If `keylume-cli list` reports an identity mismatch (wrong strings, wrong descriptor,
   more than one candidate device), stop — that is Keylume correctly refusing an
   unverified board, not a bug to work around.

JSON summary (model, vid/pid, family, company, onboard profiles, Fn layers, side-light
hint — 1,551 rows, covering every keyboard-type array in the vendor's table) was made
while researching this, and kept out of the repository. Every candidate still needs a real
owner and real hardware before it becomes a supported board.

## 2. Open questions

| Question | What would settle it |
|---|---|
| Exact MCU part number and package | A photo of the chip's top-side markings with the case open (Experiment 1) |
| Is it an Artery AT32F4 (like the sibling RY5088/YC3121 board) or something cheaper/older | Chip markings; failing that, comparing a captured firmware image's vector table/entry code against known AT32/STM32 Cortex-M signatures |
| Does the TK68 PCB expose SWD test points | Visual inspection once the case is open (Experiment 1) — usually 4-6 pads near the MCU, sometimes unlabelled |
| Does the reconstructed HID-bootloader-entry protocol (above) actually work this way on our exact board, and does an aborted transfer leave it recoverable | Only findable on real hardware — high risk, see Experiment 4 |
| What the stripped 64 KB (or 20 KB, on other boards) header in the firmware file actually is | Capturing a real `firmwareFile.bin` (Experiment 3) and inspecting its structure |
| Whether the TK68's firmware shares code with the AT32F405-based sibling (making `monsgeek-akko-linux`'s findings partly reusable) | Comparing a captured TK68 firmware image against public AT32F4 vector tables/HAL fingerprints |

## 3. Experiments on real hardware

Ordered safest/most-informative first. None of these has been run yet.

### 1. Chip ID by photo — risk: none
- **Goal**: settle the biggest open question (which MCU) without any HID traffic.
- **Backup**: none needed — purely visual, keyboard stays powered off/disconnected.
- **Steps**: unplug the keyboard, open the case, photograph the main PCB — especially
  the largest QFN/LQFP chip near the USB connector. Zoom in on any printed part number
  and date code. Note any 4-6-pad header near that chip (possible SWD).
- **What could go wrong**: physical damage from prying the case (a case-opening/teardown
  guide for this shell is worth checking first); no electrical risk.
- **Expected result**: a part number like `AT32F4xx`, or something else entirely.
- **Recovery**: not applicable.

### 2. USB descriptor + Device Manager capture — risk: none
- **Goal**: corroborate/refine the VID:PID/bcdDevice/strings already in
  `docs/PROTOCOL.md`; OEMs occasionally leak a build date or internal name in an unused
  descriptor field.
- **Backup**: none needed.
- **Steps**: Device Manager → the keyboard's HID entry → Details tab → "Hardware Ids"
  and "Device instance path"; or a USB descriptor dump tool (e.g. USBView). No
  `keylume-cli`, no feature-report writes at all — this is normal enumeration only.
- **What could go wrong**: nothing — this is read-only OS-level inspection, not talking
  to the config channel.
- **Expected result**: confirms `05AC:024F`, bcdDevice, and possibly an extra
  interface or serial string not yet in `docs/PROTOCOL.md`.
- **Recovery**: not applicable.

### 3. Capture a real vendor firmware update — risk: low (passive capture; the vendor app does the actual write, not us)
- **Goal**: verify the reconstructed bootloader-entry/transfer protocol above against
  real traffic, and get an actual `firmwareFile.bin` to inspect (is the 64 KB header
  real container metadata or an address-mapped region? does the image look like ARM
  Cortex-M code?).
- **Backup**: take a full `keylume-cli` backup of pictures, keymaps (all 3 profiles +
  Fn layer) and macros first, in case the real update changes onboard defaults. This
  does not protect against a bad flash — only against losing current profiles.
- **Steps**: run Wireshark with USBPcap (or a hardware USB analyzer) on Windows, then
  use the EPOMAKER Driver app's own "check/apply firmware update" feature — **only if
  a real update is actually offered; never force one you don't need.** Capture the HID
  traffic and the downloaded zip.
- **What could go wrong**: this is the vendor's own update mechanism, run by their own
  app exactly as intended — the risk profile is whatever risk already exists in using
  that app normally, not something this capture adds. If their update itself fails,
  that's a pre-existing vendor/board risk, unrelated to Keylume.
- **Expected result**: byte-for-byte confirmation (or correction) of the `0x7F` enter-
  boot and `0xBA 0xC0`/`0xC2` transfer protocol reconstructed above, plus a real
  firmware image to inspect offline.
- **Recovery**: not applicable to us — we're only observing.

### 4. Enter the vendor bootloader and probe it read-only — risk: HIGH — do not attempt before 1-3, and only with backups and a recovery plan in hand
- **Goal**: confirm the board actually re-enumerates at PID `0x4001` as reconstructed,
  touching nothing beyond a single ping.
- **Backup**: full `keylume-cli` backup of pictures, all 3 keymap profiles + Fn layer,
  and macros. Record the exact stock firmware version (`0x0304`) first (`0x80` read).
  Only attempt this with a spare board or real risk tolerance — see "what could go
  wrong" below.
- **Steps**: with Keylume and the vendor app closed, send the single feature report
  `7F 55 AA 55 AA 00 00 82` (CK7, matches `docs/PROTOCOL.md`'s checksum formula) with
  `keylume-cli raw` or another raw HID tool (check how the tool fills in checksums
  first). Watch for a new HID device at VID `0x3151` or `0x0461`, PID
  `0x4001`. If it appears, send only the `0xBA FF` ping and read the reply. **Do not**
  send the start/data/finish sequence — that would write flash with no known-good image
  to write and no verified recovery path.
- **What could go wrong**: if the board enters the bootloader and doesn't return to
  normal firmware on replug (e.g. because the bootloader is waiting for a transfer that
  never completes, or because entering it corrupts some non-flash state), the board is
  bricked with **no currently known independent recovery path** — we don't yet know the
  MCU (see section 2), so there's no confirmed SWD/ISP fallback the way
  `monsgeek-akko-linux` has for the AT32F405 sibling board.
  If it never re-enumerates as a bootloader device at all, that's actually the *safe*
  failure — it just means this board doesn't respond to `0x7F` the way the app's code
  suggested, or needs an interface we didn't try.
- **Expected result**: keyboard drops out; a new bootloader HID interface appears; a
  `0xBA FF` ping returns `0xAB FF`.
- **Recovery**: unplugging/replugging should return the board to normal firmware if
  nothing was written to flash — this is the standard behaviour of a "wait for a
  transfer" ISP bootloader. If it doesn't, there is no known recovery for this exact
  chip today; **do not run this experiment until Experiment 1 has identified the MCU**
  and, ideally, a documented SWD/ISP recovery method for that exact part is in hand.

## 4. Options for the brain

- **Deeper use of the stock firmware's own commands** — little headroom left.
  `crates/keylume-proto` already implements every command documented in
  `docs/PROTOCOL.md`. This track is essentially done at the "shallow protocol" level.
- **Vendor firmware-update format, for update/rollback tooling in Keylume** — medium
  effort, currently blocked on data. We'd need a real captured `firmwareFile.bin`
  (Experiment 3) to know whether the container is trivially parseable, whether it's
  signed or otherwise validated by the bootloader (nothing in the app's JS suggests
  cryptographic signing — the "finish" checksum is a plain additive sum — but that's not
  proof the firmware itself checks nothing), and whether replaying it independently of
  the vendor app is safe. Even with that data in hand, shipping a "flash your keyboard"
  feature in Keylume is a large responsibility (bricking risk) for uncertain benefit —
  a decision to take deliberately before more work goes here.
- **Open firmware (QMK/ZMK) port** — not realistic yet; effort is dominated by the
  unknowns in section 2. If Experiment 1 confirms an Artery AT32F4 (matching the sibling
  RY5088/YC3121 board's AT32F405), the picture improves substantially: AT32F4 is
  STM32-register-compatible, Artery publishes datasheets, and `monsgeek-akko-linux` is a
  working precedent for running custom code on a sibling chip via SWD — worth studying
  (and possibly reaching out to its authors) before attempting anything similar here. If
  the chip turns out to be an unrelated part with no public datasheet and no SWD access,
  an open-firmware port isn't practical, and the project should stay on the HID-protocol
  track for the foreseeable future.
- **Independent bricked-board recovery tooling** (not dependent on Epomaker's cloud) —
  only possible once the MCU is confirmed and, ideally, once SWD test points are found
  (Experiment 1). Until then, Keylume has no fallback if a firmware experiment goes
  wrong beyond a warranty claim.

## 5. Sources

- `docs/PROTOCOL.md` (this repo) — existing clean-room HID protocol notes, cross-checked
  against the vendor app's own checksum and enter-boot bytes in this research pass.
- EPOMAKER Driver v2.1.92, installed locally at
  `%LOCALAPPDATA%\Programs\EPOMAKER Driver` — `resources/app/package.json`,
  `resources/app/dist/static/js/main_c38c7efc.js` (renderer bundle, also the source of
  the full cross-brand device table behind "Other boards on the platform" above),
  `resources/app/company/company_EPOMAKER/` (device table images, one per model). Read
  and reverse-engineered in this document's own words; nothing copied into the repo. The
  extracted device-table summary (1,551 models) was kept out of the repository.
- [Rongyuan (容圆科技) — About Us](https://rongyuan.tech/aboutus.html) — company's own
  MCU/SoC lineup.
- [YC3121-D — YICHIP, JLCPCB parts library](https://jlcpcb.com/partdetail/YICHIP-YC3121D/C2916800)
- [`monsgeek-akko-linux`](https://github.com/AuriSolCorues/monsgeek-akko-linux)
  (GitHub, GPL-3.0) — independent reverse-engineering of a related RongYuan
  RY5088/YC3121-firmware board (MonsGeek M1 V5 HE / Akko MOD007B-HE): documents an
  AT32F405 main MCU + PAN1080 wireless SoC, SWD pinout, ROM DFU recovery, and an
  "unbrick" tool.
- [`qmk/qmk_firmware` — `keyboards/epomaker/tide65/keyboard.json`](https://github.com/qmk/qmk_firmware/blob/master/keyboards/epomaker/tide65/keyboard.json) —
  shows Epomaker's existing mainline QMK support is on unrelated WB32FQ95 hardware, not
  the yc300/Rongyuan family.
- [DeviceHunt — USB 05AC:024F](https://devicehunt.com/view/type/usb/vendor/05AC/device/024F)
  and [USB ID Repository — 05ac:024f](https://usb-ids.gowdy.us/read/UD/05ac/024f) —
  confirms `0x05AC:0x024F` is officially an Apple keyboard ID, reused here.
