# Design packs

A **pack** is a set of designs in one `.keylumepack` file, to give away or to sell: seasonal
sets, event sets, your own collection, anything. A pack's collections sit in the Library like
any other, in their section (Themes, unless a collection says Games or Comics), and they work
on whatever keyboard is connected.

A pack can hold two kinds of design:

- **Themes** in the catalogue's format: a name and a few colours that Keylume turns into a
  whole family of looks (per-key patterns, an animation, a live effect), drawn for the
  keyboard that's connected. This is how the built-in catalogue is made.
- **Designs made in Keylume**: your own per-key paintings, animations, live effects and
  spells, exactly as you made them on the **Create** page. They're listed under the pack's
  name. Keys a keyboard doesn't have are left out (a 60 % board has no arrows).

## Packs that come with Keylume

| Pack | Designs |
|---|---|
| **Solar Terms** | the 24 solar terms of the East Asian calendar, one design family for each fortnight of the year |
| **Life's Moments** | 18 designs for occasions worth celebrating: weddings, anniversaries, graduations, a new home, a new baby… |

They're built into the app: always there, shown like any other collection, and not removable. Only an
official file of the same pack (same `id`, signed by Keylume) can replace one, until that file
is removed.

## Adding a pack

Drop a `.keylumepack` file on the window, or use **Upload** under Library → Mine. The file is
checked in full before anything is kept: its format, every theme (with the same rules as the
built-in catalogue, including *names are our own*), every design (with the rules for your own
profiles), and that none of its ids or collection names clash with the library. A pack that
fails says why, and nothing is added. **Remove pack** takes one out again.

Everything is checked on your computer. Keylume never contacts anyone to check a pack.

## Making a pack

### In the app

1. Make your designs on the **Create** page. They're saved under Library → **Mine**.
2. Under Mine, press **Make a pack**.
3. Give the pack a name and a one-line description, say who made it, and pick the designs.
   Optionally add your web page (an `https://` address): people who have your pack get a
   *More from you* link that opens it in their browser.
4. Choose who may have it: **Anyone** (free to pass on) or **Only who gets it from you** (for a
   pack you sell).
5. **Save pack…** writes the file. That file is the whole pack: share it, sell it, put it on
   your site.

Keylume remembers what you filled in. Make the pack again under the same name and it becomes
the next version (1.0, 1.1, …) of the same pack. People who add the new file get the update.

### With the command line (themes)

Theme packs, like the two that come with Keylume, are written as files and built with
`keylume-cli`. A pack's sources live in a folder: a `pack.json` with the id, name, publisher,
version and description (and optionally `licence` and `url`), one or more theme collection
files (e.g. `themes.json`), and optionally a `designs.json` with designs made in Keylume.

```bash
# check a collection while you design it (the catalogue checker)
cargo run -q -p keylume-profiles --example themes -- packs/<id>/themes.json

# build it into a .keylumepack (checked as the app will check it); signed with --key
cargo run -q -p keylume-cli -- pack build packs/<id> --key ~/.config/keylume/publisher-key.txt -o <id>-1.0.0.keylumepack

# check a pack file as the app would
cargo run -q -p keylume-cli -- pack check <file>.keylumepack
```

## Who made it: signatures

A pack may carry an Ed25519 signature over its content. Keylume checks it when the pack is
added, and shows who it's from:

| The Library says | Meaning |
|---|---|
| *(no badge)* | It comes with Keylume: it's just part of the catalogue. |
| **Official** | Signed with a publisher key built into Keylume (`PUBLISHERS` in `crates/keylume-core/src/packs.rs`). The publisher's name comes from that list, whatever the file claims. |
| **Community**, *signed with their key 3F9A B2C1* | Made in Keylume and signed with its maker's own key. The key's fingerprint tells two makers with the same name apart. |
| **Community**, *unsigned* | Nobody vouches for it. It works all the same. |

