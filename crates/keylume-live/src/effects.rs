//! Keyboard animations drawn by Keylume, for boards that don't run them themselves (HID
//! LampArray keyboards, where Keylume drives every light). The maths is the app's preview
//! (`src/lib/animate.ts`, `effectFrame`) without made-up key presses: the modes that
//! answer typing aren't offered on those boards.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use keylume_proto::{Effect, Layout, Mode, Rgb};

/// A key's centre in 0..1 of the board, and its place in row-by-row order.
struct K {
    id: String,
    x: f32,
    y: f32,
    order: usize,
}

/// Plays one animation on one layout.
pub struct EffectPlayer {
    effect: Effect,
    keys: Vec<K>,
}

fn hash01(n: f64) -> f32 {
    let x = (n * 127.1 + 311.7).sin() * 43758.5453;
    (x - x.floor()) as f32
}

fn lin(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb(v: f32) -> u8 {
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// `c` at `k` of its light (mixed with black in linear light, as the previews do).
fn dim(c: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    Rgb(srgb(lin(c.0) * k), srgb(lin(c.1) * k), srgb(lin(c.2) * k))
}

/// A fully saturated colour at hue `h` degrees.
fn hue(h: f32) -> Rgb {
    let h = h.rem_euclid(360.0);
    let k = |n: f32| (n + h / 30.0) % 12.0;
    let a = 0.5;
    let f = |n: f32| 0.5 - a * (k(n) - 3.0).min(9.0 - k(n)).clamp(-1.0, 1.0);
    let c = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(c(f(0.0)), c(f(8.0)), c(f(4.0)))
}

impl EffectPlayer {
    pub fn new(effect: Effect, layout: &Layout) -> EffectPlayer {
        let mut keys: Vec<_> = layout.keys.iter().collect();
        keys.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        let (w, h) = (layout.width.max(1.0), layout.height.max(1.0));
        let keys = keys
            .into_iter()
            .enumerate()
            .map(|(order, k)| K { id: k.id.clone(), x: (k.x + k.w / 2.0) / w, y: (k.y + k.h / 2.0) / h, order })
            .collect();
        EffectPlayer { effect, keys }
    }

    pub fn effect(&self) -> &Effect {
        &self.effect
    }

    /// Every key's colour `t` seconds in.
    pub fn frame(&self, t: f32) -> BTreeMap<String, Rgb> {
        let e = &self.effect;
        let s = 0.35 + e.speed as f32 * 0.35; // speed 0..4 -> 0.35..1.75
        let color_at = |k: &K| if e.rainbow { hue(k.x * 300.0 + t * 60.0 * s) } else { e.color };
        let each = |f: &dyn Fn(&K) -> Rgb| self.keys.iter().map(|k| (k.id.clone(), f(k))).collect();
        match e.mode {
            Mode::Off => each(&|_| Rgb::BLACK),
            Mode::Static | Mode::UserPicture | Mode::ScreenSync => each(&|k| color_at(k)),
            Mode::Breathing => {
                let b = 0.08 + 0.92 * (0.5 - 0.5 * (t * s * TAU / 3.0).cos());
                let breath = (t * s / 3.0).floor();
                let c = if e.rainbow { hue(breath * 60.0) } else { e.color };
                each(&|_| dim(c, b))
            }
            Mode::Spectrum => {
                let c = hue(t * 50.0 * s);
                each(&|_| c)
            }
            Mode::Wave => {
                let vertical = e.direction >= 2;
                let sign = if e.direction % 2 == 1 { 1.0 } else { -1.0 };
                each(&|k| {
                    let along = if vertical { k.y } else { k.x };
                    if e.rainbow {
                        hue(along * 360.0 + sign * t * 120.0 * s)
                    } else {
                        dim(e.color, 0.2 + 0.8 * (0.5 + 0.5 * (TAU * (along * 1.2 + sign * t * s * 0.6)).cos()))
                    }
                })
            }
            Mode::LineWave => {
                let sign = if e.direction == 1 { -1.0 } else { 1.0 };
                each(&|k| {
                    let p = (k.x - sign * t * s * 0.5).rem_euclid(1.0);
                    dim(color_at(k), 0.08 + 0.92 * (-((p - 0.5) * 7.0).powi(2)).exp())
                })
            }
            Mode::SineWave => each(&|k| {
                let crest = 0.5 + 0.32 * (TAU * (k.x * 1.3 - t * s * 0.5)).sin();
                dim(color_at(k), 0.08 + 0.92 * (-((k.y - crest) * 5.0).powi(2)).exp())
            }),
            Mode::Kaleidoscope | Mode::Converge => {
                let dir = if e.mode == Mode::Converge || e.direction == 1 { 1.0 } else { -1.0 };
                each(&|k| {
                    let d = ((k.x - 0.5) * 3.2).hypot(k.y - 0.5);
                    if e.rainbow {
                        hue(d * 400.0 + dir * t * 150.0 * s)
                    } else {
                        dim(e.color, 0.1 + 0.9 * (0.5 + 0.5 * (d * 12.0 + dir * t * s * 4.0).cos()))
                    }
                })
            }
            Mode::CircleWave => {
                let dir = if e.direction == 1 { 1.0 } else { -1.0 };
                each(&|k| {
                    let a = ((k.y - 0.5) * 0.4).atan2(k.x - 0.5);
                    if e.rainbow {
                        hue(a / TAU * 360.0 + dir * t * 120.0 * s)
                    } else {
                        dim(e.color, 0.1 + 0.9 * (0.5 + 0.5 * (a - dir * t * s * 2.5).cos()))
                    }
                })
            }
            // starlight: every key twinkles on its own clock
            Mode::Raindrop => each(&|k| {
                let ph = ((t * s * 0.6 + hash01(k.order as f64) * 7.0) % 3.0) / 3.0;
                let c = if e.rainbow { hue(hash01(k.order as f64 + (t * s * 0.2).floor() as f64) * 360.0) } else { e.color };
                dim(c, if ph < 0.25 { (ph / 0.25 * std::f32::consts::PI).sin() } else { 0.04 })
            }),
            Mode::RainDown | Mode::Meteor => {
                let tail = if e.mode == Mode::Meteor { 0.45 } else { 0.18 };
                each(&|k| {
                    let col = (k.x * 16.0).round();
                    let head = ((t * s * 0.7 + hash01(col as f64) * 3.0) % 1.4) - 0.2;
                    let behind = head - k.y;
                    dim(color_at(k), if (0.0..tail).contains(&behind) { 1.0 - behind / tail } else { 0.04 })
                })
            }
            Mode::Snake => {
                let n = self.keys.len().max(1) as f32;
                let head = (t * s * 14.0) % n;
                each(&|k| {
                    let behind = (head - k.order as f32 + n) % n; // row-by-row order approximates the path
                    dim(color_at(k), if behind < 10.0 { 1.0 - behind / 10.0 } else { 0.04 })
                })
            }
            Mode::Dazzle => {
                let tick = (t * s * 3.0).floor() as f64;
                each(&|k| {
                    let r = hash01(k.order as f64 * 7.0 + tick);
                    if e.rainbow {
                        hue(r * 360.0)
                    } else {
                        dim(e.color, 0.2 + 0.8 * r)
                    }
                })
            }
            // they answer typing, which Keylume doesn't watch: at rest they sit dark (lit
            // for reactive-off), as the keyboard itself does until you type
            Mode::Ripple => each(&|k| dim(color_at(k), 0.05)),
            Mode::Reactive | Mode::Laser => each(&|k| dim(color_at(k), 0.04)),
            Mode::ReactiveOff => each(&|k| color_at(k)),
            Mode::MusicBars | Mode::MusicPulse => {
                let levels: Vec<f32> = (0..32).map(|i| (6.0 * (0.5 + 0.5 * (i as f32 * 0.45 - t * 4.0).sin())).round()).collect();
                each(&|k| {
                    let band = ((k.x * 32.0) as usize).min(31);
                    let c = if e.rainbow { hue(k.x * 300.0) } else { e.color };
                    if (1.0 - k.y) * 6.0 < levels[band] + 0.4 {
                        c
                    } else {
                        dim(c, 0.04)
                    }
                })
            }
        }
    }
}

/// Music bars (32 bands, levels 0..6) on the keys, rising from the bottom row: how a
/// host-driven board shows the music effects.
pub fn bars(layout: &Layout, levels: &[u8], color: Rgb, rainbow: bool) -> BTreeMap<String, Rgb> {
    let (w, h) = (layout.width.max(1.0), layout.height.max(1.0));
    layout
        .keys
        .iter()
        .map(|k| {
            let (x, y) = ((k.x + k.w / 2.0) / w, (k.y + k.h / 2.0) / h);
            let band = ((x * levels.len() as f32) as usize).min(levels.len().saturating_sub(1));
            let lit = (1.0 - y) * 6.0 < levels.get(band).copied().unwrap_or(0) as f32 + 0.4;
            let c = if rainbow { hue(x * 300.0) } else { color };
            (k.id.clone(), if lit { c } else { dim(c, 0.04) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(mode: Mode) -> Effect {
        Effect { mode, speed: 2, brightness: 4, direction: 0, rainbow: false, color: Rgb(0, 0x60, 0xFF) }
    }

    #[test]
    fn animations_move_and_stills_hold() {
        let l = Layout::tk68();
        let still = [Mode::Off, Mode::Static, Mode::Ripple, Mode::Reactive, Mode::ReactiveOff, Mode::Laser, Mode::UserPicture, Mode::ScreenSync];
        for m in Mode::ALL {
            let p = EffectPlayer::new(effect(m), &l);
            let frames: std::collections::HashSet<String> = [0.0, 0.7, 1.9, 3.1].iter().map(|t| format!("{:?}", p.frame(*t))).collect();
            assert_eq!(p.frame(0.0).len(), 68, "{m:?}");
            assert_eq!(frames.len() > 1, !still.contains(&m), "{m:?}");
        }
        assert!(EffectPlayer::new(effect(Mode::Off), &l).frame(1.0).values().all(|c| *c == Rgb::BLACK));
    }

    #[test]
    fn matches_the_previews_colour_maths() {
        assert_eq!(hue(0.0), Rgb(255, 0, 0));
        assert_eq!(hue(120.0), Rgb(0, 255, 0));
        assert_eq!(hue(240.0), Rgb(0, 0, 255));
        assert_eq!(dim(Rgb(255, 255, 255), 0.5), Rgb(188, 188, 188), "half the light, in linear light");
        assert_eq!(dim(Rgb(10, 20, 30), 0.0), Rgb::BLACK);
    }

    #[test]
    fn bars_rise_from_the_bottom() {
        let l = keylume_proto::standard::standard("ansi-full").unwrap();
        let red = Rgb(255, 0, 0);
        let none = bars(&l, &[0; 32], red, false);
        let full = bars(&l, &[6; 32], red, false);
        assert!(none.values().all(|c| *c != red));
        assert!(full.values().all(|c| *c == red));
        let half = bars(&l, &[3; 32], red, false);
        assert_eq!(half["space"], red);
        assert_ne!(half["esc"], red);
    }
}
