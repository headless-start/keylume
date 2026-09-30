//! Flags of the world on the keys, read from the JSON files in `flags/` (one per
//! region). A flag is stripes with simple shapes on top (a canton, a cross, a disc, a
//! hoist triangle, …), drawn by key position, so it reads well even on a 15 × 5 board.

use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use keylume_live::LiveEffect;
use keylume_proto::{Layout, Rgb};
use serde::Deserialize;

use crate::{chroma_i32, pick_unused, sum_i32, Lighting, Profile, Source};

/// The regions, in the order the Library lists them.
const FILES: &[(&str, &str)] = &[
    ("europe.json", include_str!("../flags/europe.json")),
    ("americas.json", include_str!("../flags/americas.json")),
    ("asia.json", include_str!("../flags/asia.json")),
    ("africa.json", include_str!("../flags/africa.json")),
    ("oceania.json", include_str!("../flags/oceania.json")),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// The collection's name in the Library, e.g. "Europe".
    pub collection: String,
    pub flags: Vec<Flag>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flag {
    /// `flag-` + the country's name in kebab-case, e.g. `flag-france`.
    pub id: String,
    /// The country's common English name.
    pub name: String,
    pub stripes: Stripes,
    /// Drawn in order on top of the stripes.
    #[serde(default)]
    pub shapes: Vec<Shape>,
}

/// Stripes across the whole flag: `h` runs top to bottom, `v` left to right. Equal
/// widths unless `weights` are given (one per colour).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stripes {
    pub dir: Dir,
    pub colors: Vec<Rgb>,
    #[serde(default)]
    pub weights: Vec<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    H,
    V,
}

/// Positions are fractions of the board (x left to right, y top to bottom); sizes marked
/// "keys" are in key widths.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Shape {
    /// The top-left rectangle, `w` × `h` of the board.
    Canton { color: Rgb, w: f32, h: f32 },
    /// A cross centred on (x, y) with arms `thick` keys wide (Nordic crosses sit left of centre).
    Cross { color: Rgb, x: f32, y: f32, thick: f32 },
    /// A diagonal cross from corner to corner, `thick` keys wide.
    Saltire { color: Rgb, thick: f32 },
    /// A disc of radius `r` keys at (x, y); small ones make stars and emblems.
    Disc { color: Rgb, x: f32, y: f32, r: f32 },
    /// A triangle on the left edge (the right edge when `right`), its tip `depth` of the
    /// way across.
    Triangle {
        color: Rgb,
        depth: f32,
        #[serde(default)]
        right: bool,
    },
    /// A straight band across the whole flag from `from` to `to` (fractions): vertical
    /// when `dir` is `v`, horizontal when `h` (the Central African Republic's red stripe).
    Band { color: Rgb, dir: Dir, from: f32, to: f32 },
    /// A band from the bottom-left to the top-right corner (top-left to bottom-right
    /// when `down`), `thick` keys wide.
    Diagonal {
        color: Rgb,
        thick: f32,
        #[serde(default)]
        down: bool,
    },
    /// Everything on one side of the corner-to-corner diagonal (bottom-left to top-right,
    /// or top-left to bottom-right when `down`): below it when `below`, else above it.
    /// Diagonally split flags such as Papua New Guinea's.
    Half {
        color: Rgb,
        #[serde(default)]
        down: bool,
        #[serde(default)]
        below: bool,
    },
    /// Shapes drawn inside a `w` × `h` part of the board whose top-left corner is at
    /// (x, y) (the top-left canton by default), positioned within it as if it were the
    /// whole flag (Greece's cross, a union flag in a canton, Zambia's bands at the fly).
    Area {
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        w: f32,
        h: f32,
        shapes: Vec<Shape>,
    },
}

impl Shape {
    fn color(&self) -> Option<Rgb> {
        match *self {
            Shape::Canton { color, .. }
            | Shape::Cross { color, .. }
            | Shape::Saltire { color, .. }
            | Shape::Disc { color, .. }
            | Shape::Triangle { color, .. }
            | Shape::Diagonal { color, .. }
            | Shape::Half { color, .. }
            | Shape::Band { color, .. } => Some(color),
            Shape::Area { .. } => None,
        }
    }
}

