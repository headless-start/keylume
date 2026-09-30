//! Profiles inspired by famous Counter-Strike 2 weapon skins: each skin's colours as
//! a few fitting per-key designs, one keyboard animation and one live effect.
//! (Colour inspiration only; names are the community's names for the finishes.)

use keylume_live::LiveEffect;
use keylume_proto::{Effect, Layout, Mode, Rgb};

use crate::palettes::{pal, Palette};
use crate::{patterns, render, Lighting, Profile, Source};

struct Skin {
    palette: Palette,
    /// The weapon(s) it's best known on.
    weapon: &'static str,
    /// Per-key patterns that suit it.
    patterns: [&'static str; 4],
    /// Keyboard animation: mode, colour, rainbow.
    animation: (Mode, &'static str, bool),
}

#[allow(clippy::too_many_arguments)] // a table row: every field is named in the data below
fn skin(
    id: &'static str,
    name: &'static str,
    weapon: &'static str,
    description: &'static str,
    stops: &[&str],
    base: &str,
    accent: &str,
    patterns: [&'static str; 4],
    animation: (Mode, &'static str, bool),
) -> Skin {
    Skin { palette: pal(id, name, "CS2 Skins", description, &["cs", "skin", "gaming"], stops, base, accent), weapon, patterns, animation }
}

fn skins() -> Vec<Skin> {
    vec![
        skin(
            "asiimov",
            "Asiimov",
            "AWP · M4A4",
            "Sci-fi white panels, safety orange and black",
            &["#ffffff", "#ff7a1a", "#ff4a00", "#3a3a3a"],
            "#141414",
            "#ff7a1a",
            ["two-tone", "split", "frame", "chevron"],
            (Mode::Wave, "#ff6a00", false),
        ),
        skin(
            "dragon-lore",
            "Dragon Lore",
            "AWP",
            "Gilded tapestry: gold, sand and a green dragon",
            &["#ffe08a", "#ffb830", "#c07a10", "#2f8a3a"],
            "#1a1206",
            "#3fdf6a",
            ["aurora", "bloom", "marble", "galaxy-arms"],
            (Mode::Breathing, "#ffb830", false),
        ),
        skin(
            "fade",
            "Fade",
            "Knives · Glock",
            "The classic fade: yellow melting into pink and purple",
            &["#ffd23f", "#ff4fa3", "#b04dff", "#5a6bff"],
            "#140818",
            "#ffffff",
            ["horizon", "slope", "prism", "tidal"],
            (Mode::Wave, "#ff4fa3", true),
        ),
        skin(
            "doppler-sapphire",
            "Doppler Sapphire",
            "Doppler knives",
            "Doppler Sapphire: deep, glowing blue",
            &["#bff0ff", "#2f9dff", "#0030ff", "#0a1a80"],
            "#030826",
            "#ffffff",
            ["nebula", "galaxy-arms", "marble", "plasma"],
            (Mode::Breathing, "#0040ff", false),
        ),
        skin(
            "doppler-ruby",
            "Doppler Ruby",
            "Doppler knives",
            "Doppler Ruby: blood-red gemstone",
            &["#ffb0b0", "#ff2030", "#b00018", "#4a0010"],
            "#160004",
            "#ffffff",
            ["nebula", "galaxy-arms", "marble", "plasma"],
            (Mode::Breathing, "#ff1030", false),
        ),
        skin(
            "doppler-emerald",
            "Gamma Emerald",
            "Gamma Doppler knives",
            "Gamma Doppler Emerald: vivid gem green",
            &["#c0ffd0", "#20ff60", "#00a040", "#004a20"],
            "#021408",
            "#ffffff",
            ["nebula", "galaxy-arms", "marble", "plasma"],
            (Mode::Breathing, "#00ff50", false),
        ),
        skin(
            "black-pearl",
            "Doppler Black Pearl",
            "Doppler knives",
            "Doppler Black Pearl: violet sheen on near-black",
            &["#d0b8ff", "#7a5cff", "#3a1a9a", "#140a40"],
            "#08041a",
            "#ffffff",
            ["nebula", "galaxy-arms", "marble", "kaleidoscope"],
            (Mode::Breathing, "#6a3cff", false),
        ),
        skin(
            "marble-fade",
            "Marble Fade",
            "Knives",
            "Fire and ice: yellow, red, white and blue swirled together",
            &["#ffe000", "#ff3000", "#ffffff", "#0040ff"],
            "#0c0a14",
            "#ffffff",
            ["marble", "split", "tidal", "ripple-tank"],
            (Mode::Wave, "#ff3000", true),
        ),
        skin(
            "case-hardened",
            "Case Hardened",
            "AK-47 · knives",
            "Heat-treated steel: blue and gold patina",
            &["#3d7bff", "#1a4ab0", "#d4a020", "#8a8a9a"],
            "#0a0c14",
            "#ffd040",
            ["marble", "stained-glass", "lava-lamp", "glitch"],
            (Mode::Raindrop, "#3d7bff", false),
        ),
        skin(
            "blue-gem",
            "Blue Gem",
            "AK-47 · Case Hardened",
            "The legendary all-blue Case Hardened",
            &["#60b0ff", "#0060ff", "#1a2a9a", "#d4a020"],
            "#050a26",
            "#ffd040",
            ["marble", "lava-lamp", "stained-glass", "nebula"],
            (Mode::Ripple, "#0060ff", false),
        ),
        skin(
            "hyper-beast",
            "Hyper Beast",
            "M4A1-S · AWP",
            "Neon green, purple and pink street art",
            &["#40ff9a", "#b040ff", "#ff3fa0", "#00e5ff"],
            "#0a0014",
            "#ffffff",
            ["plasma", "confetti", "kaleidoscope", "glitch"],
            (Mode::Dazzle, "#40ff9a", true),
        ),
        skin(
            "neo-noir",
            "Neo-Noir",
            "USP-S · AWP · M4A4",
            "Comic-book noir: pink, violet and white",
            &["#ffffff", "#ff4fd8", "#8a2bff", "#2a1a4a"],
            "#0a0414",
            "#ff4fd8",
            ["two-tone", "hypno-spiral", "zebra", "frame"],
            (Mode::SineWave, "#ff4fd8", false),
        ),
        skin(
            "crimson-web",
            "Crimson Web",
            "Knives · Deagle",
            "A black spider web on crimson",
            &["#ff4050", "#e00020", "#8a0010", "#3a0008"],
            "#140002",
            "#ffffff",
            ["stained-glass", "moire", "tunnel", "ripple"],
            (Mode::Breathing, "#e00020", false),
        ),
        skin(
            "printstream",
            "Printstream",
            "Deagle · M4A1-S",
            "Pearlescent white and black with a violet shimmer",
            &["#ffffff", "#e8e0ff", "#b0a8d8", "#3a3a4a"],
            "#0e0e14",
            "#c8b0ff",
            ["two-tone", "marble", "split", "diamond"],
            (Mode::Wave, "#e8e0ff", false),
        ),
        skin(
            "howl",
            "Howl",
            "M4A4",
            "A howling wolf in fiery orange and red",
            &["#ffcc40", "#ff6a00", "#e02000", "#5a0c00"],
            "#140500",
            "#ffffff",
            ["flame", "bloom", "sunburst", "tidal"],
            (Mode::RainDown, "#ff5000", false),
        ),
        skin(
            "fire-serpent",
            "Fire Serpent",
            "AK-47",
            "An Aztec serpent in green, red and gold",
            &["#9aff40", "#2aa82a", "#ff3a1a", "#ffd23f"],
            "#0a1204",
            "#ffd23f",
            ["tidal", "aurora", "marble", "chevron"],
            (Mode::Snake, "#2aa82a", false),
        ),
        skin(
            "vulcan",
            "Vulcan",
            "AK-47",
            "Racing white and electric blue with an orange flash",
            &["#ffffff", "#2aa8ff", "#1a3aa8", "#ff8a00"],
            "#06060c",
            "#ff8a00",
            ["chevron", "stripes", "frame", "split"],
            (Mode::LineWave, "#2aa8ff", false),
        ),
        skin(
            "neon-rider",
            "Neon Rider",
            "AK-47 · MAC-10",
            "Synthwave magenta, cyan and yellow",
            &["#ff2fd0", "#00f0ff", "#fff200", "#7a00ff"],
            "#0a0014",
            "#ffffff",
            ["glitch", "plasma", "chevron", "horizon"],
            (Mode::Wave, "#ff2fd0", true),
        ),
        skin(
            "wild-lotus",
            "Wild Lotus",
            "AK-47",
            "Lotus flowers: jade, teal and gold",
            &["#50ffb0", "#00a080", "#ffd060", "#0a4a3a"],
            "#021410",
            "#ffd060",
            ["bloom", "galaxy-arms", "aurora", "stained-glass"],
            (Mode::Kaleidoscope, "#00c890", false),
        ),
        skin(
            "redline",
            "Redline",
            "AK-47 · AWP",
            "Carbon fibre with a racing-red stripe",
            &["#ff1020", "#a00010", "#ffffff", "#3a3a3a"],
            "#0c0204",
            "#ff1020",
            ["stripes", "two-tone", "frame", "split"],
            (Mode::Laser, "#ff1020", false),
        ),
        skin(
            "tiger-tooth",
            "Tiger Tooth",
            "Knives",
            "Tiger stripes in molten orange and gold",
            &["#fff08a", "#ffb000", "#ff7a00", "#5a3000"],
            "#140a00",
            "#ffffff",
            ["zebra", "chevron", "stripes", "flame"],
            (Mode::Wave, "#ffa000", false),
        ),
        skin(
            "gamma-doppler",
            "Gamma Doppler",
            "Knives · Glock",
            "Toxic lime and teal swirls",
            &["#d0ff60", "#40ff80", "#00c0a0", "#004a3a"],
            "#021008",
            "#ffffff",
            ["nebula", "marble", "plasma", "aurora"],
            (Mode::CircleWave, "#40ff80", false),
        ),
    ]
}

/// Every skin profile, in a stable order.
pub fn all(layout: &Layout) -> Vec<Profile> {
    let pats = patterns::all();
    let mut out = Vec::new();
    for s in skins() {
        let p = &s.palette;
        let tags: Vec<String> = p.tags.iter().map(|t| t.to_string()).collect();
        let mk = |id: String, name: String, desc: String, lighting: Lighting| Profile {
            id,
            name,
            category: "CS2 Skins".into(),
            tags: tags.clone(),
            description: desc,
            source: Source::Builtin,
            section: crate::section::GAMES.to_string(),
            lighting,
        };
        for pid in s.patterns {
            let pat = pats.iter().find(|x| x.id == pid).expect("skin pattern exists");
            out.push(mk(
                format!("skin-{}-{}", p.id, pid),
                format!("{} · {}", p.name, pat.name),
                format!("{} ({}), as {}.", p.description, s.weapon, pat.description),
                Lighting::PerKey { keys: render(layout, pat, p), brightness: keylume_proto::led::MAX_BRIGHTNESS },
            ));
        }
        let (mode, color, rainbow) = s.animation;
        let color: Rgb = color.parse().unwrap();
        out.push(mk(
            format!("skin-{}-animated", p.id),
            format!("{} · {}", p.name, mode.info().name),
            format!("{} ({}), as the keyboard's {} animation.", p.description, s.weapon, mode.info().name.to_lowercase()),
            Lighting::Effect { effect: Effect { mode, speed: 2, brightness: 4, direction: 0, rainbow, color } },
        ));
        out.push(mk(
            format!("skin-{}-flow", p.id),
            format!("{} · Flow", p.name),
            format!("{} ({}), drifting through its colours live.", p.description, s.weapon),
            Lighting::Live { live: LiveEffect::Trip { colors: p.stops.clone(), speed: 0.8 } },
        ));
    }
    out
}
