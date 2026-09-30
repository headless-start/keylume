//! Themed collections (heroes, myths, nature, space, …) and game collections, read from
//! the JSON files in `themes/`. Each theme is a palette rendered through every pattern,
//! plus one keyboard animation and one live effect in its colours; in a game collection
//! it also lights the game's default keys by role.
//!
//! Colour schemes can't be owned, but names can: themes use our own names, never a
//! franchise's, character's or brand's (see [`check`]).

use std::collections::HashSet;
use std::sync::OnceLock;

use keylume_live::LiveEffect;
use keylume_proto::{Effect, Layout, Mode, Rgb};
use serde::Deserialize;

use crate::palettes::Palette;
use crate::{chroma_i32, patterns, pick_unused, sum_i32, Lighting, Profile, Source};

/// The collections, in the order the Library lists them (comics, themes, then games).
const FILES: &[(&str, &str)] = &[
    ("heroes.json", include_str!("../themes/heroes.json")),
    ("villains.json", include_str!("../themes/villains.json")),
    ("squads.json", include_str!("../themes/squads.json")),
    ("myths.json", include_str!("../themes/myths.json")),
    ("nature.json", include_str!("../themes/nature.json")),
    ("space.json", include_str!("../themes/space.json")),
    ("festivals.json", include_str!("../themes/festivals.json")),
    ("abstract.json", include_str!("../themes/abstract.json")),
    ("art.json", include_str!("../themes/art.json")),
    ("food.json", include_str!("../themes/food.json")),
    ("cities.json", include_str!("../themes/cities.json")),
    ("music.json", include_str!("../themes/music.json")),
    ("zodiac.json", include_str!("../themes/zodiac.json")),
    ("elements.json", include_str!("../themes/elements.json")),
    ("scifi.json", include_str!("../themes/scifi.json")),
    ("call-of-duty.json", include_str!("../themes/call-of-duty.json")),
    ("dota-2.json", include_str!("../themes/dota-2.json")),
    ("fortnite.json", include_str!("../themes/fortnite.json")),
    ("valorant.json", include_str!("../themes/valorant.json")),
    ("league-of-legends.json", include_str!("../themes/league-of-legends.json")),
    ("apex-legends.json", include_str!("../themes/apex-legends.json")),
    ("overwatch-2.json", include_str!("../themes/overwatch-2.json")),
    ("pubg.json", include_str!("../themes/pubg.json")),
    ("minecraft.json", include_str!("../themes/minecraft.json")),
    ("rainbow-six-siege.json", include_str!("../themes/rainbow-six-siege.json")),
    ("rocket-league.json", include_str!("../themes/rocket-league.json")),
    ("gta-v.json", include_str!("../themes/gta-v.json")),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    /// The collection's name in the Library, e.g. "Heroes".
    pub collection: String,
    /// Every theme id starts with `{prefix}-`, so ids never clash across collections.
    pub prefix: String,
    /// The Library section it's listed in: "Themes" (the default), "Games" or "Comics".
    #[serde(default = "themes_section")]
    pub section: String,
    /// Games: the keys the game uses by default, most important role first. Each theme
    /// then also comes as a "Game Keys" layout lighting them.
    #[serde(default)]
    pub keys: Vec<KeyRole>,
    pub themes: Vec<Theme>,
}

fn themes_section() -> String {
    crate::section::THEMES.into()
}

/// A group of keys with one job in a game (e.g. "movement": w a s d).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyRole {
    pub role: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    /// From the light / surface end to the deep end, like a palette's stops.
    pub stops: Vec<Rgb>,
    /// A dim background for patterns that need one.
    pub base: Rgb,
    /// A highlight colour.
    pub accent: Rgb,
    pub animation: Animation,
    pub live: LiveKind,
    /// `tags`, borrowed for as long as the palette lives (made once, on first use).
    #[serde(skip)]
    static_tags: OnceLock<&'static [&'static str]>,
}

/// The keyboard animation that comes with a theme.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub mode: Mode,
    pub color: Rgb,
}

/// The live effect that comes with a theme (all of them use the theme's stops).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LiveKind {
    Flow,
    Lava,
    Trip,
    Breathe,
    Aurora,
    Rave,
}

impl LiveKind {
    /// The kinds a theme gets besides the one it asked for.
    const EXTRAS: [LiveKind; 5] = [LiveKind::Lava, LiveKind::Aurora, LiveKind::Trip, LiveKind::Breathe, LiveKind::Rave];
}