A signature that doesn't match is refused: the file was changed after it was signed. And an
installed signed pack is only ever **updated by a file with the same key**, so nobody else can
push a "new version" of a pack you have. If you trust such a file anyway, remove the old pack
first.

### Your maker key

The first time you make a pack, Keylume creates your maker key and keeps it in its data folder
(`maker.json`, readable only by your account). It never leaves your computer; the packs carry
only its public half. Back up Keylume's data folder: a new key means your next pack counts as
someone else's, and people would have to remove the old one before adding it.

### The publisher key (official packs)

`keylume-cli pack keygen <file>` makes a publisher key: the secret half goes to the file
(readable only by you), and the public half is printed for `PUBLISHERS`. Keep the secret file
out of the repository and **back it up** (a password manager is a good place): packs signed
with a lost key still verify, but new ones can't be signed with it, and a new key means a new
app release before its packs count as official. Forks of Keylume list their own keys.

## Selling packs

Keylume is free and open source, and stays local: it has no shop, accounts or payments. A pack
is sold the way any digital file is sold: put it on a store that sells downloads (your own site,
or a marketplace for digital goods), and buyers add the file by dropping it on Keylume. The pack
names its maker and links to their page, so people can find more of their packs.

Worth knowing before you sell:

- **A signature proves who made a pack, not who paid for it.** Keylume can't stop a file from
  being copied: that would need an online check, and Keylume never goes online. *Only who gets
  it from you* asks people not to pass it on, and says so in the Library; it doesn't enforce it.
  Selling packs works on trust, as art, fonts and music packs do.
- **Your designs are yours.** Keylume's own code and built-in catalogue are MIT-licensed, but a
  pack you make is your own work, sold on your own terms.
- **Names are your own.** Packs follow the catalogue's rule: no franchise, character, brand or
  product names. Keylume refuses the obvious ones.
- Paid pack sources don't belong in the public repository ([RELEASING.md](RELEASING.md),
  *Before the repository goes public*).

## The file

A `.keylumepack` is JSON:

```json
{
  "keylumePack": 1,
  "id": "neon-nights-3f9ab2",
  "name": "Neon Nights",
  "publisher": "PlayerOne",
  "version": "1.1",
  "description": "Bright designs for late nights",
  "licence": "personal",
  "url": "https://example.com/packs",
  "collections": [ { "collection": "Neon Nights", "prefix": "neon", "themes": [ … ] } ],
  "designs": [
    { "id": "sunset-keys", "name": "Sunset Keys", "tags": ["warm"], "description": "Orange letters",
      "lighting": { "kind": "perKey", "brightness": 4, "keys": { "q": "#ff7a00", "esc": "#5a1a8a" } } }
  ],
  "signature": { "key": "3f9ab2c1…", "sig": "…" }
}
```

| Field | Notes |
|---|---|
| `id` | lower-case letters, digits and dashes, up to 40. Packs made in the app end with the start of the maker's key. |
| `name`, `publisher`, `version`, `description` | plain text: up to 40, 60, 20 and 300 characters. |
| `licence` | optional: `"share"` (anyone may pass it on) or `"personal"` (for the person who got it). |
| `url` | optional: the maker's page, an `https://` address. |
| `collections` | up to 8 theme collections in exactly the catalogue's format ([`crates/keylume-profiles/themes/`](../crates/keylume-profiles/themes), described in `CONTRIBUTING.md`), 200 themes at most. |
| `designs` | up to 200 designs made in Keylume: an `id` of its own, a `name`, optional `tags` and `description`, and a `lighting` exactly as in a profile file ([PROFILES.md](PROFILES.md)). |
| `signature` | optional: `key` is a publisher key's id, or the maker's public key (64 hex digits); `sig` is the signature in hex. |

A pack holds at least one theme or design, and is 4 MB at most. The signature covers the pack's
*canonical* JSON: the whole pack without its `signature` field, every object's keys sorted, no
whitespace. So re-formatting a file doesn't break its signature, and changing anything else
does.
