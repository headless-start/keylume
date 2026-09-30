//! Patterns map a key's physical position to a colour from a palette.

use keylume_proto::layout::Key;
use keylume_proto::{Layout, Rgb};

use crate::color::{dim, mix, ramp, seed, Rng};
use crate::cs::{default_binds, CsBinds, CsRole};
use crate::Palette;

/// Precomputed layout geometry shared by all patterns.
pub struct Ctx {
    pub w: f32,
    pub h: f32,
    /// centre of the spacebar
    pub space: (f32, f32),
    /// Counter-Strike binds (the player's own when known, else CS2's defaults).
    pub cs: CsBinds,
}

impl Ctx {
    pub fn new(l: &Layout) -> Self {
        Ctx::with_cs(l, default_binds())
    }

    pub fn with_cs(l: &Layout, cs: CsBinds) -> Self {
        let space = l.key("space").map(|k| (k.x + k.w / 2.0, k.y + 0.5)).unwrap_or((l.width / 2.0, l.height - 0.5));
        Ctx { w: l.width, h: l.height, space, cs }
    }
}

/// Centre of a key in key units.
fn centre(k: &Key) -> (f32, f32) {
    (k.x + k.w / 2.0, k.y + k.h / 2.0)
}

/// Smooth 2-D value noise in 0..1 (bilinear over hashed lattice points).
fn noise2(x: f32, y: f32, s: u64) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let at = |dx: i64, dy: i64| {
        let h = s ^ ((ix as i64 + dx) as u64).wrapping_mul(0x9E37_79B9) ^ ((iy as i64 + dy) as u64).wrapping_mul(0x85EB_CA6B_2F1D);
        Rng::new(h).next_f32()
    };
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * ux;
    let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * ux;
    top + (bottom - top) * uy
}

/// `n` deterministic random points spread over the board, seeded by the palette.
fn points(c: &Ctx, p: &Palette, salt: u64, n: usize) -> Vec<(f32, f32)> {
    let mut rng = Rng::new(seed(p.id) ^ salt);
    (0..n).map(|_| (rng.next_f32() * c.w, rng.next_f32() * c.h)).collect()
}

/// Smooth, seamless use of a palette for periodic values: 0 and 1 both land on the first stop.
fn cyc(p: &Palette, t: f32) -> Rgb {
    ramp(&p.stops, 0.5 - 0.5 * (t * std::f32::consts::TAU).cos())
}

fn polar(c: &Ctx, k: &Key, squash: f32) -> (f32, f32) {
    let (x, y) = centre(k);
    let (dx, dy) = (x - c.w / 2.0, (y - c.h / 2.0) * squash);
    (dy.atan2(dx), dx.hypot(dy))
}

const GAMER: &[&str] = &["w", "a", "s", "d", "up", "down", "left", "right"];
const GAMER_SUPPORT: &[&str] = &["q", "e", "r", "f", "lshift", "lctrl", "space", "tab", "1", "2", "3", "4"];
const MODIFIERS: &[&str] = &[
    "esc",
    "tab",
    "caps",
    "lshift",
    "lctrl",
    "lwin",
    "lalt",
    "ralt",
    "fn",
    "rctrl",
    "rshift",
    "enter",
    "backspace",
    "backslash",
    "grave",
    "delete",
    "pgup",
    "pgdn",
];

pub struct Pattern {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub tags: &'static [&'static str],
    f: fn(&Ctx, &Key, &Palette) -> Rgb,
}

impl Pattern {
    pub fn color(&self, ctx: &Ctx, k: &Key, p: &Palette) -> Rgb {
        (self.f)(ctx, k, p)
    }
}