impl LiveKind {
    fn effect(self, colors: &[Rgb]) -> (&'static str, &'static str, LiveEffect) {
        let colors = colors.to_vec();
        match self {
            LiveKind::Flow => ("Live Flow", "flowing through its colours", LiveEffect::PaletteFlow { colors, period: 12.0 }),
            LiveKind::Lava => ("Live Lava", "welling up like a lava lamp", LiveEffect::Lava { colors, speed: 0.8 }),
            LiveKind::Trip => ("Live Trip", "drifting through its colours", LiveEffect::Trip { colors, speed: 0.8 }),
            LiveKind::Breathe => ("Live Breath", "breathing a new colour each time", LiveEffect::Breathe { colors, bpm: 10.0 }),
            LiveKind::Aurora => ("Live Aurora", "drifting overhead like the northern lights", LiveEffect::Aurora { colors, speed: 1.0 }),
            LiveKind::Rave => ("Live Rave", "cutting between its colours on the beat", LiveEffect::Rave { colors, bpm: 128.0 }),
        }
    }
}

/// Keyboard animations that suit a theme (colour-led, not host-driven).
pub const THEME_MODES: &[Mode] = &[
    Mode::Breathing,
    Mode::Wave,
    Mode::Ripple,
    Mode::Raindrop,
    Mode::Snake,
    Mode::Reactive,
    Mode::Converge,
    Mode::SineWave,
    Mode::Kaleidoscope,
    Mode::LineWave,
    Mode::Laser,
    Mode::CircleWave,
    Mode::Dazzle,
    Mode::RainDown,
    Mode::Meteor,
    Mode::ReactiveOff,
];

/// Franchise, character and brand names that must not appear in a theme.
const NOT_OURS: &[&str] = &[
    "marvel",
    "dc comics",
    "disney",
    "pixar",
    "warner bros",
    "avengers",
    "x-men",
    "justice league",
    "spider-man",
    "spiderman",
    "spider man",
    "batman",
    "superman",
    "wonder woman",
    "iron man",
    "ironman",
    "captain america",
    "hulk",
    "black panther",
    "wakanda",
    "thanos",
    "joker",
    "harley quinn",
    "gotham",
    "krypton",
    "kryptonite",
    "deadpool",
    "wolverine",
    "aquaman",
    "green lantern",
    "catwoman",
    "doctor strange",
    "scarlet witch",
    "black widow",
    "hawkeye",
    "ant-man",
    "groot",
    "star-lord",
    "guardians of the galaxy",
    "vibranium",
    "adamantium",
    "infinity stone",
    "infinity gauntlet",
    "tony stark",
    "bruce wayne",
    "clark kent",
    "peter parker",
    "lex luthor",
    "darkseid",
    "magneto",
    "jean grey",
    "phoenix force",
    "dark knight",
    "man of steel",
    "web-slinger",
    "web slinger",
    "caped crusader",
    "daredevil",
    "punisher",
    "silver surfer",
    "galactus",
    "fantastic four",
    "star wars",
    "jedi",
    "sith",
    "lightsaber",
    "darth",
    "star trek",
    "pokemon",
    "pokémon",
    "pikachu",
    "nintendo",
    "mario",
    "zelda",
    "hyrule",
    "game boy",
    "gameboy",
    "playstation",
    "xbox",
    "sega",
    "minecraft",
    "fortnite",
    "valorant",
    "overwatch",
    "league of legends",
    "barbie",
    "lego",
    "hello kitty",
    "coca-cola",
    "pepsi",
    "fanta",
    "starbucks",
    "skittles",
    "oreo",
    "nutella",
    "red bull",
    "ferrari",
    "lamborghini",
    "porsche",
    "gucci",
    "harry potter",
    "hogwarts",
    "gryffindor",
    "slytherin",
    "middle-earth",
    "gandalf",
    "sauron",
    "mordor",
    "game of thrones",
    "stranger things",
    "godzilla",
    "transformers",
    "autobot",
    "decepticon",
    "ghostbusters",
    "blade runner",
    "cyberpunk 2077",
    "shrek",
    "simpsons",
    "spongebob",
    "teenage mutant",
    "tmnt",
];

