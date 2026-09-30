//! Lighting profiles: the data model plus the generated built-in library.
//!
//! A profile is pure data. Per-key profiles are rendered from a *pattern* (a function
//! of key position) and a *palette*; animated profiles are a firmware [`Effect`].
//! The library is `(palettes + themes) × patterns + curated effects`, so adding one
//! palette, theme or pattern grows it automatically.

use std::collections::BTreeMap;

use keylume_proto::{Effect, Layout, Rgb};
use serde::{Deserialize, Serialize};

pub mod color;
pub mod cs;
pub mod effects;
pub mod flags;
pub mod palettes;
pub mod patterns;
pub mod side;
pub mod skins;
pub mod spell;
pub mod themes;

/// The Library's sections, each holding several collections.
pub mod section {
    pub const GAMES: &str = "Games";
    pub const COMICS: &str = "Comics";
    pub const THEMES: &str = "Themes";
    pub const COLOURS: &str = "Colours";
    pub const FLAGS: &str = "Flags";
    pub const EFFECTS: &str = "Effects";
}

pub use palettes::Palette;
pub use patterns::Pattern;
pub use spell::SpellWord;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Lighting {
    /// A firmware animation (breathing, wave, ripple, …).
    Effect { effect: Effect },
    /// A static colour per key, stored in one of the keyboard's picture layers.
    PerKey {
        /// key id (from the layout) -> colour
        keys: BTreeMap<String, Rgb>,
        #[serde(default = "full")]
        brightness: u8,
    },
    /// A host-driven animation streamed in real time while Keylume runs.
    Live { live: keylume_live::LiveEffect },
    /// Words typed out on their keys letter by letter, then held (see [`spell`]).
    Spell {
        words: Vec<SpellWord>,
        background: Rgb,
        #[serde(default = "full")]
        brightness: u8,
    },
}

fn full() -> u8 {
    keylume_proto::led::MAX_BRIGHTNESS
}

/// Serialised form of a [`Lighting`], for exact-duplicate checks.
pub(crate) fn lighting_key(l: &Lighting) -> String {
    serde_json::to_string(l).expect("Lighting always serialises")
}

/// How far a colour is from grey, ignoring how bright it is (themes and flags rank their
/// own colours by this, brightest first, to find their most eye-catching shade).
pub(crate) fn chroma_i32(c: &Rgb) -> i32 {
    c.0.max(c.1).max(c.2) as i32 - c.0.min(c.1).min(c.2) as i32
}

pub(crate) fn sum_i32(c: &Rgb) -> i32 {
    c.0 as i32 + c.1 as i32 + c.2 as i32
}

/// The first of `preferred` then `all` (round-robin from `preferred`'s position) that
/// isn't in `used` and whose lighting isn't in `seen` yet; records the pick in both, so
/// later calls (for other themes or flags) avoid it too. Themes and flags use this to
/// keep every generated animation, bar pattern, music visualiser and live effect from
/// exactly duplicating one already produced. Falls back to `preferred` if every option in
/// `all` is somehow taken (kept rather than panicking; not expected with this many to
/// choose from).
pub(crate) fn pick_unused<T: Copy + PartialEq>(
    preferred: T,
    all: &[T],
    used: &mut Vec<T>,
    seen: &mut std::collections::HashSet<String>,
    light: impl Fn(T) -> Lighting + Copy,
) -> T {
    let free = |c: &T, seen: &std::collections::HashSet<String>| !used.contains(c) && !seen.contains(&lighting_key(&light(*c)));
    let start = all.iter().position(|c| c == &preferred).unwrap_or(0);
    let chosen = if free(&preferred, seen) {
        preferred
    } else {
        (1..all.len()).map(|s| all[(start + s) % all.len()]).find(|c| free(c, seen)).unwrap_or(preferred)
    };
    used.push(chosen);
    seen.insert(lighting_key(&light(chosen)));
    chosen
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: String,
    /// `builtin` profiles are regenerated on every start; `user` ones live on disk.
    #[serde(default = "user_source")]
    pub source: Source,
    /// The part of the Library a built-in profile is listed in (see [`section`]); empty
    /// for your own.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub section: String,
    pub lighting: Lighting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Builtin,
    User,
}

