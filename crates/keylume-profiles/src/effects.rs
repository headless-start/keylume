//! Curated animated presets: every firmware effect × a set of tuned colours.

use keylume_proto::{Effect, Mode, Rgb};

use crate::{Lighting, Profile, Source};

/// (mode, display name, default speed 0-4, direction)
const MODES: &[(Mode, &str, u8, u8)] = &[
    (Mode::Static, "Solid", 0, 0),
    (Mode::Breathing, "Breathe", 1, 0),
    (Mode::Wave, "Wave", 2, 0),
    (Mode::Ripple, "Ripple", 3, 0),
    (Mode::Raindrop, "Starlight", 2, 0),
    (Mode::Snake, "Snake", 3, 1),
    (Mode::Reactive, "Keypress Glow", 2, 0),
    (Mode::Converge, "Converge", 2, 0),
    (Mode::SineWave, "Sine", 2, 0),
    (Mode::Kaleidoscope, "Kaleidoscope", 2, 0),
    (Mode::LineWave, "Line Sweep", 2, 0),
    (Mode::Laser, "Laser", 3, 0),
    (Mode::CircleWave, "Vortex", 2, 1),
    (Mode::Dazzle, "Dazzle", 2, 0),
    (Mode::RainDown, "Rain", 2, 0),
    (Mode::Meteor, "Meteor", 3, 0),
    (Mode::ReactiveOff, "Fade Trail", 2, 0),
];

/// (id, name, family, colour)
/// A full colour wheel (ids kept stable so favourites survive updates).
const COLORS: &[(&str, &str, &str, &str)] = &[
    ("red", "Red", "Red", "#ff1010"),
    ("crimson", "Crimson", "Red", "#ff0040"),
    ("ember", "Ember", "Orange", "#ff4000"),
    ("orange", "Orange", "Orange", "#ff7a00"),
    ("amber", "Amber", "Orange", "#ffb000"),
    ("yellow", "Yellow", "Yellow", "#ffe600"),
    ("lime", "Lime", "Green", "#a0ff00"),
    ("emerald", "Emerald", "Green", "#00ff60"),
    ("mint", "Mint", "Aqua", "#40ffb0"),
    ("teal", "Teal", "Aqua", "#00ffc8"),
    ("electric", "Electric", "Blue", "#00ffff"),
    ("cyan", "Cyan", "Blue", "#00e5ff"),
    ("ice", "Ice", "Blue", "#7df9ff"),
    ("azure", "Azure", "Blue", "#00a8ff"),
    ("sapphire", "Sapphire", "Blue", "#0050ff"),
    ("deep-blue", "Deep Blue", "Blue", "#0040ff"),
    ("royal", "Royal", "Blue", "#2962ff"),
    ("indigo", "Indigo", "Purple", "#3d5afe"),
    ("violet", "Violet", "Purple", "#8a2be2"),
    ("purple", "Purple", "Purple", "#b000ff"),
    ("lavender", "Lavender", "Purple", "#b08aff"),
    ("magenta", "Magenta", "Pink", "#ff00c8"),
    ("pink", "Pink", "Pink", "#ff4fa0"),
    ("rose", "Rose", "Pink", "#ff6f91"),
    ("white", "White", "Soft", "#ffffff"),
    ("warm-white", "Warm White", "Soft", "#ffd9a0"),
];