/// The names in `text` that belong to someone else (franchises, characters, brands: see
/// `NOT_OURS`), as whole words, ignoring case. Themes and pack designs must have none.
pub fn not_ours(text: &str) -> Vec<&'static str> {
    let text = format!(" {} ", text.to_lowercase());
    NOT_OURS
        .iter()
        .copied()
        .filter(|word| {
            let w = word.to_lowercase();
            // whole words only, so an ordinary word that merely contains one still passes
            text.match_indices(&w).any(|(i, _)| {
                let before = text[..i].chars().next_back().is_none_or(|ch| !ch.is_alphanumeric());
                let after = text[i + w.len()..].chars().next().is_none_or(|ch| !ch.is_alphanumeric());
                before && after
            })
        })
        .collect()
}

/// Everything wrong with a theme file (empty when it's good). `taken` holds names and
/// ids already used elsewhere in the library, lower-cased.
pub fn check(json: &str, taken: &HashSet<String>) -> Result<Collection, Vec<String>> {
    let c: Collection = serde_json::from_str(json).map_err(|e| vec![format!("not a valid theme file: {e}")])?;
    let mut bad = Vec::new();
    let kebab = |s: &str| {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !s.starts_with('-') && !s.ends_with('-')
    };
    if c.collection.trim().is_empty() || c.collection.len() > 20 {
        bad.push(format!("collection name {:?} should be 1-20 characters", c.collection));
    }
    if ![crate::section::THEMES, crate::section::GAMES, crate::section::COMICS].contains(&c.section.as_str()) {
        bad.push(format!("section {:?} should be \"Themes\", \"Games\" or \"Comics\"", c.section));
    }
    if !c.keys.is_empty() {
        let layout = Layout::tk68();
        let mut used = HashSet::new();
        if c.section != crate::section::GAMES || c.keys.len() > 10 {
            bad.push("key roles are for game collections (\"section\": \"Games\"), at most 10 of them".into());
        }
        for r in &c.keys {
            if !kebab(&r.role) || r.keys.is_empty() {
                bad.push(format!("role {:?}: a kebab-case name and at least one key", r.role));
            }
            for k in &r.keys {
                if layout.key(k).is_none() {
                    bad.push(format!("role {:?}: {k:?} isn't a key on the 68-key layout (see docs/PROFILES.md)", r.role));
                } else if !used.insert(k.as_str()) {
                    bad.push(format!("role {:?}: {k:?} is already in another role", r.role));
                }
            }
        }
    }
    if !kebab(&c.prefix) {
        bad.push(format!("prefix {:?} should be kebab-case", c.prefix));
    }
    let lum = |c: Rgb| c.0 as u32 + c.1 as u32 + c.2 as u32;
    let bright = |c: Rgb| c.0.max(c.1).max(c.2);
    let (mut ids, mut names) = (HashSet::new(), HashSet::new());
    for t in &c.themes {
        let who = format!("{} ({})", t.id, t.name);
        if !kebab(&t.id) || !t.id.starts_with(&format!("{}-", c.prefix)) || t.id.len() > 36 {
            bad.push(format!("{who}: id must be kebab-case, start with \"{}-\" and be at most 36 characters", c.prefix));
        }
        if !ids.insert(t.id.clone()) || taken.contains(&t.id) {
            bad.push(format!("{who}: the id is already used"));
        }
        let lname = t.name.to_lowercase();
        if t.name.trim() != t.name || !(3..=24).contains(&t.name.chars().count()) || t.name.contains('·') {
            bad.push(format!("{who}: the name must be 3-24 characters, without '·' or outer spaces"));
        }
        if !names.insert(lname.clone()) || taken.contains(&lname) {
            bad.push(format!("{who}: the name is already used"));
        }
        let n = t.description.chars().count();
        if !(12..=100).contains(&n) || t.description.ends_with('.') || !t.description.starts_with(|ch: char| ch.is_uppercase()) {
            bad.push(format!("{who}: the description must be 12-100 characters, start with a capital and not end with '.'"));
        }
        if !(2..=6).contains(&t.tags.len()) || t.tags.iter().any(|g| !kebab(g) || g.len() > 16) {
            bad.push(format!("{who}: 2-6 tags, each lower-case kebab-case and at most 16 characters"));
        }
        let text = format!("{} {} {} {}", t.id.replace('-', " "), lname, t.description, t.tags.join(" "));
        for word in not_ours(&text) {
            bad.push(format!("{who}: {word:?} is someone else's name; describe the colours in your own words"));
        }
        if !(3..=6).contains(&t.stops.len()) {
            bad.push(format!("{who}: 3-6 stops"));
        }
        if t.stops.iter().filter(|s| bright(**s) >= 0xa0).count() < 2 {
            bad.push(format!("{who}: at least two stops need a channel at 0xa0 or more (LEDs show dark colours as off)"));
        }
        if let Some(s) = t.stops.iter().find(|s| lum(**s) < 60) {
            bad.push(format!("{who}: stop {s} is too dark for an LED (r+g+b under 60)"));
        }
        if !(24..=240).contains(&lum(t.base)) {
            bad.push(format!("{who}: base {} should be dim but not off (r+g+b between 24 and 240)", t.base));
        }
        if bright(t.accent) < 0x90 {
            bad.push(format!("{who}: accent {} is too dark", t.accent));
        }
        if !THEME_MODES.contains(&t.animation.mode) {
            bad.push(format!("{who}: animation mode {:?} isn't one of the theme modes", t.animation.mode));
        }
        if bright(t.animation.color) < 0xa0 {
            bad.push(format!("{who}: animation colour {} is too dark", t.animation.color));
        }
    }
    // two themes that are nearly the same colours
    for (i, a) in c.themes.iter().enumerate() {
        for b in &c.themes[i + 1..] {
            if a.stops.len() == b.stops.len() {
                let d = a
                    .stops
                    .iter()
                    .zip(&b.stops)
                    .map(|(x, y)| x.0.abs_diff(y.0).max(x.1.abs_diff(y.1)).max(x.2.abs_diff(y.2)))
                    .max()
                    .unwrap_or(0);
                if d < 32 {
                    bad.push(format!("{} and {} have nearly the same colours", a.id, b.id));
                }
            }
        }
    }
    if bad.is_empty() {
        Ok(c)
    } else {
        Err(bad)
    }
}