fn user_source() -> Source {
    Source::User
}

/// Limits on profiles that come from outside the built-in library (uploads, saves, files
/// on disk), generous next to what the editors make. Every built-in profile fits them.
pub mod limits {
    pub const NAME: usize = 60;
    pub const CATEGORY: usize = 60;
    pub const DESCRIPTION: usize = 500;
    pub const TAGS: usize = 24;
    pub const TAG: usize = 40;
    /// Key colours in one per-key profile (the matrix has 128 slots).
    pub const KEYS: usize = 128;
    pub const KEY_ID: usize = 24;
    /// A spell writes one picture per letter (about 1.5 s of flash commit each), so
    /// words stay few and short, as in Create.
    pub const SPELL_WORDS: usize = 4;
    pub const SPELL_WORD: usize = 16;
}

/// Plain text: no control characters, and at most `max` characters.
fn text(what: &str, s: &str, max: usize) -> Result<(), String> {
    if s.chars().count() > max || s.chars().any(char::is_control) {
        return Err(format!("{what} should be plain text of up to {max} characters"));
    }
    Ok(())
}

/// A key id as layouts write them: lower-case letters, digits, `-` and `_`.
pub fn is_key_id(id: &str) -> bool {
    (1..=limits::KEY_ID).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

impl Lighting {
    /// Can the keyboard show this? Err says why, in words. Key ids a layout lacks are
    /// allowed (a profile may come from a board with more keys) but at least one key
    /// must exist on `layout`.
    pub fn validate(&self, layout: &Layout) -> Result<(), String> {
        let bright = |b: u8| if b > keylume_proto::led::MAX_BRIGHTNESS { Err("brightness goes from 0 to 4".to_string()) } else { Ok(()) };
        match self {
            Lighting::Effect { effect } => effect.validate().map_err(|e| e.to_string()),
            Lighting::PerKey { keys, brightness } => {
                bright(*brightness)?;
                if keys.is_empty() || keys.len() > limits::KEYS || !keys.keys().all(|k| is_key_id(k)) {
                    return Err(format!("it needs 1 to {} key colours, named by key id (\"esc\", \"w\", …)", limits::KEYS));
                }
                if !keys.keys().any(|k| layout.key(k).is_some()) {
                    return Err(format!("none of its keys is on the {}", layout.name));
                }
                Ok(())
            }
            Lighting::Live { live } => live.validate(),
            Lighting::Spell { words, brightness, .. } => {
                bright(*brightness)?;
                if words.is_empty() || words.len() > limits::SPELL_WORDS {
                    return Err(format!("a spell has 1 to {} words", limits::SPELL_WORDS));
                }
                for w in words {
                    text("a spell word", &w.text, limits::SPELL_WORD)?;
                }
                if words.iter().all(|w| spell::keys_of(layout, &w.text).is_empty()) {
                    return Err("it has no letters to spell".into());
                }
                Ok(())
            }
        }
    }
}

impl Profile {
    /// Is this a profile Keylume can store and show, within [`limits`]? Err says why.
    pub fn validate(&self, layout: &Layout) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err(format!("every profile needs a name of up to {} characters", limits::NAME));
        }
        text("the name", &self.name, limits::NAME)?;
        text("the category", &self.category, limits::CATEGORY)?;
        if self.description.chars().count() > limits::DESCRIPTION {
            return Err(format!("the description has more than {} characters", limits::DESCRIPTION));
        }
        if self.tags.len() > limits::TAGS {
            return Err(format!("a profile has up to {} tags", limits::TAGS));
        }
        for t in &self.tags {
            text("a tag", t, limits::TAG)?;
        }
        self.lighting.validate(layout)
    }

    /// Colour of every key, for previews. Effects preview as their base colour.
    pub fn preview(&self, layout: &Layout) -> BTreeMap<String, Rgb> {
        match &self.lighting {
            Lighting::PerKey { keys, .. } => layout.keys.iter().map(|k| (k.id.clone(), keys.get(&k.id).copied().unwrap_or(Rgb::BLACK))).collect(),
            Lighting::Effect { effect } => {
                let c = if effect.mode == keylume_proto::Mode::Off { Rgb::BLACK } else { effect.color };
                layout.keys.iter().map(|k| (k.id.clone(), c)).collect()
            }
            Lighting::Live { live } => {
                // First frame of the animation, as a flat colour.
                let c = match keylume_live::Animator::new(live.clone()).frame(0.3, &Default::default()) {
                    keylume_live::LiveFrame::Color(c) => c,
                    keylume_live::LiveFrame::Levels(_) => live.base_effect().color,
                };
                layout.keys.iter().map(|k| (k.id.clone(), c)).collect()
            }
            Lighting::Spell { words, background, .. } => spell::final_frame(layout, words, *background),
        }
    }
}