pub fn all() -> Vec<Pattern> {
    vec![
        Pattern {
            id: "cascade",
            name: "Cascade",
            description: "a top-to-bottom gradient",
            tags: &["gradient"],
            f: |c, k, p| {
                let (_, y) = centre(k);
                ramp(&p.stops, (y - 0.5) / (c.h - 1.0))
            },
        },
        Pattern {
            id: "horizon",
            name: "Horizon",
            description: "a left-to-right gradient",
            tags: &["gradient"],
            f: |c, k, p| {
                let (x, _) = centre(k);
                ramp(&p.stops, x / c.w)
            },
        },
        Pattern {
            id: "slope",
            name: "Slope",
            description: "a diagonal gradient",
            tags: &["gradient"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                ramp(&p.stops, 0.7 * x / c.w + 0.3 * y / c.h)
            },
        },
        Pattern {
            id: "bloom",
            name: "Bloom",
            description: "a glow radiating from the centre",
            tags: &["radial"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let d = ((x - c.w / 2.0) / (c.w / 2.0)).hypot((y - c.h / 2.0) / (c.h / 1.2));
                ramp(&p.stops, d)
            },
        },
        Pattern {
            id: "flame",
            name: "Flame",
            description: "a flame rising from the spacebar",
            tags: &["radial"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let d = ((x - c.space.0) / 1.6).hypot((y - c.space.1) / 0.95);
                ramp(&p.stops, d / 5.2)
            },
        },
        Pattern {
            id: "ripple",
            name: "Ripple",
            description: "frozen ripples spreading from the centre",
            tags: &["radial", "rings"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let d = (x - c.w / 2.0).hypot((y - c.h / 2.0) * 1.6);
                ramp(&p.stops, 0.5 - 0.5 * (d * 0.95).cos())
            },
        },
        Pattern {
            id: "strata",
            name: "Strata",
            description: "bold contrasting bands, one colour per row",
            tags: &["bands"],
            f: |_, k, p| {
                // step through the palette out of order so neighbouring rows contrast
                // (a plain row gradient would just repeat Cascade)
                let n = p.stops.len();
                let step = n / 2 + 1;
                p.stops[(k.y.round() as usize * step) % n]
            },
        },
        Pattern {
            id: "stripes",
            name: "Racing Stripes",
            description: "bold diagonal stripes",
            tags: &["stripes"],
            f: |_, k, p| {
                let (x, y) = centre(k);
                let band = ((x + y * 1.2) / 2.0).floor() as i32;
                let n = p.stops.len().max(2);
                p.stops[(band.rem_euclid(2) as usize) * (n - 1)]
            },
        },
        Pattern {
            id: "gamer",
            name: "Gamer",
            description: "a dim base with WASD and arrows lit up",
            tags: &["gaming", "accent"],
            f: |_, k, p| {
                if GAMER.contains(&k.id.as_str()) {
                    p.accent
                } else if GAMER_SUPPORT.contains(&k.id.as_str()) {
                    p.stops[p.stops.len() / 2]
                } else if k.id == "esc" {
                    p.stops[0]
                } else {
                    mix(p.base, p.stops[p.stops.len() - 1], 0.3)
                } // dim, never off
            },
        },
        Pattern {
            id: "two-tone",
            name: "Two-Tone",
            description: "classic keycap two-tone: alphas and modifiers",
            tags: &["accent", "minimal"],
            f: |_, k, p| {
                if k.id == "esc" || k.id == "enter" {
                    p.accent
                } else if MODIFIERS.contains(&k.id.as_str()) {
                    p.stops[0]
                } else if k.id == "space" {
                    p.stops[p.stops.len() / 2]
                } else {
                    p.stops[p.stops.len() - 1]
                }
            },
        },
        Pattern {
            id: "nebula",
            name: "Nebula",
            description: "a starfield with glowing nebula clouds",
            tags: &["space", "random"],
            f: |_, k, p| {
                let mut rng = Rng::new(seed(p.id) ^ seed(&k.id));
                if rng.next_f32() < 0.13 {
                    return p.accent;
                }
                let (x, y) = centre(k);
                let n1 = (-(((x - 4.5) / 3.0).powi(2) + ((y - 1.5) / 1.4).powi(2))).exp();
                let n2 = (-(((x - 11.5) / 3.2).powi(2) + ((y - 3.0) / 1.5).powi(2))).exp();
                let mid = p.stops[p.stops.len() / 2];
                mix(mix(p.base, mid, n1 * 1.1), p.stops[0], n2 * 0.85)
            },
        },
        Pattern {
            id: "aurora",
            name: "Aurora",
            description: "a glowing ribbon across a dark sky",
            tags: &["wave"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let mid = c.h / 2.0 + 1.2 * (x * 0.55 + 0.6).sin();
                let band = (-((y - mid) / 1.25).powi(2)).exp();
                mix(p.base, ramp(&p.stops, x / c.w), (band * 1.15).min(1.0))
            },
        },
        Pattern {
            id: "split",
            name: "Split",
            description: "left and right halves in contrasting colours",
            tags: &["contrast"],
            f: |c, k, p| {
                let (x, _) = centre(k);
                if x < c.w / 2.0 {
                    p.stops[0]
                } else {
                    p.stops[p.stops.len() - 1]
                }
            },
        },
        Pattern {
            id: "checker",
            name: "Checker",
            description: "an alternating checkerboard",
            tags: &["retro"],
            f: |_, k, p| {
                let (x, y) = centre(k);
                if ((x.floor() as i32) + (y.floor() as i32)) % 2 == 0 {
                    p.stops[0]
                } else {
                    p.stops[p.stops.len() - 1]
                }
            },
        },
        Pattern {
            id: "sunburst",
            name: "Sunburst",
            description: "rays fanning out from the centre",
            tags: &["radial", "rays"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let a = ((y - c.h / 2.0) * 2.4).atan2(x - c.w / 2.0); // -pi..pi
                let rays = (0.5 + 0.5 * (a * 5.0).cos()).powi(2); // sharpened: distinct beams
                let d = ((x - c.w / 2.0) / (c.w / 2.0)).hypot((y - c.h / 2.0) / (c.h / 2.0)).min(1.0);
                // deep background, beams brighten toward the palette's lightest stop
                mix(p.stops[p.stops.len() - 1], p.stops[0], (rays * (1.0 - 0.45 * d)).max(1.0 - d * 2.2).clamp(0.0, 1.0))
            },
        },
        Pattern {
            id: "frame",
            name: "Frame",
            description: "a glowing outline around the edge",
            tags: &["accent", "outline"],
            f: |c, k, p| {
                let edge = k.y < 0.5 || k.y > c.h - 1.5 || k.x < 0.5 || k.x + k.w > c.w - 0.5;
                if edge {
                    p.stops[0]
                } else {
                    mix(p.base, p.stops[p.stops.len() - 1], 0.35)
                }
            },
        },
        Pattern {
            id: "home-row",
            name: "Home Row",
            description: "the typing home row lit, the rest dimmed",
            tags: &["accent", "typing"],
            f: |_, k, p| {
                if ["f", "j"].contains(&k.id.as_str()) {
                    p.accent
                } else if k.y.round() == 2.0 {
                    p.stops[0]
                } else if k.y.round() == 1.0 || k.y.round() == 3.0 {
                    mix(p.base, p.stops[p.stops.len() / 2], 0.45)
                } else {
                    p.base
                }
            },
        },
        Pattern {
            id: "diamond",
            name: "Diamond",
            description: "concentric diamonds from the centre",
            tags: &["radial", "geometric"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let d = (x - c.w / 2.0).abs() / 2.2 + (y - c.h / 2.0).abs();
                ramp(&p.stops, 0.5 - 0.5 * (d * 1.9).cos()) // smooth bands, no hard seams
            },
        },
        Pattern {
            id: "tidal",
            name: "Tidal Wave",
            description: "a sine wave rolling across the keys",
            tags: &["wave"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let crest = c.h / 2.0 + 1.6 * (x * 0.7).sin();
                let t = ((y - crest) / c.h + 0.5).clamp(0.0, 1.0);
                ramp(&p.stops, t)
            },
        },
        Pattern {
            id: "spotlight",
            name: "Spotlight",
            description: "a pool of light on the home row fading to shadow",
            tags: &["radial", "accent"],
            f: |_, k, p| {
                let (x, y) = centre(k);
                let d = ((x - 7.5) / 5.0).hypot((y - 2.5) / 1.6);
                mix(p.stops[0], dim(p.stops[p.stops.len() - 1], 0.55), d.min(1.0))
            },
        },
        // ---- Trippy ------------------------------------------------------------
        Pattern {
            id: "plasma",
            name: "Plasma",
            description: "a psychedelic plasma of interfering waves",
            tags: &["trippy", "wave"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let v = (x * 0.8).sin() + (y * 1.3 + 1.0).sin() + ((x + y) * 0.6).sin() + ((x - c.w / 2.0).hypot(y - c.h / 2.0) * 1.1).sin();
                cyc(p, v / 8.0 + 0.5)
            },
        },
        Pattern {
            id: "hypno-spiral",
            name: "Hypno Spiral",
            description: "a hypnotic spiral winding into the centre",
            tags: &["trippy", "radial"],
            f: |c, k, p| {
                let (a, r) = polar(c, k, 2.6);
                cyc(p, a / std::f32::consts::TAU + r * 0.13)
            },
        }, // one arm: legible on 68 keys
        Pattern {
            id: "galaxy-arms",
            name: "Galaxy Arms",
            description: "spiral arms of a galaxy around a bright core",
            tags: &["trippy", "space", "radial"],
            f: |c, k, p| {
                let (a, r) = polar(c, k, 2.0);
                let arm = (0.5 + 0.5 * (a * 2.0 - (r + 0.5).ln() * 3.0).cos()).powi(2);
                let core = (-(r / 1.3).powi(2)).exp();
                let glow = (arm * (1.0 - r / 10.0).max(0.2) + core).min(1.0);
                mix(p.base, if core > 0.5 { p.accent } else { ramp(&p.stops, r / 8.0) }, glow)
            },
        },
        Pattern {
            id: "kaleidoscope",
            name: "Kaleidoscope",
            description: "a mirror-symmetric kaleidoscope",
            tags: &["trippy", "symmetric"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let (u, v) = ((x - c.w / 2.0).abs(), (y - c.h / 2.0).abs() * 2.0);
                cyc(p, ((u * 0.9).sin() + (v * 1.1).cos() + ((u + v) * 0.7).sin()) / 6.0 + 0.5)
            },
        },
        Pattern {
            id: "moire",
            name: "Moiré",
            description: "two sets of rings clashing into an optical moiré",
            tags: &["trippy", "rings", "contrast"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let d1 = (x - c.w * 0.3).hypot((y - c.h / 2.0) * 1.3);
                let d2 = (x - c.w * 0.7).hypot((y - c.h / 2.0) * 1.3);
                if (d1 * 1.7).cos() * (d2 * 1.7).cos() > 0.0 {
                    p.stops[0]
                } else {
                    p.stops[p.stops.len() - 1]
                }
            },
        },
        Pattern {
            id: "ripple-tank",
            name: "Ripple Tank",
            description: "waves from two stones meeting in the middle",
            tags: &["trippy", "wave", "rings"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let d1 = (x - c.w * 0.25).hypot((y - c.h / 2.0) * 1.5);
                let d2 = (x - c.w * 0.75).hypot((y - c.h / 2.0) * 1.5);
                ramp(&p.stops, 0.5 + 0.25 * ((d1 * 1.8).sin() + (d2 * 1.8).sin()))
            },
        },
        Pattern {
            id: "tunnel",
            name: "Tunnel",
            description: "nested frames pulling you into a vanishing point",
            tags: &["trippy", "radial", "contrast"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                // rectangular distance: 0 at the centre, 1 at the board's edge
                let r = ((x - c.w / 2.0).abs() / (c.w / 2.0)).max((y - c.h / 2.0).abs() / (c.h / 2.0));
                let ring = ((1.0 - r.min(0.999)) * 5.0).floor() as usize; // five frames, counted from the edge
                let c0 = p.stops[ring % p.stops.len()];
                // lit and dark frames alternate; everything dims toward the vanishing point
                if ring.is_multiple_of(2) {
                    mix(dim(c0, 0.12), c0, 0.25 + 0.75 * r)
                } else {
                    mix(p.base, dim(c0, 0.1), 0.5)
                }
            },
        },
        Pattern {
            id: "prism",
            name: "Prism",
            description: "the palette fanned around the centre like light through a prism",
            tags: &["trippy", "radial", "gradient"],
            f: |c, k, p| {
                let (a, _) = polar(c, k, 2.4);
                cyc(p, a / std::f32::consts::TAU + 0.5)
            },
        },
        Pattern {
            id: "marble",
            name: "Marble",
            description: "swirling marbled stone",
            tags: &["trippy", "organic"],
            f: |_, k, p| {
                let (x, y) = centre(k);
                let warp = noise2(x * 0.35, y * 0.6, seed(p.id)) * 6.0;
                ramp(&p.stops, 0.5 + 0.5 * (x * 0.55 + y * 0.4 + warp).sin())
            },
        },
        Pattern {
            id: "stained-glass",
            name: "Stained Glass",
            description: "cells of coloured glass with darker leading",
            tags: &["trippy", "geometric", "random"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let mut d: Vec<(f32, usize)> =
                    points(c, p, 0x57A1, 9).iter().enumerate().map(|(i, &(px, py))| ((x - px).hypot((y - py) * 1.6), i)).collect();
                d.sort_by(|a, b| a.0.total_cmp(&b.0));
                let col = p.stops[d[0].1 % p.stops.len()];
                if d[1].0 - d[0].0 < 0.35 {
                    dim(col, 0.3)
                } else {
                    col
                }
            },
        },
        Pattern {
            id: "fractal",
            name: "Fractal",
            description: "the Mandelbrot set stretched across the keys",
            tags: &["trippy", "math"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let (cx, cy) = (-2.2 + x / c.w * 3.0, -1.15 + y / c.h * 2.3);
                let (mut zx, mut zy) = (0.0f32, 0.0f32);
                for n in 0..80 {
                    let (x2, y2) = (zx * zx, zy * zy);
                    if x2 + y2 > 256.0 {
                        let mu = n as f32 + 1.0 - ((x2 + y2).sqrt().ln()).log2();
                        return cyc(p, mu * 0.09);
                    }
                    zy = 2.0 * zx * zy + cy;
                    zx = x2 - y2 + cx;
                }
                p.base
            },
        },
        Pattern {
            id: "lava-lamp",
            name: "Lava Lamp",
            description: "molten blobs floating in the dark",
            tags: &["trippy", "organic"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let f: f32 = points(c, p, 0x1A7A, 5).iter().map(|&(bx, by)| 1.2 / ((x - bx).powi(2) + ((y - by) * 1.5).powi(2) + 0.4)).sum();
                let t = (f / 1.6).min(1.0);
                mix(p.base, ramp(&p.stops, 1.0 - t), (t * 1.4).min(1.0))
            },
        },
        Pattern {
            id: "chevron",
            name: "Chevron",
            description: "bold zig-zag chevrons",
            tags: &["trippy", "stripes", "retro"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let band = (x / 1.6 + (y - c.h / 2.0).abs() * 0.9).floor() as usize;
                p.stops[band % p.stops.len()]
            },
        },
        Pattern {
            id: "zebra",
            name: "Zebra Warp",
            description: "warped high-contrast stripes that seem to move",
            tags: &["trippy", "stripes", "contrast"],
            f: |_, k, p| {
                let (x, y) = centre(k);
                let v = (x * 1.4 + (y * 1.5).sin() * 2.2).sin();
                mix(p.stops[p.stops.len() - 1], p.stops[0], (0.5 + 0.5 * v * 2.0).clamp(0.0, 1.0))
            },
        },
        Pattern {
            id: "glitch",
            name: "Glitch",
            description: "a corrupted signal: shifted rows and stray pixels",
            tags: &["trippy", "random", "cyber"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let mut row = Rng::new(seed(p.id) ^ (y as u64 * 7919));
                let off = row.next_f32() * c.w;
                let mut px = Rng::new(seed(p.id) ^ seed(&k.id) ^ 0x61);
                if px.next_f32() < 0.12 {
                    return p.accent;
                }
                let col = p.stops[((x + off) / 2.3).floor() as usize % p.stops.len()];
                if row.next_f32() < 0.3 {
                    dim(col, 0.35)
                } else {
                    col
                }
            },
        },
        Pattern {
            id: "confetti",
            name: "Confetti",
            description: "every key a random pick from the palette",
            tags: &["trippy", "random", "party"],
            f: |_, k, p| {
                let mut rng = Rng::new(seed(p.id) ^ seed(&k.id) ^ 0xC0FE);
                if rng.next_f32() < 0.1 {
                    p.accent
                } else {
                    p.stops[(rng.next_f32() * p.stops.len() as f32) as usize % p.stops.len()]
                }
            },
        },
        Pattern {
            id: "heart",
            name: "Heart",
            description: "a glowing heart in the middle of the board",
            tags: &["shape", "love"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let dx = (x - c.w / 2.0).abs();
                // drawn row by row: two lobes with a notch, then tapering to a point
                let inside = match y.floor() as i32 {
                    0 => (0.6..3.3).contains(&dx),
                    1 => dx < 3.7,
                    2 => dx < 2.8,
                    3 => dx < 1.6,
                    _ => false,
                };
                if inside {
                    ramp(&p.stops, (y / c.h).clamp(0.0, 1.0) * 0.6)
                } else {
                    dim(p.stops[p.stops.len() - 1], 0.3)
                }
            },
        },
        // ---- Counter-Strike ----------------------------------------------------
        Pattern {
            id: "crosshair",
            name: "Crosshair",
            description: "a crisp CS-style crosshair centred on the board",
            tags: &["gaming", "cs", "accent"],
            f: |c, k, p| {
                let (x, y) = centre(k);
                let (dx, dy) = ((x - c.w / 2.0).abs(), (y - c.h / 2.0).abs());
                let arm_h = dy < 0.5 && (1.0..4.6).contains(&dx);
                let arm_v = dx < 0.6 && dy >= 0.9;
                if arm_h || arm_v {
                    p.stops[0]
                } else {
                    mix(p.base, p.stops[p.stops.len() - 1], (0.45 - 0.05 * dx).clamp(0.0, 0.45))
                }
            },
        },
        Pattern {
            id: "loadout",
            name: "CS Loadout",
            description: "your Counter-Strike binds lit by role: movement, weapons, grenades, utility",
            tags: &["gaming", "cs", "accent"],
            f: |c, k, p| {
                let n = p.stops.len();
                match c.cs.get(&k.id) {
                    Some(CsRole::Movement) => p.accent,
                    Some(CsRole::Weapon) => p.stops[0],
                    Some(CsRole::Grenade) => p.stops[1.min(n - 1)],
                    Some(CsRole::Utility | CsRole::JumpThrow) => p.stops[n / 2],
                    Some(CsRole::Body) => p.stops[n - 1],
                    Some(CsRole::Comms | CsRole::Menu) => mix(p.base, p.stops[n / 2], 0.4),
                    _ => p.base,
                }
            },
        },
    ]
}
