# TK68 (Rongyuan "yc300" family) HID protocol

Clean-room notes from observing traffic between the vendor app and the keyboard
(packet capture + black-box testing with hidapi). Verified on an Epomaker TK68,
firmware 0x0304 (772).

## Device

| | |
|---|---|
| VID / PID | `0x05AC` / `0x024F` (USB wired) |
| Interface | 0, usage page `0x01`, usage `0x06` (the keyboard collection) |
| Transport | HID **feature reports**, report ID `0`, 64-byte payload (65 with ID) |
| Name strings | product "Acrylic68", manufacturer "ROYUAN" |

Only one host program should drive the config channel at a time; interleaved
traffic garbles replies (you get string-descriptor bytes back). Replug resets it.

**The ids aren't the board's own.** `05AC:024F` is Apple's vendor id and the product id of
Apple's wired Aluminium Keyboard, so matching the ids proves nothing. Before anything is sent,
Keylume also requires interface 0 with the keyboard collection, the strings above (compared
ignoring case), and a HID report descriptor with exactly one feature report of 64 bytes and no
report ids (`keylume-device::identity`); it refuses when more than one device passes. All of
that is read from the OS, not the keyboard. Its last check, before it keeps the keyboard, is
that the firmware answers the version query. `keylume-cli list` shows each device with the
ids and why it was or wasn't accepted.

## Timing (important!)

The firmware is single-threaded and slow to persist settings. Get this wrong and the
config channel wedges: SETs stall for Windows' 5 s timeout (`os error 121`) or fail
with `os error 31`, sometimes for minutes. Typing is never affected.

| After… | Wait before the next SET | Why |
|---|---|---|
| a config write (LED 0x07, report rate, debounce, sleep, profile) | **~1.1 s** (0.7 s fails, 1.0 s passes) | firmware saves the setting |
| a picture or macro paged write | **1.5 s** after the last page | flash commit |
| a key-map / Fn-map paged write | **3 s** after the last page (1.5 s wedges) | larger flash commit |
| a read (SET request + GET reply) | nothing | – |
| pages of one paged write | 20 ms between pages | – |

- A **bare GET_FEATURE** (no preceding SET) clears a wedged channel. Do that, wait
  ~300 ms, retry once.
- GET replies are the firmware's last buffer; a stale echo is *not* a readiness signal.
- The vendor app gets away with this because its helper blocks on every request and
  sleeps 1000 ms (`BIGCMDDELAY`) after config writes.
- Only one program may use the channel at a time. Close the vendor driver first.

## Onboard profiles (gotcha)

The TK68 has **3** onboard key-map profiles (vendor device table: `layer: 3`, plus one
Fn layer). Key-map **reads** honour the profile byte (`89 <profile> <page>`), but
**writes ignore it and always land in the active profile**. To edit profile N:
switch to N (`05`), write, switch back. Writing a nonexistent 4th profile stalls the
firmware for ~20 s and is discarded.

## Framing

Every command is a 64-byte buffer `b`. `b[0]` is the command. Two checksum styles:

- **CK7** (vendor enum `BIT7 = 0`) — `b[7] = 0xFF - (sum(b[0..6]) & 0xFF)`, header
  only. Used by all reads, the small setters and every paged-write header.
- **CK8** (`BIT8 = 1`) — `b[8] = 0xFF - (sum(b[0..7]) & 0xFF)`. Used by the LED
  command (0x07) because bytes 5–7 carry RGB.
- (`NONE = 2` — the vendor app sometimes computes CK7 itself and sends with NONE.)
- Paged bulk writes (key map 0x09, Fn map 0x10, user picture 0x0C, macro 0x0B)
  carry data at `b[8..63]` (56 bytes per page); the checksum covers the header only.

**Read** = send `[cmd|0x80, ...]` with CK7, short delay (~30 ms), then
`GetFeature(report 0)`. Reply echoes the command in `b[0]` for single-value reads;
paged reads (0x89, 0x8C, 0x8B, 0x90) return 64 raw data bytes with no header.

## Commands