fn live_profiles() -> Vec<Profile> {
    keylume_live::presets()
        .into_iter()
        .map(|(name, desc, live)| {
            use keylume_live::LiveEffect as L;
            // everything Counter-Strike lives in the CS2 collection
            let cs2 = matches!(live, L::Bomb { .. } | L::Flashbang { .. });
            let mut tags: Vec<String> = vec!["live".into(), "animated".into(), "dynamic".into()];
            if cs2 {
                tags.extend(["cs".into(), "gaming".into()]);
            }
            Profile {
                id: format!("live-{}", name.to_lowercase().replace(" · ", "-").replace(' ', "-")),
                name: name.to_string(),
                category: if cs2 { "CS2" } else { "Live" }.to_string(),
                tags,
                description: desc.to_string(),
                source: Source::Builtin,
                section: if cs2 { section::GAMES } else { section::EFFECTS }.to_string(),
                lighting: Lighting::Live { live },
            }
        })
        .collect()
}

/// A profile as the app's window receives it: per-key colours packed into one hex string
/// in layout order (`"rrggbb…"`, a third of the size of the key map), the rest as usual.
/// Profile files on disk keep the readable key map.
pub fn to_wire(p: &Profile, layout: &Layout) -> serde_json::Value {
    let Lighting::PerKey { keys, brightness } = &p.lighting else {
        return serde_json::to_value(p).unwrap_or_default();
    };
    let packed: String = layout
        .keys
        .iter()
        .map(|k| {
            let c = keys.get(&k.id).copied().unwrap_or(Rgb::BLACK);
            format!("{:02x}{:02x}{:02x}", c.0, c.1, c.2)
        })
        .collect();
    // everything but the lighting, without cloning the key map
    let shell = Profile {
        id: p.id.clone(),
        name: p.name.clone(),
        category: p.category.clone(),
        tags: p.tags.clone(),
        description: p.description.clone(),
        source: p.source,
        section: p.section.clone(),
        lighting: Lighting::Spell { words: Vec::new(), background: Rgb::BLACK, brightness: *brightness },
    };
    let mut v = serde_json::to_value(&shell).unwrap_or_default();
    v["lighting"] = serde_json::json!({ "kind": "perKey", "brightness": brightness, "packed": packed });
    v
}

/// Render a pattern with a palette into a per-key profile.
pub fn render(layout: &Layout, pattern: &Pattern, palette: &Palette) -> BTreeMap<String, Rgb> {
    render_ctx(layout, &patterns::Ctx::new(layout), pattern, palette)
}

fn render_ctx(layout: &Layout, ctx: &patterns::Ctx, pattern: &Pattern, palette: &Palette) -> BTreeMap<String, Rgb> {
    layout.keys.iter().map(|k| (k.id.clone(), pattern.color(ctx, k, palette))).collect()
}

