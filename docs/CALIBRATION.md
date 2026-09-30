# Checking True colours on a keyboard

**True colours** (Settings, on by default) assumes the keyboard's LEDs give out light in
proportion to the value they're sent, and converts every colour from the screen's curve
(sRGB) before it goes to the keyboard (`crates/keylume-core/src/color.rs`). That's the
theory, and it hasn't been measured on a real board. This check tells whether the keys now
look closer to the screen, and which way they're off if not. It takes about ten minutes
and changes nothing but the lighting.

## Before you start

- A dim room, the keyboard at full brightness, and the app window on a screen at its
  normal brightness. Colour-accurate screens make it easier, but any will do.
- Close the maker's app (Keylume pauses while it runs).
- Nothing needs backing up: the test design goes to the picture layer Keylume already uses
  for per-key designs, and any design from the Library puts things back.

## The test

1. **Create → Paint keys.** With the Fill tool and the colour field, make one row per
   colour: `#808080` (mid grey) on the number row, `#ff8000` (orange) on Q–P, `#ff4080`
   (pink) on A–L, `#80c0ff` (pale blue) on Z–M, and `#ffffff` (white) on the bottom row.
   Save it as "Calibration" so you can come back to it.
2. With **True colours on**, press **Show on keyboard**. Look from the keyboard to the
   screen and back for each row.
3. **Settings → True colours off**, then open the Calibration design again and show it on
   the keyboard. Compare each row again.
4. For each colour, note which setting looks closer to the screen, and if neither does,
   whether the keys look **darker/deeper** or **lighter/washed out** than the screen.
   A phone photo of each setting (same place, same exposure) helps, but trust your eyes
   over the phone.

## What the answers mean

| What you see | Meaning | Next step |
|---|---|---|
| Closer with True colours on, for most rows | The LEDs are close to linear | Keep it on; nothing to change |
| Closer with it off | The firmware already corrects colours | Turn it off by default, or drop it |
| On is better but mid-tones are now too dark | The LEDs are only partly linear | Try a gentler curve (an exponent near 1.8 instead of sRGB's 2.4) |
| Grey looks tinted (blue or pink) | The LED channels differ in strength | A per-channel balance would be needed; report which tint |

Please write down the firmware version (Home shows it) and what you saw, so a change can be
tested against it. Until someone has run this, the app and the docs only say that colours
come *closer* to the screen, not that they match it.