| Cmd | Read | Purpose | Layout |
|---|---|---|---|
| 0x80 | yes | firmware version | reply `b[1..2]` u16 LE |
| 0x02 | – | factory reset | – |
| 0x04 / 0x84 | yes | report rate | `b[1]`=profile, `b[2]` code: 1=1000 Hz, 2=500, 4=250, 8=125 |
| 0x05 / 0x85 | yes | active onboard profile | `b[2]` = profile (0-based) |
| 0x06 / 0x86 | yes | keyboard options (win-lock, WASD swap, …) | bit-field, see below |
| 0x07 / 0x87 | yes | LED effect | see **LED** (CK8) |
| 0x08 / 0x88 | yes | side light strip | see **Side strip** (CK8) |
| 0x09 / 0x89 | yes | key map (profile) | paged, see **Key map** |
| 0x0B / 0x8B | yes | macro | paged, see **Macros** |
| 0x0C / 0x8C | yes | user picture (per-key RGB) | paged, see **User picture** |
| 0x10 / 0x90 | yes | Fn-layer key map | paged, same as key map |
| 0x11 / 0x91 | yes | debounce | `b[2]` = value (ms-ish units) |
| 0x12 / 0x92 | yes | sleep timers | `b[8..15]` = 4× u16 LE seconds: bt, 2.4G, deep-bt, deep-2.4G |

### LED (0x07, CK8)

```
07  mode  (4-speed)  brightness  flags  R  G  B  [ck8]
flags = (direction << 4) | (rainbow ? 8 : 7)
speed 0..4, brightness 0..4
```

| mode | effect (vendor UI name) | directions |
|---|---|---|
| 00 | off | – |
| 01 | Always On | – |
| 02 | Dynamic breathing | – |
| 03 | Spectrum cycle (rainbow only) | – |
| 04 | Drift (wave) | 0 right, 1 left, 2 down, 3 up |
| 05 | Waves ripple (reactive) | – |
| 06 | Stars twinkle (raindrop) | – |
| 07 | Steady stream (snake) | 0 z, 1 return |
| 08 | Shadowing (press action) | – |
| 09 | Peaks rising (converge) | – |
| 0A | Sine wave | – |
| 0B | Caispring surging (kaleidoscope) | 0 out, 1 in |
| 0C | Flowers blooming (line wave) | 0 right, 1 left |
| 0D | **User picture** | `flags` = layer<<4 (0,1,2); speed byte 4; RGB ignored |
| 0E | Laser | – |
| 0F | Peak turn (circle wave) | 0 anti-cw, 1 cw |
| 10 | Colorful vertical/horizontal (dazzling) | – |
| 11 | Snow (rain down) | – |
| 12 | Meteor | – |
| 13 | Through the snow (press action off) | – |
| 14 / 16 | Music follow 1/2 (host streams audio levels) | 0 upright, 1 separate, 2 intersect; flags low nibble 4 (or 0 = rainbow) |
| 15 | Screen colour (host streams colour) | – |

### Side strip (0x08 write / 0x88 read, CK8)

The vendor app calls it "SLED" (`FEA_CMD_SET_SLEDPARAM = 8`) and only shows it for
boards whose device entry has a `logoLayout`; the TK68's entry doesn't, but its
firmware implements the command. Same layout as the LED command, with its own modes:

```
08  mode  (4-speed)  brightness  flags  R  G  B  [ck8]
flags = (direction << 4) | (rainbow ? 8 : 7)
```

| mode | effect | directions |
|---|---|---|
| 0 | off | – |
| 1 | always on | – |
| 2 | breathing | – |
| 3 | neon (rainbow cycle) | – |
| 4 | wave | 0 right, 1 left, 2 down, 3 up |
| 5 | snake | 0 zigzag, 1 return |

Factory state on the test board: `88 03 00 04 01 ff ff ff` (neon). Writes read back
verbatim and leave the key lighting untouched. It's a config write: ~1.1 s settle.

### What Keylume can see

- **Readable:** the LED register (0x87: mode, speed, brightness, direction, rainbow, colour),
  the side strip (0x88), each picture layer (0x8C), key maps, macros and settings. Keylume
  reads the LED register and the showing picture layer whenever a keyboard connects, and after
  a restore or a reset, so Home starts from what the keyboard really shows.
- **Not readable:** where a firmware animation is in its cycle (no phase or frame counter is
  exposed), and which keys the reactive effects are lighting. Home's pictures of firmware
  animations are therefore approximations, labelled as such, and show no key presses.
- **Known exactly:** anything Keylume streams (0x0D bars, 0x0E whole-board colour): the frames
  Home plays are the ones sent, in the colours they were designed with (True colours is applied
  on the way to the keyboard only).
- Faking synchronisation by rewriting pictures in a loop is ruled out: each picture is a flash
  commit (*Timing*).