/// The registered collections, parsed once.
pub fn collections() -> &'static [Collection] {
    static ALL: OnceLock<Vec<Collection>> = OnceLock::new();
    ALL.get_or_init(|| FILES.iter().map(|(file, json)| serde_json::from_str(json).unwrap_or_else(|e| panic!("themes/{file}: {e}"))).collect())
}

impl Theme {
    /// This theme as a palette (for the patterns).
    pub fn palette(&'static self, family: &'static str) -> Palette {
        let tags = *self.static_tags.get_or_init(|| Vec::leak(self.tags.iter().map(String::as_str).collect()));
        Palette {
            id: &self.id,
            name: &self.name,
            family,
            description: &self.description,
            tags,
            stops: self.stops.clone(),
            base: self.base,
            accent: self.accent,
            only: None,
        }
    }
}

/// Every themed profile, collection by collection, in a stable order. Within a
/// collection the themes take turns (each in a different pattern, then its animation and
/// live effect, then the other patterns), so browsing one shows its variety first.
pub(crate) fn profiles(layout: &Layout, ctx: &patterns::Ctx, seen: &mut HashSet<String>) -> Vec<Profile> {
    let bundled: Vec<&'static Collection> = collections().iter().collect();
    render(&bundled, layout, ctx, seen)
}

/// Every profile of `collections` (bundled ones, or a pack's), in the Library's order.
pub(crate) fn render(collections: &[&'static Collection], layout: &Layout, ctx: &patterns::Ctx, seen: &mut HashSet<String>) -> Vec<Profile> {
    let pats = patterns::all();
    let mut out = Vec::new();
    for &c in collections {
        let sets: Vec<Vec<Profile>> = c.themes.iter().enumerate().map(|(i, t)| theme_profiles(layout, ctx, &pats, c, t, i, seen)).collect();
        for round in 0..sets.iter().map(Vec::len).max().unwrap_or(0) {
            out.extend(sets.iter().filter_map(|s| s.get(round).cloned()));
        }
    }
    out
}

/// A colour's hue in degrees (0..360, meaningless for greys) and its chroma (0..1: 0 is
/// grey, 1 is fully saturated), matching the audit tooling's `colorsys`-based maths.
pub fn hue_chroma(c: Rgb) -> (f32, f32) {
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let chroma = max - min;
    if chroma < 1e-6 {
        return (0.0, 0.0);
    }
    let hue = if max == r {
        60.0 * ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / chroma + 2.0)
    } else {
        60.0 * ((r - g) / chroma + 4.0)
    };
    (hue.rem_euclid(360.0), chroma)
}

