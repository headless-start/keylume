# Your own profiles

A Keylume profile is plain JSON. Make one on the **Create** page, or write one by hand
and add it with **Library → Upload** (or drop the file anywhere on the window). Uploaded
profiles appear under *Mine*. Each gets a fresh `user-…` id, so an upload never
overwrites anything, and a file with one bad profile adds nothing and says why.

To share or sell your designs, put them in a **design pack**: Library → Mine → **Make a
pack** saves them as one signed file that anyone can add to Keylume ([PACKS.md](PACKS.md)).

## A profile file

A `.json` (or `.keylume.json`) file holds one or more profiles:

```json
{
  "keylume": 1,
  "profiles": [
    {
      "id": "",
      "name": "Sunset Keys",
      "category": "Mine",
      "tags": ["warm", "orange"],
      "description": "Orange letters, purple everything else.",
      "lighting": { "kind": "perKey", "brightness": 4, "keys": { "q": "#ff7a00", "esc": "#5a1a8a" } }
    }
  ]
}
```

A file containing a single profile object (without the `profiles` wrapper) is also
accepted, and a file can hold up to 500 profiles.

| Field | Required | Notes |
|---|---|---|
| `id` | no | ignored; a new id is assigned |
| `name` | yes | shown on the card (up to 60 characters) |
| `category` | no | the collection it's listed under (default `"Mine"`) |
| `tags` | no | used by search |
| `description` | no | shown on hover |
| `lighting` | yes | one of the four kinds below |

Colours are always `"#rrggbb"`. Brightness and speed go from `0` to `4`.

## Lighting kinds

### `perKey`: a colour for each key

```json
{ "kind": "perKey", "brightness": 4, "keys": { "esc": "#ff0000", "w": "#00c8ff", "space": "#ffffff" } }
```

Keys you leave out are dark. The key ids for the 68-key layout:

```
esc 1 2 3 4 5 6 7 8 9 0 minus equal backspace grave
tab q w e r t y u i o p lbracket rbracket backslash delete
caps a s d f g h j k l semicolon quote enter pgup
lshift z x c v b n m comma period slash rshift up pgdn
lctrl lwin lalt space ralt fn rctrl left down right
```

Per-key profiles are stored in one of the keyboard's onboard picture layers, and
writing one takes about 1.5 s.

### `effect`: one of the keyboard's built-in animations

```json
{ "kind": "effect", "effect": { "mode": "wave", "speed": 2, "brightness": 4, "direction": 0, "rainbow": true, "color": "#00c8ff" } }
```

`mode` is one of: `off static breathing spectrum wave ripple raindrop snake reactive
converge sine-wave kaleidoscope line-wave laser circle-wave dazzle rain-down meteor
reactive-off music-bars music-pulse`. Reactive modes (`reactive`, `ripple`, `laser`,
`reactive-off`) light up the keys you press. These effects run on the keyboard itself
and keep working when Keylume is closed.

### `live`: a real-time effect streamed by Keylume

```json
{ "kind": "live", "live": { "kind": "breathe", "colors": ["#ff0080", "#00c8ff"], "bpm": 12 } }
```

Live effects run only while Keylume is open. The keyboard's real-time channel sets the
whole board to one colour or drives 32 bars, so live effects can't set each key
separately. The available kinds and their fields are listed in
[`src/lib/types.ts`](../src/lib/types.ts) (`LiveEffect`). A few examples:

| kind | fields |
|---|---|
| `paletteFlow` | `colors: [..]`, `period` (s) |
| `heartbeat` | `color`, `bpm` |
| `breathe` / `rave` | `colors: [..]`, `bpm` |
| `morse` | `message`, `color`, `background`, `wpm` |
| `bars` | `style` (`plasma`, `fire`, `helix`, …), `color`, `rainbow`, `speed` |

### `spell`: words typed out on their own keys

```json
{ "kind": "spell", "brightness": 4, "background": "#020a3a",
  "words": [ { "text": "Hello", "color": "#00c8ff" }, { "text": "World", "color": "#ffb000" } ] }
```

Letters light up one at a time, in each word's colour, and the finished picture stays
on. A letter shared by two words glows white. Each letter costs a picture write
(about 1.5 s), so keep words short.

## Other keyboards with the same form factor

The profiles use key ids, not positions, so a profile works on any board with the same
68-key (65%) layout once Keylume supports that board. Only the TK68 is verified today.
Adding a board means giving it a layout (key ids with positions and firmware slots) and
testing it on real hardware; see [PROTOCOL.md](PROTOCOL.md) and the roadmap. Keys that
a board doesn't have are ignored.

## Growing the built-in catalogue

- **Themes** are the easiest way in: a theme is a set of colours with a name, written
  as JSON in `crates/keylume-profiles/themes/<collection>.json`. Each one becomes 41
  profiles (every pattern, one keyboard animation, one live effect). Check a file with
  `cargo run -q -p keylume-profiles --example themes -- <file>`; it explains anything
  it rejects.
- **Games** are theme files with `"section": "Games"` and `"keys"`: the game's default
  keys by role (`[{ "role": "movement", "keys": ["w", "a", "s", "d"] }, …]`, most
  important first). Every theme in them also becomes a *Game Keys* layout.
- **Flags** live in `crates/keylume-profiles/flags/<region>.json`: stripes plus simple
  shapes (canton, cross, saltire, disc, `triangle` at the hoist or with `"right": true`
  at the fly, diagonal, `band` for a stripe anywhere, `half` for one side of a diagonal,
  and `area` for shapes inside a rectangle placed with `x`/`y`/`w`/`h`, such as a
  canton, a quarter or an upward peak made of two halves), drawn on the 15 × 5 board, with the
  official colours (the renderer adapts light ones for LEDs). The same checker covers
  them, and `python3 tools/flag_grid.py flag-<id>` prints one as a letter grid.
- New patterns and palettes live in `crates/keylume-profiles/src` (see
  [CONTRIBUTING.md](../CONTRIBUTING.md)). Built-in ids never change once released,
  because favourites refer to them.
- Names are always our own. Colour schemes can't be owned, but names can: no
  franchise, character, brand or product names, nicknames or logos. Describe the
  colours instead ("a caped night guardian"), and keep everything free of personal
  information.