/// Paint `shapes` in order over colour `c` at (cx, cy) keys, on a flag `w` × `h` keys.
fn paint(shapes: &[Shape], cx: f32, cy: f32, w: f32, h: f32, mut c: Rgb) -> Rgb {
    let (u, v) = (cx / w, cy / h);
    for s in shapes {
        let hit = match *s {
            Shape::Canton { w: cw, h: ch, .. } => u < cw && v < ch,
            Shape::Cross { x, y, thick, .. } => (cx - x * w).abs() < thick / 2.0 || (cy - y * h).abs() < thick / 2.0,
            Shape::Saltire { thick, .. } => to_line(cx, cy, (0.0, 0.0), (w, h)).min(to_line(cx, cy, (0.0, h), (w, 0.0))) < thick / 2.0,
            Shape::Disc { x, y, r, .. } => (cx - x * w).hypot(cy - y * h) < r,
            Shape::Triangle { depth, right, .. } => (if right { 1.0 - u } else { u }) < depth * (1.0 - (2.0 * v - 1.0).abs()),
            Shape::Band { dir, from, to, .. } => (from..to).contains(&if dir == Dir::V { u } else { v }),
            Shape::Diagonal { thick, down, .. } => {
                let (a, b) = if down { ((0.0, 0.0), (w, h)) } else { ((0.0, h), (w, 0.0)) };
                to_line(cx, cy, a, b) < thick / 2.0
            }
            Shape::Half { down, below, .. } => {
                let line = if down { u * h } else { h - u * h };
                if below {
                    cy > line
                } else {
                    cy < line
                }
            }
            Shape::Area { x, y, w: aw, h: ah, ref shapes } => {
                if (x..x + aw).contains(&u) && (y..y + ah).contains(&v) {
                    c = paint(shapes, cx - x * w, cy - y * h, aw * w, ah * h, c);
                }
                false
            }
        };
        if hit {
            c = s.color().unwrap_or(c);
        }
    }
    c
}

/// The registered regions, parsed once.
pub fn regions() -> &'static [Region] {
    static ALL: OnceLock<Vec<Region>> = OnceLock::new();
    ALL.get_or_init(|| FILES.iter().map(|(file, json)| serde_json::from_str(json).unwrap_or_else(|e| panic!("flags/{file}: {e}"))).collect())
}

/// Distance from (px, py) to the line through (ax, ay) and (bx, by).
fn to_line(px: f32, py: f32, (ax, ay): (f32, f32), (bx, by): (f32, f32)) -> f32 {
    ((by - ay) * px - (bx - ax) * py + bx * ay - by * ax).abs() / (bx - ax).hypot(by - ay)
}

impl Flag {
    /// The flag's colour at the centre of every key.
    pub fn render(&self, layout: &Layout) -> BTreeMap<String, Rgb> {
        let (w, h) = (layout.width, layout.height);
        let weights: Vec<f32> =
            if self.stripes.weights.len() == self.stripes.colors.len() { self.stripes.weights.clone() } else { vec![1.0; self.stripes.colors.len()] };
        let total: f32 = weights.iter().sum();
        layout
            .keys
            .iter()
            .map(|k| {
                let (cx, cy) = (k.x + k.w / 2.0, k.y + k.h / 2.0);
                let (u, v) = (cx / w, cy / h);
                let t = if self.stripes.dir == Dir::H { v } else { u };
                let mut acc = 0.0;
                let mut c = *self.stripes.colors.last().unwrap_or(&Rgb::BLACK);
                for (col, wt) in self.stripes.colors.iter().zip(&weights) {
                    acc += wt / total;
                    if t < acc {
                        c = *col;
                        break;
                    }
                }
                (k.id.clone(), paint(&self.shapes, cx, cy, w, h, c))
            })
            .collect()
    }

    /// Every colour in the flag, in order, without repeats.
    pub fn colours(&self) -> Vec<Rgb> {
        fn walk(shapes: &[Shape], out: &mut Vec<Rgb>) {
            for s in shapes {
                match s {
                    Shape::Area { shapes, .. } => walk(shapes, out),
                    s => out.extend(s.color()),
                }
            }
        }
        let mut shapes = Vec::new();
        walk(&self.shapes, &mut shapes);
        let mut out: Vec<Rgb> = Vec::new();
        for c in self.stripes.colors.iter().copied().chain(shapes) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }

    /// The colour the flag is known by: the most colourful one, brightest first, so a
    /// white or black field never stands in for it.
    pub fn signature(&self) -> Rgb {
        let chroma = |c: &Rgb| c.0.max(c.1).max(c.2) as i32 - c.0.min(c.1).min(c.2) as i32;
        let sum = |c: &Rgb| c.0 as i32 + c.1 as i32 + c.2 as i32;
        self.colours().into_iter().max_by_key(|c| (chroma(c), sum(c))).unwrap_or(Rgb(255, 255, 255))
    }
}