/// A colour needs at least this much chroma to count as showing its hue, not just tinting
/// white or grey (also used by the catalogue checker).
pub const REAL_COLOR_CHROMA: f32 = 0.25;

/// Colour words a theme name might use, and the hue range(s) in degrees that count as
/// that colour (a range that would wrap past 360 is split into two). "Royal" is skipped:
/// it isn't really a colour on its own. Kept in sync with the catalogue checker.
pub const COLOR_WORDS: &[(&str, &[(f32, f32)])] = &[
    ("red", &[(345.0, 360.0), (0.0, 15.0)]),
    ("crimson", &[(335.0, 360.0), (0.0, 10.0)]),
    ("scarlet", &[(350.0, 360.0), (0.0, 15.0)]),
    ("ruby", &[(330.0, 360.0), (0.0, 10.0)]),
    ("blood", &[(340.0, 360.0), (0.0, 15.0)]),
    ("rose", &[(320.0, 360.0), (0.0, 5.0)]),
    ("pink", &[(290.0, 350.0)]),
    ("magenta", &[(280.0, 330.0)]),
    ("fuchsia", &[(285.0, 330.0)]),
    ("purple", &[(255.0, 300.0)]),
    ("violet", &[(250.0, 295.0)]),
    ("lavender", &[(240.0, 290.0)]),
    ("indigo", &[(225.0, 265.0)]),
    ("blue", &[(195.0, 250.0)]),
    ("sapphire", &[(205.0, 245.0)]),
    ("cobalt", &[(205.0, 240.0)]),
    ("navy", &[(210.0, 250.0)]),
    ("azure", &[(190.0, 220.0)]),
    ("cyan", &[(170.0, 200.0)]),
    ("aqua", &[(160.0, 200.0)]),
    ("teal", &[(160.0, 195.0)]),
    ("turquoise", &[(160.0, 190.0)]),
    ("mint", &[(130.0, 175.0)]),
    ("jade", &[(130.0, 170.0)]),
    ("green", &[(75.0, 165.0)]),
    ("emerald", &[(120.0, 165.0)]),
    ("lime", &[(65.0, 110.0)]),
    ("olive", &[(50.0, 90.0)]),
    ("yellow", &[(45.0, 65.0)]),
    ("gold", &[(35.0, 58.0)]),
    ("golden", &[(35.0, 58.0)]),
    ("amber", &[(28.0, 48.0)]),
    ("orange", &[(15.0, 40.0)]),
    ("ember", &[(5.0, 35.0)]),
    ("copper", &[(10.0, 35.0)]),
    ("rust", &[(5.0, 30.0)]),
    ("bronze", &[(20.0, 45.0)]),
];

/// The colour words from [`COLOR_WORDS`] that appear as whole words in `name` (case
/// insensitive), with their hue ranges.
pub fn color_words(name: &str) -> Vec<(&'static str, &'static [(f32, f32)])> {
    let words: Vec<String> = name.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).map(str::to_ascii_lowercase).collect();
    COLOR_WORDS.iter().filter(|(w, _)| words.iter().any(|nw| nw == w)).copied().collect()
}

/// When the theme's name names a colour, the theme's own stop or accent that best matches
/// it (most saturated, brightest first); `None` when the name names no colour, or none of
/// the theme's colours actually fall in that colour's hue range with real colour in it.
fn named_hue_match(t: &Theme) -> Option<Rgb> {
    let ranges: Vec<(f32, f32)> = color_words(&t.name).into_iter().flat_map(|(_, r)| r.iter().copied()).collect();
    if ranges.is_empty() {
        return None;
    }
    std::iter::once(t.accent)
        .chain(t.stops.iter().copied())
        .filter(|c| {
            let (hue, chroma) = hue_chroma(*c);
            chroma >= REAL_COLOR_CHROMA && ranges.iter().any(|&(a, b)| (a..=b).contains(&hue))
        })
        .max_by_key(|c| (chroma_i32(c), sum_i32(c)))
}

/// The colour a theme is known by: the stop or accent matching a colour word in its name,
/// or (when its name names no colour, or none of its colours fall in that colour's hue
/// range) its most colourful shade, brightest first, so a white accent never stands in
/// for it (its animations, bar patterns and music visualiser use this).
fn signature(t: &Theme) -> Rgb {
    named_hue_match(t)
        .unwrap_or_else(|| std::iter::once(t.accent).chain(t.stops.iter().copied()).max_by_key(|c| (chroma_i32(c), sum_i32(c))).unwrap_or(t.accent))
}