/// The two CS2 designs drawn from CS2's default key binds: every key by what it does, and
/// a crosshair.
fn cs_profiles(layout: &Layout) -> Vec<Profile> {
    let binds = cs::default_binds();
    let crosshair = Rgb(0x00, 0xFF, 0x60);
    let mk = |id: &str, name: &str, desc: &str, keys| Profile {
        id: id.into(),
        name: name.into(),
        category: "CS2".into(),
        tags: vec!["blue".into(), "cs".into(), "gaming".into()],
        description: desc.into(),
        source: Source::Builtin,
        section: section::GAMES.to_string(),
        lighting: Lighting::PerKey { keys, brightness: full() },
    };
    vec![
        mk(
            "cs-binds",
            "CS2 Binds",
            "Every key lit by what it does in CS2's default binds, in shades of blue; the jump-throw key in crosshair green.",
            cs::binds_keys(layout, &binds, crosshair),
        ),
        mk("cs-crosshair", "CS2 Crosshair", "A green crosshair across a blue board.", cs::crosshair_keys(layout, crosshair, false)),
    ]
}

/// The whole built-in library for `layout`, in a stable order.
pub fn builtin(layout: &Layout) -> Vec<Profile> {
    let ctx = patterns::Ctx::with_cs(layout, cs::default_binds());
    let mut out = Vec::new();
    for pal in palettes::all() {
        for pat in patterns::all() {
            if pal.only.is_some_and(|only| !only.contains(&pat.id)) {
                continue;
            }
            out.push(Profile {
                id: format!("{}-{}", pal.id, pat.id),
                name: format!("{} · {}", pal.name, pat.name),
                category: pal.family.to_string(),
                tags: pal.tags.iter().chain(pat.tags.iter()).map(|s| s.to_string()).collect(),
                description: format!("{} rendered as {}.", pal.description, pat.description),
                source: Source::Builtin,
                section: if pal.family == "CS2" { section::GAMES } else { section::COLOURS }.to_string(),
                lighting: Lighting::PerKey { keys: render_ctx(layout, &ctx, &pat, &pal), brightness: full() },
            });
        }
    }
    // Themes and flags both add keyboard animations and live effects in round-robin
    // fashion; a shared record of every lighting produced so far (seeded with the
    // curated effects below, which never change) keeps any of them from exactly
    // duplicating another (see `pick_unused`).
    let mut seen: std::collections::HashSet<String> =
        effects::all().iter().chain(effects::ghosts().iter()).map(|p| lighting_key(&p.lighting)).collect();
    out.extend(themes::profiles(layout, &ctx, &mut seen));
    out.extend(flags::profiles(layout, &mut seen));
    out.extend(cs_profiles(layout));
    out.extend(skins::all(layout));
    out.extend(effects::all());
    out.extend(effects::ghosts());
    out.extend(live_profiles());
    fit(layout, &mut out);
    out
}