fn title(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn profile(id: String, name: String, family: &str, tags: Vec<String>, description: String, effect: Effect) -> Profile {
    Profile {
        id,
        name,
        category: format!("{family} · Animated"),
        tags,
        description,
        source: Source::Builtin,
        section: crate::section::COLOURS.to_string(),
        lighting: Lighting::Effect { effect },
    }
}

pub fn all() -> Vec<Profile> {
    let mut out = Vec::new();
    for &(mode, mname, speed, direction) in MODES {
        let mslug = mname.to_lowercase().replace(' ', "-");
        for &(cid, cname, family, hex) in COLORS {
            let color: Rgb = hex.parse().expect("valid preset colour");
            out.push(profile(
                format!("fx-{mslug}-{cid}"),
                format!("{cname} {mname}"),
                family,
                vec!["animated".into(), family.to_lowercase(), mslug.clone()],
                format!("{} effect in {}.", mode.info().name, cname.to_lowercase()),
                Effect { mode, speed, brightness: 4, direction, rainbow: false, color },
            ));
        }
        if mode.info().has_rainbow {
            out.push(profile(
                format!("fx-{mslug}-rainbow"),
                format!("Rainbow {mname}"),
                "Vivid",
                vec!["animated".into(), "rainbow".into(), mslug.clone()],
                format!("{} effect cycling through every colour.", mode.info().name),
                Effect { mode, speed, brightness: 4, direction, rainbow: true, color: Rgb(255, 0, 0) },
            ));
            // every other direction of the moving ones, plus a fast version
            for (d, dname) in mode.info().directions.iter().enumerate() {
                if d as u8 == direction {
                    continue;
                }
                out.push(profile(
                    format!("fx-{mslug}-rainbow-{}", dname.replace(' ', "-")),
                    format!("Rainbow {mname} · {}", title(dname)),
                    "Vivid",
                    vec!["animated".into(), "rainbow".into(), "dynamic".into(), mslug.clone()],
                    format!("{} effect in every colour, moving {dname}.", mode.info().name),
                    Effect { mode, speed, brightness: 4, direction: d as u8, rainbow: true, color: Rgb(255, 0, 0) },
                ));
            }
            if speed < 4 && !matches!(mode, Mode::Static | Mode::Reactive | Mode::ReactiveOff) {
                out.push(profile(
                    format!("fx-{mslug}-rainbow-turbo"),
                    format!("Rainbow {mname} · Turbo"),
                    "Vivid",
                    vec!["animated".into(), "rainbow".into(), "dynamic".into(), "fast".into(), mslug.clone()],
                    format!("{} effect in every colour at full speed.", mode.info().name),
                    Effect { mode, speed: 4, brightness: 4, direction, rainbow: true, color: Rgb(255, 0, 0) },
                ));
            }
        }
    }
    out.push(profile(
        "fx-spectrum".into(),
        "Spectrum Cycle".into(),
        "Vivid",
        vec!["animated".into(), "rainbow".into()],
        "The whole board fading through the spectrum.".into(),
        Effect { speed: 1, ..Effect::new(Mode::Spectrum) },
    ));
    out.push(profile(
        "fx-off".into(),
        "Lights Off".into(),
        "Soft",
        vec!["off".into()],
        "All lighting off.".into(),
        Effect { brightness: 0, ..Effect::new(Mode::Off) },
    ));
    out
}

/// Keypress "ghost" effects: the keys you hit light up and fade, or shoot a trail.
/// These run in the keyboard's firmware, the only place fast enough to react to typing.
/// Slower speeds leave longer ghosts.
pub fn ghosts() -> Vec<Profile> {
    let h = |s: &str| -> Rgb { s.parse().unwrap() };
    let royal = h("#1f45ff");
    // (id, name, description, mode, speed, rainbow, colour)
    type Ghost<'a> = (&'a str, &'a str, &'a str, Mode, u8, bool, Rgb);
    let list: &[Ghost] = &[
        ("ghost-royal-quick", "Royal Ghost · Quick", "Short, snappy royal-blue ghosts for fast typing and gaming.", Mode::Reactive, 3, false, royal),
        ("ghost-royal-laser", "Royal Laser Ghost", "Each keypress fires a royal-blue laser across its row.", Mode::Laser, 2, false, royal),
        ("ghost-royal-ripple", "Royal Ripple Ghost", "Each keypress sends a royal-blue ripple out across the board.", Mode::Ripple, 2, false, royal),
        (
            "ghost-royal-shadow",
            "Royal Shadow Ghost",
            "The board glows royal blue; keys you hit go dark and fade back in.",
            Mode::ReactiveOff,
            1,
            false,
            royal,
        ),
        ("ghost-gold", "Gold Ghost", "Golden ghosts on every keypress.", Mode::Reactive, 1, false, h("#ffb000")),
        ("ghost-rainbow", "Rainbow Ghost", "Every keypress leaves a fading ghost in a new colour.", Mode::Reactive, 1, true, h("#ff0000")),
        ("ghost-rainbow-laser", "Rainbow Laser Ghost", "Rainbow lasers fired from every key you hit.", Mode::Laser, 2, true, h("#ff0000")),
        ("ghost-rainbow-ripple", "Rainbow Ripple Ghost", "Rainbow ripples from every key you hit.", Mode::Ripple, 2, true, h("#ff0000")),
        ("ghost-ice", "Ice Ghost", "Ice-white ghosts that melt away.", Mode::Reactive, 1, false, h("#d8fbff")),
        ("ghost-fire", "Fire Ghost", "Keys flare orange-red and burn out.", Mode::Reactive, 1, false, h("#ff4000")),
        ("ghost-toxic", "Toxic Ghost", "Radioactive green ghosts.", Mode::Reactive, 1, false, h("#80ff00")),
        ("ghost-violet", "Violet Laser Ghost", "Violet lasers from every key you hit.", Mode::Laser, 2, false, h("#b000ff")),
        ("ghost-pink", "Pink Ripple Ghost", "Hot-pink ripples from every keypress.", Mode::Ripple, 2, false, h("#ff3fc8")),
        ("ghost-red", "Red Shadow Ghost", "A red board where the keys you hit go dark.", Mode::ReactiveOff, 1, false, h("#ff1020")),
    ];
    list.iter()
        .map(|&(id, name, desc, mode, speed, rainbow, color)| Profile {
            id: id.into(),
            name: name.into(),
            category: "Ghost".into(),
            tags: vec!["ghost".into(), "keypress".into(), "reactive".into(), "animated".into(), "dynamic".into()],
            description: desc.into(),
            source: Source::Builtin,
            section: crate::section::EFFECTS.to_string(),
            lighting: Lighting::Effect { effect: Effect { mode, speed, brightness: 4, direction: 0, rainbow, color } },
        })
        .collect()
}