/// A game's default keys lit by role in a theme's colours (the accent first, then the
/// brightest stops); every other key dim in the theme's base.
pub fn game_keys(layout: &Layout, roles: &[KeyRole], t: &Theme) -> std::collections::BTreeMap<String, Rgb> {
    let mut stops = t.stops.clone();
    stops.sort_by_key(|c| std::cmp::Reverse(c.0.max(c.1).max(c.2)));
    let mut colours = vec![t.accent];
    for c in stops {
        if !colours.contains(&c) {
            colours.push(c);
        }
    }
    let mut keys: std::collections::BTreeMap<String, Rgb> = layout.keys.iter().map(|k| (k.id.clone(), t.base)).collect();
    for (i, r) in roles.iter().enumerate() {
        for k in &r.keys {
            keys.insert(k.clone(), colours[i % colours.len()]);
        }
    }
    keys
}

/// An animation pick for a theme: `preferred_mode` in `color`, or (in order) a clash with
/// one already produced takes the next mode, then the next of `all`'s other colours.
/// Never repeats a mode this theme already used (a repeat would need the same name).
fn pick_theme_anim(
    preferred_mode: Mode,
    color: Rgb,
    all: &[(Rgb, Mode)],
    used_modes: &mut Vec<Mode>,
    seen: &mut HashSet<String>,
    light: impl Fn((Rgb, Mode)) -> Lighting + Copy,
) -> (Rgb, Mode) {
    let candidates: Vec<(Rgb, Mode)> = all.iter().copied().filter(|(_, m)| !used_modes.contains(m)).collect();
    let pick = pick_unused((color, preferred_mode), &candidates, &mut Vec::new(), seen, light);
    used_modes.push(pick.1);
    pick
}