### User picture (0x0C write / 0x8C read)

Frame = 128 slots × RGB = 384 bytes, **column-major, 6 slots per column** (row 0 is
unused on the TK68; rows 1–5 are the physical rows). Slot order = the key matrix
order below.

- Select the target layer first: LED command mode `0D`, flags `layer<<4`.
- Write: 7 pages. `0C 00 80 01 <page> 00 00 <ck7>` + 56 data bytes (`0x0180` = 384 LE total length).
- Read: `8C 00 <page>` (pages 0–5), reply = 64 raw bytes each.

### Key map (0x09 write / 0x89 read), Fn map (0x10 / 0x90)

Matrix = 128 slots × 4 bytes = 512 bytes, same column-major slot order.

- Write: 9 pages. `09 <profile> F8 01 <page> 00 00 <ck7>` + 56 data bytes
  (`0x01F8` = 504 LE; the last 8 bytes of the matrix are never sent).
  Fn map uses `10 <fnIndex> …` identically.
  Keylume reads the layer back after writing it (`keylume_core::board::write_keys`): a key
  the keyboard didn't keep is written once more, then reported, never shown as done.
- Read: `89 <profile> <page>` pages 0–7 (64 bytes each).

4-byte key action:

| bytes | meaning |
|---|---|
| `00 00 00 00` | disabled / default |
| `00 mod key key2` | plain key or combo (`mod` = HID code of Ctrl 0xE0 / Shift 0xE1 / Alt 0xE2 / Win 0xE3) |
| `01 00 code x` | mouse: F0 left, F1 right, F2 middle, F3 back, F4 forward, F5 wheel (x = +1/-1 scroll) |
| `02 81/82/83 00 00` | system power / sleep / wake |
| `03 00 lo hi` | consumer (media) usage, e.g. `CD 00` play/pause, `E9 00` vol+, `EA 00` vol−, `E2 00` mute, `B5`/`B6` next/prev |
| `08 00 n x` | profile switch (0 value, 1 +, 2 −, 3 loop, 4 set x) |
| `09 mode idx 00` | run macro `idx` (mode 0 repeat-N, 1 toggle, 2 while held) |
| `0A 01 00 00` | Fn key |
| `0D xx yy 00` | lighting control (Fn layer: brightness/speed/mode/colour) |

### Macros (0x0B write / 0x8B read)

Buffer = 256 bytes: `u16 LE repeatCount`, then events, terminated by 4 zero bytes.
Event: `code, flags[, delayLo, delayHi]` where `flags` bit 7 = key down.
If `1 ≤ delay ≤ 127` it is packed into `flags` low 7 bits (2-byte event); otherwise
flags low bits = 0 and a u16 LE delay follows (4-byte event). `code` 4..239 = HID
key; 240+ = mouse buttons; `0xF9` = mouse move (`F9 flags dx dy`).

- Write: 5 pages. `0B <idx> 00 01 <page> 00 00 <ck7>` + 56 data bytes.
- Read: `8B <idx> <page>` pages 0–3.

## Key matrix (TK68, slot → HID usage)

Slots are column-major, 6 per column: `slot = col*6 + row`. Examples:
col 0 = `-, Esc, Tab, Caps, LShift, LCtrl`; col 1 = `-, 1, Q, A, (ISO key), -`;
col 5 = `F4 (phantom 0x3D), 5, T, G, V, -`. Slot 59 is `Fn` (`0A 01 00 00`).
The full 128-slot table plus every key's position is in
`layouts/epomaker-tk68.json` (`defaultMatrix`, `keys[].slot`).

**Hidden LEDs.** The board is shared with the ISO version, and two ISO keys keep a
matrix slot on the ANSI TK68: slot 10 (ISO `\|`, under the left Shift cap, right below
A) and slot 75 (ISO `#`, under the left half of the Enter cap). The factory pictures
give both slots their own colours, so the LEDs are most likely fitted. Anything else
that writes a picture should light them too, or they stay dark (or keep an old colour)
beside Enter and A. Keylume gives them the colour of the key above them (`hiddenLeds`
in the layout file). On the test board a blue glow shows at those spots even
under a solid red firmware effect, and it stayed after `keylume-cli hidden-leds --off`
turned the slots off in picture layers 1-2 (the factory pictures, saved first). So the
stored pictures aren't its source: either the firmware drives those LEDs some other way,
or the LEDs themselves are faulty. A look under the keycaps tells which.
