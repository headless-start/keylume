# Keylume UI design

How the app is laid out and how new screens should look. The goal: minimal and modern, with
colour where it counts: the device-first feel of the big vendors' apps, with fewer controls.

## Principles

- **The device is the hero** (as in Logitech G HUB). The app opens on Home: the keyboard, as big
  as the window allows, lit with what's on it now, and under it only its name, the name of the
  look on its keys, whether it's connected, the power button and Customise. Clicking the keyboard (or Customise) opens its
  pages. The interface stays dark and quiet: the lighting, and the logo's spectrum on the few
  things that matter, are the colour.
- **Any keyboard.** Nothing names a model: the connected keyboard's shape and features decide
  what's drawn and what's offered ([DEVICES.md](DEVICES.md)). A place, tab, animation or design
  the keyboard can't use isn't shown.
- **The catalogue is the product.** Lighting opens on the Library's Discover: favourites, then
  every collection as a picture. Finding a look takes a click or two: by collection, by word, by
  colour, or "Surprise me".
- **One way around.** Every place is in one rail down the left (Library, Create, Side light,
  Keys, and Settings at its foot); the top bar only says which keyboard and how it is,
  with the way back to Home and the power button. No tabs across the top, and no control or
  setting appears twice.
- **Fewer, clearer controls.** Prefer a grouped dropdown to a wall of chips, one description line
  per choice, sensible defaults, and "Advanced" folds for the rarely needed.
- **Designs, not duplicates.** The Library shows one card per design. Its looks (per-key patterns,
  animations, live effects) are chosen in the side panel.

## Structure

```
Home                                 Every other page
─────────────────────────────        ─────────────────────────────────────────────────
                                     ‹ Epomaker TK68 ● Connected                     ⏻
                                     ┌──────────┬────────────────────────────────────
     the keyboard, big, lit          │ Library  │  the page (Library: Discover,
                                     │ Create   │  collections, packs, results, and
     Epomaker TK68                   │ Side     │  the side panel with the big
     Deep Ocean · Cascade            │ Keys     │  keyboard)
     ● Connected                     │          │
     [⏻] [Customise]                 │          │
                                     │ …        │
                                     │ Settings │
                                     └──────────┴────────────────────────────────────
```

Home has no bar and no buttons in its corners: the window's title already says Keylume, and
Settings is at the foot of the rail, one click past Customise. Nothing pops up over the keyboard
when you point at it; clicking it opens the Library, like Customise. More devices will each get their place on Home; their pages follow the same shape.

## Home shows what's on the keys

Home's keyboard (`Mirror`, `src/lib/mirror.ts`) draws only what the device worker reports in its
lighting state (`keylume-core::lighting`): a look counts as on the keyboard once the keyboard
accepted it, never when it was merely clicked. The look's name follows the same rule: it sits
under the keyboard's name once the keyboard took it ("Lights off" when they are), and goes while
the keyboard is unplugged or paused. The words stay few: the status line says
connected, not connected (with how to plug in), paused or busy, and a line appears only while
something is being applied or when it failed. What a keyboard animation is (an approximation of
what the keyboard runs) is told to screen readers, not printed under the keyboard.

- **Per-key designs and spells** are exact. A spell shows the picture it has really reached
  (each one is a flash write, about 1.5 s), then holds the last one; it never loops on Home.
- **Live effects** play the frames the keyboard was sent, at the brightness they were sent.
- **The keyboard's own animations** can't be read back frame by frame (see `PROTOCOL.md`,
  *What Keylume can see*), so Home plays an approximation and says so. It never shows made-up key
  presses: a reactive effect sits dark until you type, as on the keyboard.
- The "On keyboard" badge in the Library and the tray's check mark follow the same state.

## Motion

- Every hover, press and selection uses the same short transition (`--t`, `--ease`).
- The system's *reduce motion* setting (on Windows: *Animation effects* off) stops the interface's
  own transitions. Lighting previews still play: a design you point at or open is there to show
  what it does, and Home mirrors a keyboard that moves anyway. The keyboard itself is never
  changed by this setting.
- Previews stop while off screen, while the window is hidden, and when nobody's looking at Home
  (the app then stops sending live frames to the window).

## Keyboard access

Everything works without a mouse. On the Paint and Keys pages the keys are buttons: Tab reaches
the keyboard, the arrow keys move between keys by position, Enter or Space presses one. Keyboard
focus previews a design the way a hover does.

## The Library

- **Discover** (the start view): favourites as a row, then each section's collections as tiles
  (four designs each). A tile opens the collection; "‹ Section" goes back.