/// Everything wrong with a region file (empty when it's good). `taken` holds names and
/// ids already used elsewhere in the library, lower-cased.
pub fn check(json: &str, taken: &HashSet<String>) -> Result<Region, Vec<String>> {
    let r: Region = serde_json::from_str(json).map_err(|e| vec![format!("not a valid flags file: {e}")])?;
    let mut bad = Vec::new();
    if r.collection.trim().is_empty() || r.collection.len() > 20 {
        bad.push(format!("collection name {:?} should be 1-20 characters", r.collection));
    }
    let kebab = |s: &str| s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !s.ends_with('-');
    let (mut ids, mut names) = (HashSet::new(), HashSet::new());
    for f in &r.flags {
        let who = format!("{} ({})", f.id, f.name);
        if !f.id.starts_with("flag-") || !kebab(&f.id) || f.id.len() > 40 {
            bad.push(format!("{who}: id must be \"flag-\" + kebab-case, at most 40 characters"));
        }
        if !ids.insert(f.id.clone()) || taken.contains(&f.id) {
            bad.push(format!("{who}: the id is already used"));
        }
        if !(2..=40).contains(&f.name.chars().count()) || f.name.contains('·') {
            bad.push(format!("{who}: the name must be 2-40 characters, without '·'"));
        }
        if !names.insert(f.name.to_lowercase()) || taken.contains(&f.name.to_lowercase()) {
            bad.push(format!("{who}: the name is already used"));
        }
        let s = &f.stripes;
        if !(1..=13).contains(&s.colors.len()) || !(s.weights.is_empty() || s.weights.len() == s.colors.len()) || s.weights.iter().any(|w| *w <= 0.0)
        {
            bad.push(format!("{who}: 1-13 stripe colours, and either no weights or one positive weight per colour"));
        }
        fn in_range(sh: &Shape) -> bool {
            let frac = |f: f32| (0.0..=1.0).contains(&f);
            match sh {
                Shape::Canton { w, h, .. } => frac(*w) && frac(*h),
                Shape::Cross { x, y, thick, .. } => frac(*x) && frac(*y) && (0.5..=4.0).contains(thick),
                Shape::Saltire { thick, .. } | Shape::Diagonal { thick, .. } => (0.5..=4.0).contains(thick),
                Shape::Disc { x, y, r, .. } => frac(*x) && frac(*y) && (0.3..=3.0).contains(r),
                Shape::Triangle { depth, .. } => frac(*depth),
                Shape::Band { from, to, .. } => frac(*from) && frac(*to) && from < to,
                Shape::Half { .. } => true,
                Shape::Area { x, y, w, h, shapes } => frac(*x) && frac(*y) && frac(x + w) && frac(y + h) && shapes.iter().all(in_range),
            }
        }
        for sh in &f.shapes {
            if !in_range(sh) {
                bad.push(format!("{who}: a shape is out of range ({sh:?}); positions are 0-1, thicknesses 0.5-4 keys, radii 0.3-3 keys"));
            }
        }
        if f.colours().len() < 2 {
            bad.push(format!("{who}: a flag needs at least two colours"));
        }
    }
    if bad.is_empty() {
        Ok(r)
    } else {
        Err(bad)
    }
}