/// The designs of a pack's collections, drawn for `layout`. A pack's collections sit in the
/// Library like any other, in the section they name (Themes unless they say Games or Comics).
pub fn pack_profiles(layout: &Layout, collections: &[&'static themes::Collection]) -> Vec<Profile> {
    let ctx = patterns::Ctx::new(layout);
    let mut seen = std::collections::HashSet::new();
    let mut out = themes::render(collections, layout, &ctx, &mut seen);
    fit(layout, &mut out);
    out
}

/// Keep only the keys `layout` has: designs name keys by id, and a smaller board lacks
/// some of them (a 60 % board has no arrows).
pub fn fit(layout: &Layout, profiles: &mut [Profile]) {
    let ids: std::collections::HashSet<&str> = layout.keys.iter().map(|k| k.id.as_str()).collect();
    for p in profiles {
        if let Lighting::PerKey { keys, .. } = &mut p.lighting {
            keys.retain(|k, _| ids.contains(k.as_str()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_is_big_unique_and_complete() {
        let layout = Layout::tk68();
        let lib = builtin(&layout);
        assert!(lib.len() >= 24_000, "only {} profiles", lib.len());
        let mut ids: Vec<_> = lib.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), lib.len(), "duplicate ids");
        for p in &lib {
            if let Lighting::PerKey { keys, .. } = &p.lighting {
                assert_eq!(keys.len(), layout.keys.len(), "{}", p.id);
            }
            if let Lighting::Effect { effect } = &p.lighting {
                effect.validate().unwrap_or_else(|e| panic!("{}: {e}", p.id));
            }
        }
    }

    #[test]
    fn the_library_fits_the_limits_uploads_are_held_to() {
        let layout = Layout::tk68();
        for p in builtin(&layout) {
            p.validate(&layout).unwrap_or_else(|e| panic!("{}: {e}", p.id));
        }
    }

    #[test]
    fn every_standard_layout_gets_the_whole_library() {
        for id in keylume_proto::standard::STANDARD {
            let layout = keylume_proto::standard::standard(id).unwrap();
            let t = std::time::Instant::now();
            let lib = builtin(&layout);
            let took = t.elapsed();
            assert!(lib.len() > 20_000, "{id}: {} profiles", lib.len());
            let ids: std::collections::HashSet<&str> = layout.keys.iter().map(|k| k.id.as_str()).collect();
            for p in &lib {
                p.validate(&layout).unwrap_or_else(|e| panic!("{id} {}: {e}", p.id));
                if let Lighting::PerKey { keys, .. } = &p.lighting {
                    assert!(keys.keys().all(|k| ids.contains(k.as_str())), "{id} {}: keys the board doesn't have", p.id);
                }
            }
            eprintln!("{id}: {} profiles in {took:?}", lib.len());
        }
    }

    #[test]
    fn validation_says_why() {
        let layout = Layout::tk68();
        let p = |lighting: Lighting| Profile {
            id: String::new(),
            name: "Test".into(),
            category: "Mine".into(),
            tags: vec![],
            description: String::new(),
            source: Source::User,
            section: String::new(),
            lighting,
        };
        let keys = |pairs: &[(&str, Rgb)]| pairs.iter().map(|(k, c)| (k.to_string(), *c)).collect::<BTreeMap<_, _>>();
        let red = Rgb(255, 0, 0);
        assert!(
            p(Lighting::PerKey { keys: keys(&[("esc", red), ("f13", red)]), brightness: 4 }).validate(&layout).is_ok(),
            "unknown keys are ignored"
        );
        let word = |t: &str| SpellWord { text: t.into(), color: red };
        for (bad, why) in [
            (p(Lighting::PerKey { keys: keys(&[]), brightness: 4 }), "key colours"),
            (p(Lighting::PerKey { keys: keys(&[("../esc", red)]), brightness: 4 }), "key id"),
            (p(Lighting::PerKey { keys: keys(&[("f13", red)]), brightness: 4 }), "none of its keys"),
            (p(Lighting::PerKey { keys: keys(&[("esc", red)]), brightness: 9 }), "brightness"),
            (p(Lighting::Spell { words: vec![word("a"); 5], background: red, brightness: 4 }), "words"),
            (p(Lighting::Spell { words: vec![word(&"a".repeat(17))], background: red, brightness: 4 }), "spell word"),
            (p(Lighting::Spell { words: vec![word("?!")], background: red, brightness: 4 }), "no letters"),
            (p(Lighting::Live { live: keylume_live::LiveEffect::Steps { colors: vec![], hold: 1.0 } }), "colours"),
        ] {
            let e = bad.validate(&layout).unwrap_err();
            assert!(e.contains(why), "{e:?} should mention {why:?}");
        }
        let long = Profile { name: "x".repeat(61), ..p(Lighting::PerKey { keys: keys(&[("esc", red)]), brightness: 4 }) };
        assert!(long.validate(&layout).is_err());
        let control = Profile { name: "a\u{7}b".into(), ..long.clone() };
        assert!(control.validate(&layout).is_err());
    }

    #[test]
    fn profiles_round_trip_through_json() {
        let layout = Layout::tk68();
        for p in builtin(&layout).iter().step_by(37) {
            let s = serde_json::to_string(p).unwrap();
            assert_eq!(&serde_json::from_str::<Profile>(&s).unwrap(), p);
        }
    }

    #[test]
    fn the_library_is_neutral_and_cs2_is_its_own_collection() {
        let lib = builtin(&Layout::tk68());
        let cs2: Vec<_> = lib.iter().filter(|p| p.category == "CS2").collect();
        for kind in ["perKey", "live"] {
            assert!(cs2.iter().any(|p| serde_json::to_value(&p.lighting).unwrap()["kind"] == kind), "CS2 has no {kind}");
        }
        for id in ["cs-binds", "cs-crosshair", "live-blue-defuse", "ghost-royal-laser", "ghost-gold"] {
            assert!(lib.iter().any(|p| p.id == id), "missing {id}");
        }
    }

    #[test]
    fn cs2_skins_and_a_full_colour_wheel() {
        let lib = builtin(&Layout::tk68());
        let skins = lib.iter().filter(|p| p.category == "CS2 Skins").count();
        assert!(skins >= 100, "{skins} skin profiles");
        for id in ["skin-asiimov-split", "skin-dragon-lore-flow", "skin-fade-animated", "skin-blue-gem-marble"] {
            assert!(lib.iter().any(|p| p.id == id), "missing {id}");
        }
        // animated effects come in every hue, not mostly blue
        let fx: Vec<_> = lib
            .iter()
            .filter_map(|p| match &p.lighting {
                Lighting::Effect { effect } if !effect.rainbow => Some(effect.color),
                _ => None,
            })
            .collect();
        let bluish = fx.iter().filter(|c| c.2 > c.0 && c.2 > c.1).count();
        assert!((bluish as f32) < fx.len() as f32 * 0.45, "{bluish} of {} animated colours are blue", fx.len());
    }

    #[test]
    fn no_two_per_key_designs_are_identical_and_none_are_mostly_off() {
        let layout = Layout::tk68();
        let lib = builtin(&layout);
        let mut names = std::collections::HashSet::new();
        for p in &lib {
            assert!(names.insert(p.name.clone()), "two profiles are called {}", p.name);
        }
        let mut seen = std::collections::HashMap::new();
        for p in &lib {
            let Lighting::PerKey { keys, .. } = &p.lighting else { continue };
            let sig: Vec<_> = keys.values().copied().collect();
            if let Some(other) = seen.insert(sig, p.id.clone()) {
                panic!("{} and {other} look identical", p.id);
            }
            let dark = keys.values().filter(|c| (c.0 as u32 + c.1 as u32 + c.2 as u32) < 18).count();
            assert!(dark * 10 < keys.len() * 6, "{}: {dark} keys are nearly off", p.id);
        }
    }

    #[test]
    fn no_two_built_in_profiles_have_identical_lighting() {
        let lib = builtin(&Layout::tk68());
        let mut seen = std::collections::HashMap::new();
        for p in &lib {
            if let Some(other) = seen.insert(lighting_key(&p.lighting), p.id.clone()) {
                panic!("{} and {other} light up exactly the same way", p.id);
            }
        }
    }

    #[test]
    fn every_pattern_uses_more_than_one_colour() {
        let layout = Layout::tk68();
        let pal = palettes::all().into_iter().find(|p| p.id == "psychedelic").unwrap();
        for pat in patterns::all() {
            let keys = render(&layout, &pat, &pal);
            let mut distinct: Vec<_> = keys.values().collect();
            distinct.sort_by_key(|c| (c.0, c.1, c.2));
            distinct.dedup();
            assert!(distinct.len() >= 2, "{} renders only {} colours", pat.id, distinct.len());
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let layout = Layout::tk68();
        assert_eq!(builtin(&layout), builtin(&layout));
    }
}