/// One theme's profiles: its patterns starting from the `turn`-th, with the animation and
/// live effect second and third (and a game's key layout first).
fn theme_profiles(
    layout: &Layout,
    ctx: &patterns::Ctx,
    pats: &[patterns::Pattern],
    c: &'static Collection,
    t: &'static Theme,
    turn: usize,
    seen: &mut HashSet<String>,
) -> Vec<Profile> {
    let (collection, section) = (c.collection.as_str(), c.section.as_str());
    let pal = t.palette(collection);
    let mk = |id: String, name: String, description: String, extra: &[&str], lighting| Profile {
        id,
        name,
        category: collection.to_string(),
        tags: t.tags.iter().cloned().chain(extra.iter().map(|s| s.to_string())).collect(),
        description,
        source: Source::Builtin,
        section: section.to_string(),
        lighting,
    };
    let mut out: Vec<Profile> = (0..pats.len())
        .map(|k| &pats[(turn + k) % pats.len()])
        .map(|pat| {
            mk(
                format!("{}-{}", t.id, pat.id),
                format!("{} · {}", t.name, pat.name),
                format!("{} rendered as {}.", t.description, pat.description),
                pat.tags,
                Lighting::PerKey { keys: crate::render_ctx(layout, ctx, pat, &pal), brightness: keylume_proto::led::MAX_BRIGHTNESS },
            )
        })
        .collect();
    // The animation colour: the theme's own colour word (see `named_hue_match`) when its
    // stops or accent actually have a match for it, otherwise the colour the theme was
    // authored with, exactly as before. A clash with an animation another theme already
    // produced takes the next mode round `THEME_MODES` instead; a handful of colours are
    // reused by a lot of themes, so once every mode is taken for one exact colour, the
    // theme's next most colourful stop or accent takes over as well.
    let anim_color = named_hue_match(t).unwrap_or(t.animation.color);
    let mut anim_alts: Vec<Rgb> = std::iter::once(t.accent).chain(t.stops.iter().copied()).filter(|c| *c != anim_color).collect();
    anim_alts.sort_by_key(|c| std::cmp::Reverse((chroma_i32(c), sum_i32(c))));
    anim_alts.dedup();
    let anim_colors: Vec<Rgb> = std::iter::once(anim_color).chain(anim_alts).collect();
    let anim_all: Vec<(Rgb, Mode)> = anim_colors.iter().flat_map(|&c| THEME_MODES.iter().map(move |&m| (c, m))).collect();
    let anim_light =
        |(color, mode): (Rgb, Mode)| Lighting::Effect { effect: Effect { mode, speed: 2, brightness: 4, direction: 0, rainbow: false, color } };
    let mut used_modes = Vec::new();
    let anim_pick = pick_theme_anim(t.animation.mode, anim_color, &anim_all, &mut used_modes, seen, anim_light);
    let anim_mode = anim_pick.1;
    let name = anim_mode.info().name;
    let animated = mk(
        format!("{}-animated", t.id),
        format!("{} · {name} Animation", t.name),
        format!("{}, as the keyboard's {} animation.", t.description, name.to_lowercase()),
        &["animated"],
        anim_light(anim_pick),
    );
    let (label, how, live) = t.live.effect(&t.stops);
    let live = mk(
        format!("{}-live", t.id),
        format!("{} · {label}", t.name),
        format!("{}, {how}.", t.description),
        &["live", "animated"],
        Lighting::Live { live },
    );
    // Every theme also comes with more of both: a few other keyboard animations, another
    // live effect, a bar pattern and a music visualiser, so each has plenty that moves.
    let mut more = vec![animated, live];
    for k in 1..4 {
        let mode = THEME_MODES[(turn * 3 + k * 5) % THEME_MODES.len()];
        if mode == anim_mode {
            continue;
        }
        let pick = pick_theme_anim(mode, anim_color, &anim_all, &mut used_modes, seen, anim_light);
        let name = pick.1.info().name;
        more.push(mk(
            format!("{}-anim-{}", t.id, name.to_lowercase().replace(' ', "-")),
            format!("{} · {name} Animation", t.name),
            format!("{}, as the keyboard's {} animation.", t.description, name.to_lowercase()),
            &["animated"],
            anim_light(pick),
        ));
    }
    let preferred_extra = LiveKind::EXTRAS[turn % LiveKind::EXTRAS.len()];
    if preferred_extra != t.live {
        // A clash takes the next extra kind instead (never this theme's own primary one).
        let live_light = |k: LiveKind| Lighting::Live { live: k.effect(&t.stops).2 };
        let mut used_live = vec![t.live];
        let extra = pick_unused(preferred_extra, &LiveKind::EXTRAS, &mut used_live, seen, live_light);
        let (label, how, live) = extra.effect(&t.stops);
        more.push(mk(
            format!("{}-{}", t.id, label.to_lowercase().replace(' ', "-")),
            format!("{} · {label}", t.name),
            format!("{}, {how}.", t.description),
            &["live", "animated"],
            Lighting::Live { live },
        ));
    }
    // A clash takes the next bar style round `BarStyle::ALL` instead.
    let bars_light =
        |style: keylume_live::bars::BarStyle| Lighting::Live { live: LiveEffect::Bars { style, color: signature(t), rainbow: false, speed: 1.0 } };
    let style = pick_unused(
        keylume_live::bars::BarStyle::ALL[turn % keylume_live::bars::BarStyle::ALL.len()],
        &keylume_live::bars::BarStyle::ALL,
        &mut Vec::new(),
        seen,
        bars_light,
    );
    more.push(mk(
        format!("{}-bars", t.id),
        format!("{} · {} Bars", t.name, style.label()),
        format!("{}, drawn on the 32 bars.", t.description),
        &["live", "animated", "bars"],
        bars_light(style),
    ));
    // A clash takes the theme's next most colourful stop or accent instead, then (if even
    // those are all taken) mirrors the bass to the middle instead of running it edge to edge.
    let sig = signature(t);
    let mut alt: Vec<Rgb> = std::iter::once(t.accent).chain(t.stops.iter().copied()).filter(|c| *c != sig).collect();
    alt.sort_by_key(|c| std::cmp::Reverse((chroma_i32(c), sum_i32(c))));
    alt.dedup();
    let music_order: Vec<(Rgb, bool)> = std::iter::once((sig, false))
        .chain(alt.iter().map(|c| (*c, false)))
        .chain(std::iter::once((sig, true)))
        .chain(alt.iter().map(|c| (*c, true)))
        .collect();
    let music_light = |(color, mirror): (Rgb, bool)| Lighting::Live { live: LiveEffect::Spectrum { color, rainbow: false, mirror } };
    let music = pick_unused(music_order[0], &music_order, &mut Vec::new(), seen, music_light);
    more.push(mk(
        format!("{}-music", t.id),
        format!("{} · Music Bars", t.name),
        format!("{}, dancing to your music.", t.description),
        &["live", "animated", "music"],
        music_light(music),
    ));
    out.splice(1..1, more);
    if !c.keys.is_empty() {
        let roles: Vec<&str> = c.keys.iter().map(|r| r.role.as_str()).collect();
        out.insert(
            0,
            mk(
                format!("{}-keys", t.id),
                format!("{} · Game Keys", t.name),
                format!("{}, with {collection}'s default keys lit by role ({}).", t.description, roles.join(", ").replace('-', " ")),
                &["keys", "gaming"],
                Lighting::PerKey { keys: game_keys(layout, &c.keys, t), brightness: keylume_proto::led::MAX_BRIGHTNESS },
            ),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    /// Names and ids the rest of the library uses (as the checker sees them).
    fn taken_outside(lib: &[Profile], collection: &str) -> HashSet<String> {
        lib.iter()
            .filter(|p| p.category != collection)
            .flat_map(|p| [p.id.to_lowercase(), p.name.split(" · ").next().unwrap_or(&p.name).to_lowercase()])
            .collect()
    }

    #[test]
    fn every_theme_file_passes_the_checker() {
        let lib = builtin(&Layout::tk68());
        for (file, json) in FILES {
            let c: Collection = serde_json::from_str(json).unwrap();
            if let Err(bad) = check(json, &taken_outside(&lib, &c.collection)) {
                panic!("themes/{file}:\n{}", bad.join("\n"));
            }
        }
    }

    #[test]
    fn every_theme_becomes_a_full_set_of_profiles_in_its_collection() {
        let lib = builtin(&Layout::tk68());
        let games = collections().iter().filter(|c| c.section == crate::section::GAMES).count();
        assert!(games >= 12 && collections().len() - games >= 13, "{games} games of {}", collections().len());
        for c in collections() {
            let game = c.section == crate::section::GAMES;
            // every pattern, plus the animations and live effects each theme comes with
            let least = patterns::all().len() + if game { 6 } else { 5 };
            assert!(c.themes.len() >= if game { 5 } else { 20 }, "{} has only {} themes", c.collection, c.themes.len());
            assert_eq!(game, !c.keys.is_empty(), "{}: games (and only games) light their keys", c.collection);
            let mine: Vec<_> = lib.iter().filter(|p| p.category == c.collection).collect();
            assert!(
                (c.themes.len() * least..=c.themes.len() * (least + 3)).contains(&mine.len()),
                "{}: {} profiles for {} themes",
                c.collection,
                mine.len(),
                c.themes.len()
            );
            assert!(mine.iter().all(|p| p.section == c.section), "{}", c.collection);
        }
        for id in ["hero-web-line-cascade", "hero-web-line-animated", "hero-web-line-live", "food-dragon-fruit-plasma", "dota-jungle-dawn-keys"] {
            assert!(lib.iter().any(|p| p.id == id), "missing {id}");
        }
    }

    #[test]
    fn the_checker_refuses_other_peoples_names_and_colours_leds_cant_show() {
        let theme = |name: &str, stops: &str| {
            format!(
                r##"{{ "collection": "Heroes", "prefix": "hero", "themes": [ {{ "id": "hero-x", "name": "{name}", "description": "A test theme for the checker",
                "tags": ["hero", "test"], "stops": [{stops}], "base": "#101020", "accent": "#ffffff",
                "animation": {{ "mode": "wave", "color": "#ff2020" }}, "live": "flow" }} ] }}"##
            )
        };
        let ok = r##""#ff4040", "#ff1030", "#2050ff""##;
        assert!(check(&theme("Night Swing", ok), &HashSet::new()).is_ok());
        let bad = check(&theme("Batman Night", ok), &HashSet::new()).unwrap_err();
        assert!(bad.iter().any(|b| b.contains("\"batman\"")), "{bad:?}");
        // whole words only: "Marionette" isn't a plumber's name
        assert!(check(&theme("Marionette", ok), &HashSet::new()).is_ok());
        let bad = check(&theme("Dim", r##""#101010", "#202020", "#303030""##), &HashSet::new()).unwrap_err();
        assert!(bad.iter().any(|b| b.contains("too dark")), "{bad:?}");
        assert!(check(&theme("Night Swing", ok), &["night swing".to_string()].into()).is_err(), "names are unique");
    }
}