/// Every flag profile, region by region: the flag on the keys, and its colours flowing live.
pub(crate) fn profiles(layout: &Layout, seen: &mut HashSet<String>) -> Vec<Profile> {
    let mut out = Vec::new();
    for r in regions() {
        let region = r.collection.to_lowercase();
        let mk = |id: String, name: String, description: String, lighting| Profile {
            id,
            name,
            category: r.collection.clone(),
            tags: vec!["flag".into(), region.clone()],
            description,
            source: Source::Builtin,
            section: crate::section::FLAGS.to_string(),
            lighting,
        };
        for f in &r.flags {
            out.push(mk(
                f.id.clone(),
                f.name.clone(),
                format!("The flag of {} on your keys.", f.name),
                Lighting::PerKey { keys: f.render(layout), brightness: keylume_proto::led::MAX_BRIGHTNESS },
            ));
        }
        // one keyboard animation each, in the flag's boldest colour
        for (i, f) in r.flags.iter().enumerate() {
            const MODES: [keylume_proto::Mode; 4] =
                [keylume_proto::Mode::Wave, keylume_proto::Mode::Breathing, keylume_proto::Mode::LineWave, keylume_proto::Mode::Ripple];
            let sig = f.signature();
            let mut alts: Vec<Rgb> = f.colours().into_iter().filter(|c| *c != sig).collect();
            alts.sort_by_key(|c| std::cmp::Reverse((chroma_i32(c), sum_i32(c))));
            let colors: Vec<Rgb> = std::iter::once(sig).chain(alts).collect();
            // The 4 curated modes first (unchanged for the overwhelming majority of
            // flags), then the rest of `THEME_MODES` as extra headroom: a few very simple
            // flags (just two official colours, shared with others) need it.
            let mode_order: Vec<keylume_proto::Mode> =
                MODES.iter().copied().chain(crate::themes::THEME_MODES.iter().copied().filter(|m| !MODES.contains(m))).collect();
            let all: Vec<(Rgb, keylume_proto::Mode)> = colors.iter().flat_map(|&c| mode_order.iter().map(move |&m| (c, m))).collect();
            let light = |(color, mode): (Rgb, keylume_proto::Mode)| Lighting::Effect {
                effect: keylume_proto::Effect { mode, speed: 2, brightness: 4, direction: 0, rainbow: false, color },
            };
            // A clash with another flag's animation takes the next mode round the list;
            // several flags share official colours, so once every mode is taken for one
            // exact colour, the flag's next most colourful colour takes over as well.
            let pick = pick_unused((sig, MODES[i % MODES.len()]), &all, &mut Vec::new(), seen, light);
            let name = pick.1.info().name;
            out.push(mk(
                format!("{}-animated", f.id),
                format!("{} · {name} Animation", f.name),
                format!("{}'s flag colour, as the keyboard's {} animation.", f.name, name.to_lowercase()),
                light(pick),
            ));
        }
        for f in &r.flags {
            // Some flags share the same colours exactly (e.g. two red-and-white ones); a
            // clash takes the next cycle length instead, so the flow is never identical.
            const PERIODS: [u32; 6] = [12, 9, 15, 11, 14, 10];
            let colors = f.colours();
            let flow = |period: u32| Lighting::Live { live: LiveEffect::PaletteFlow { colors: colors.clone(), period: period as f32 } };
            let period = pick_unused(PERIODS[0], &PERIODS, &mut Vec::new(), seen, flow);
            out.push(mk(
                format!("{}-live", f.id),
                format!("{} · Live Colours", f.name),
                format!("The colours of {}'s flag, flowing live.", f.name),
                flow(period),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(json: &str) -> Flag {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn stripes_and_shapes_land_on_the_right_keys() {
        let l = Layout::tk68();
        let tricolour =
            flag(r##"{ "id": "flag-x", "name": "X", "stripes": { "dir": "v", "colors": ["#0055a4", "#ffffff", "#ef4135"] } }"##).render(&l);
        // the keys carry the flag's exact official colours (LED correction happens once,
        // downstream, for every colour the app sends — see `keylume_core::color`)
        let c = |hex: &str| hex.parse::<Rgb>().unwrap();
        assert_eq!((tricolour["esc"], tricolour["h"], tricolour["pgup"]), (c("#0055a4"), c("#ffffff"), c("#ef4135")));
        let nordic = flag(
            r##"{ "id": "flag-y", "name": "Y", "stripes": { "dir": "h", "colors": ["#c8102e"] },
                 "shapes": [ { "kind": "cross", "color": "#ffffff", "x": 0.36, "y": 0.5, "thick": 1 } ] }"##,
        )
        .render(&l);
        assert_eq!(nordic["a"], c("#ffffff"), "the middle row is the cross");
        assert_eq!(nordic["q"], c("#c8102e"));
        let disc = flag(
            r##"{ "id": "flag-z", "name": "Z", "stripes": { "dir": "h", "colors": ["#ffffff"] },
                 "shapes": [ { "kind": "disc", "color": "#bc002d", "x": 0.5, "y": 0.5, "r": 1.2 } ] }"##,
        )
        .render(&l);
        assert_eq!(disc["h"], c("#bc002d"));
        assert_eq!(disc["esc"], c("#ffffff"));
        // a cross only inside the canton, centred on it
        let greek = flag(
            r##"{ "id": "flag-g", "name": "G", "stripes": { "dir": "h", "colors": ["#0d5eaf", "#ffffff"] },
                 "shapes": [ { "kind": "canton", "color": "#0d5eaf", "w": 0.4, "h": 0.6 },
                             { "kind": "area", "w": 0.4, "h": 0.6, "shapes": [ { "kind": "cross", "color": "#ffffff", "x": 0.5, "y": 0.5, "thick": 1 } ] } ] }"##,
        )
        .render(&l);
        // a diagonal split: black below the top-left to bottom-right diagonal
        let split = flag(
            r##"{ "id": "flag-s", "name": "S", "stripes": { "dir": "h", "colors": ["#ce1126"] },
                 "shapes": [ { "kind": "half", "color": "#000000", "down": true, "below": true } ] }"##,
        )
        .render(&l);
        assert_eq!((split["lctrl"], split["backspace"]), (c("#000000"), c("#ce1126")));
        // a band across the stripes, a triangle on the right, an area at the fly
        let more = flag(
            r##"{ "id": "flag-m", "name": "M", "stripes": { "dir": "h", "colors": ["#003082"] },
                 "shapes": [ { "kind": "band", "color": "#d21034", "dir": "v", "from": 0.45, "to": 0.55 },
                             { "kind": "triangle", "color": "#000000", "depth": 0.2, "right": true },
                             { "kind": "area", "x": 0.0, "y": 0.8, "w": 0.3, "h": 0.2,
                               "shapes": [ { "kind": "band", "color": "#ffce00", "dir": "v", "from": 0.0, "to": 1.0 } ] } ] }"##,
        )
        .render(&l);
        assert_eq!(more["u"], c("#d21034"), "the band crosses the middle");
        assert_eq!(more["pgup"], c("#000000"), "the triangle sits on the right edge");
        assert_eq!(more["lctrl"], c("#ffce00"), "the area sits bottom-left");
        assert_eq!(more["esc"], c("#003082"));
        assert_eq!(greek["q"], c("#ffffff"), "the canton's middle row is the cross");
        assert_eq!(greek["o"], c("#0d5eaf"), "outside the canton the stripes carry on");
    }

    #[test]
    fn official_colours_are_kept() {
        // flags carry their exact official colours; the LED correction happens once,
        // downstream in keylume-core, for every colour the app sends.
        for plain in [Rgb(255, 255, 255), Rgb(0, 0, 0), Rgb(0x80, 0x80, 0x80), Rgb(0xde, 0x29, 0x10), Rgb(0xff, 0xde, 0x00)] {
            let f = flag(&format!(r##"{{ "id": "flag-t", "name": "T", "stripes": {{ "dir": "h", "colors": ["{plain}"] }} }}"##));
            assert_eq!(f.colours(), vec![plain], "{plain} should pass through unchanged");
        }
        let india: Flag = serde_json::from_str(
            r##"{ "id": "flag-india", "name": "India", "stripes": { "dir": "h", "colors": ["#ff9933", "#ffffff", "#138808"] } }"##,
        )
        .unwrap();
        assert_eq!(india.colours()[0], Rgb(0xff, 0x99, 0x33), "saffron is exactly the official colour");
    }

    #[test]
    fn a_flags_signature_colour_is_never_its_white() {
        let india: Flag = serde_json::from_str(
            r##"{ "id": "flag-india", "name": "India", "stripes": { "dir": "h", "colors": ["#ff9933", "#ffffff", "#138808"] } }"##,
        )
        .unwrap();
        let sig = india.signature();
        assert!(sig.0 > sig.2 && sig != Rgb(255, 255, 255), "{sig}");
    }

    #[test]
    fn every_region_file_passes_the_checker() {
        let lib = crate::builtin(&Layout::tk68());
        for (file, json) in FILES {
            let r: Region = serde_json::from_str(json).unwrap();
            let taken: HashSet<String> = lib
                .iter()
                .filter(|p| p.category != r.collection)
                .flat_map(|p| [p.id.to_lowercase(), p.name.split(" · ").next().unwrap_or(&p.name).to_lowercase()])
                .collect();
            if let Err(bad) = check(json, &taken) {
                panic!("flags/{file}:\n{}", bad.join("\n"));
            }
        }
        let flags: usize = regions().iter().map(|r| r.flags.len()).sum();
        assert!(regions().len() == 5 && flags >= 190, "{} regions, {flags} flags", regions().len());
        assert!(lib.iter().any(|p| p.id == "flag-france" && p.section == crate::section::FLAGS));
    }

    #[test]
    fn the_checker_explains_what_is_wrong() {
        let bad = check(
            r##"{ "collection": "Europe", "flags": [ { "id": "france", "name": "France", "stripes": { "dir": "v", "colors": ["#0055a4"] } } ] }"##,
            &HashSet::new(),
        )
        .unwrap_err();
        assert!(bad.iter().any(|b| b.contains("\"flag-\"")), "{bad:?}");
        assert!(bad.iter().any(|b| b.contains("two colours")), "{bad:?}");
    }
}