- **Packs** have no tab of their own: a pack's collections sit in their section (Themes unless
  they say Games or Comics), so the catalogue reads as one. A pack someone added is marked
  *Official* or *Community* on its tile; open, it says who made it (with the maker's key when
  it's signed by one), whether it may be passed on, a *More from …* link to the maker's page,
  and Remove. The packs that come with Keylume look like any other collection. Packs are added
  like profiles: Upload under Mine, or a drop on the window. **Make a pack** under
  Mine puts your own designs in one signed file, in a dialog (`Dialog`, `MakePack`): name, a
  line about it, who made it, version, page, who may have it, and the designs
  ([PACKS.md](PACKS.md)).
- Only what the keyboard can show is listed (`canShow` in `src/lib/features.ts`).
- **Find**: search, the colour dots (a design's colour family, `colourFamily` in
  `src/lib/library.ts`), the kind dropdown, and Surprise me (a random design from what's shown).
- A **design** (family) is every profile named `Design · Look` in the same collection; a profile
  without ` · ` is a design on its own. Your own profiles always stand alone
  (`src/lib/families.ts`).
- Cards show the design's first matching look, its collection and how many looks it has of each
  kind. Hovering flips through its per-key looks, or plays an animated one.
- Clicking a card puts that look on the keyboard and opens the design in the side panel. The panel
  shows a large animated preview, the look's name, ★ and "Edit a copy", and every look grouped as
  Per-key, Animated and Live. Clicking a look applies it; hovering previews it.
- Tabs: Discover, Favourites, Mine, then the sections. With nothing open, the side panel plays a
  slow rainbow wave and offers Surprise me.

## Layouts

Every page past Home starts with the same head (`PageHead`): its title, then what switches its
view (Create's four kinds, Keys' Remap and Macros), and the page's own actions on the right.
Below it comes one of three frames, and nothing else:

```
Builder (Create, Side light, Keys)                 Column (Settings)
Title  [tab | tab | tab]                                 Title
┌──────────────────────────────┐ ┌──────────┐            GROUP
│ stage bar: tools    actions  │ │ settings │            ┌──────────────────────┐
│                              │ │          │            │ label          [ctl] │
│        the keyboard          │ │          │            │ label          [ctl] │
│                              │ │──────────│            └──────────────────────┘
│ one-line note                │ │ [Save]   │            GROUP …
└──────────────────────────────┘ └──────────┘
```

- **Builder** (`.page-wide` > `.builder`): the stage (`.stage-card`) holds what you're making,
  as it will look: a bar of tools (`.stage-bar`, actions pushed right with `.bar-actions`), the
  keyboard (`.stage-board`, the same size on every tab: animated previews let their glow spill
  instead of shrinking the keys) and one note at its foot (`.stage-note`). The panel
  (`.builder-panel`, 340 px) holds the settings, and its one main action sits at its foot
  (`.panel-foot`: the name and Save, or Write to keyboard). The two line up at the top and the
  bottom. When the panel's content can run long (Remap's list of keys), the row takes the
  stage's height and the list scrolls inside the panel (`.fit-stage`, `.panel-cell`).
- **Column** (`.page-column`, 760 px, centred): Settings, the one place for every setting.
  Titled groups (`Group`), each a card of rows (`Row`, `Toggle`): a short label, at most one
  short hint, the control on the right. No paragraphs. The keyboard's own settings come first
  (Keyboard, Keys); they're stored on the keyboard, so they're written together: a bar sticks
  to the foot of the page while any changed (`.save-bar`: Discard, Save to keyboard). Keylume's
  own settings apply at once; Backup, then the About line, close the page.
- **Library**: the grid on the left, the side panel on the right (`.library`, `.inspector`).

Check a layout at 1280×800 and at 1024×680; below 1000 px the builder's panel goes under the
stage.

## Tokens and components

One typeface: **Plus Jakarta Sans** (OFL), bundled with the app (`@fontsource-variable`), bold
for headings. Colours, radii and shadows are CSS variables at the top of `src/styles.css`:
backgrounds `--bg`, `--bg-2`; surfaces `--surface` to `--surface-3`; lines `--line` to
`--line-3`; text `--text`, `--muted`, `--faint`; accents `--accent` (indigo), `--accent-2`
(cyan), `--accent-3` (magenta). The logo's spectrum is `--grad` (cyan → indigo → magenta →
coral): primary buttons, the edge of the active tab and of the design on the keyboard, the
"On keyboard" badge, the rail's marker, section headings (`--grad-cool`) and switches. The top
bar and rail are glass over faint aurora light (the body's background). The logo is
`src/assets/logo.svg`: a dark keycap whose K shines through in the spectrum.

| Component | Where |
|---|---|
| `Keyboard`: case, lit keycaps, underglow; interactive | `src/components/Keyboard.tsx` |
| `AnimatedBoard`: a keyboard playing any profile | `src/components/AnimatedBoard.tsx` |
| `Thumb`: fast canvas thumbnail (a case, keycaps with a lighter top face, the lit keys glowing on the plate), hover to play | `src/components/Thumb.tsx` (`paintBoard`) |
| Buttons (`.btn`, `.primary`, `.ghost`, `.small`, `.icon-btn`), `.segmented`, `Toggle`, `Slider`, `ColorField`, `SaveRow` | `src/components/controls.tsx`, `src/styles.css` |
| Device status, "On the keyboard", power button | `src/components/Device.tsx` |

Unlit keys are drawn as keycaps, never as holes: dark ones (`capColor`) everywhere, except that
Home draws the keyboard in its own finish (the layout's `finish`): on a white keyboard like the
TK68 the keycaps are white and a lit key's colour shines through them (`whiteCapColor`). Previews show a design's own
colours; with **True colours** on (the default), the app adjusts every colour it sends to the
keyboard for the LEDs, so the keys come closer to the screen. How close is for a real board to
say: [CALIBRATION.md](CALIBRATION.md) is the check.

## Checking a change

`tools/ui_shots.mjs` screenshots the browser preview in a few steps, for example:

```bash
node --experimental-websocket tools/ui_shots.mjs /tmp/shots click:Customise nav:Create shot:create
```

Look at the pictures, then delete them: screenshots never go in the repo except the README's
(`tools/readme_shots.mjs`).
