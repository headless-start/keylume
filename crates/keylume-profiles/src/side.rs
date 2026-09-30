//! "Match my lighting" for the side strip: a side-light look derived from any profile.

use keylume_proto::{Effect, Mode, Rgb, SideLight, SideMode};

use crate::Lighting;

/// The most representative colour of a per-key picture, at full brightness: an
/// average weighted toward bright, saturated keys (the design, not the background).
pub fn dominant(colors: impl IntoIterator<Item = Rgb>) -> Rgb {
    let (mut r, mut g, mut b, mut w) = (0f32, 0f32, 0f32, 0f32);
    for c in colors {
        let (cr, cg, cb) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
        let max = cr.max(cg).max(cb);
        let min = cr.min(cg).min(cb);
        let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
        let weight = max * max * (0.25 + sat);
        r += cr * weight;
        g += cg * weight;
        b += cb * weight;
        w += weight;
    }
    if w <= 0.0 {
        return Rgb::BLACK;
    }
    let (r, g, b) = (r / w, g / w, b / w);
    let m = r.max(g).max(b).max(1e-6);
    let f = |v: f32| (v / m * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(f(r), f(g), f(b))
}

fn from_effect(e: &Effect) -> SideLight {
    let base = SideLight { speed: e.speed, brightness: e.brightness, rainbow: e.rainbow, ..SideLight::new(SideMode::Static, e.color) };
    match e.mode {
        Mode::Off => SideLight::off(),
        Mode::Static => base,
        Mode::Breathing => SideLight { mode: SideMode::Breathing, ..base },
        Mode::Spectrum => SideLight { mode: SideMode::Neon, rainbow: false, ..base },
        Mode::Wave => SideLight { mode: SideMode::Wave, direction: e.direction.min(3), ..base },
        Mode::Snake => SideLight { mode: SideMode::Snake, direction: e.direction.min(1), ..base },
        // keypress effects leave the keys dark until you type: a soft steady glow
        Mode::Reactive | Mode::ReactiveOff | Mode::Laser | Mode::Ripple => {
            if e.rainbow {
                SideLight { mode: SideMode::Neon, rainbow: false, ..base }
            } else {
                SideLight { brightness: e.brightness.min(2), ..base }
            }
        }
        Mode::MusicBars | Mode::MusicPulse | Mode::ScreenSync | Mode::UserPicture => base,
        _ if e.rainbow => SideLight { mode: SideMode::Neon, rainbow: false, ..base },
        _ => SideLight { mode: SideMode::Breathing, ..base },
    }
}

/// What the side strip shows for `lighting` when it follows the keyboard.
pub fn matching(lighting: &Lighting) -> SideLight {
    match lighting {
        Lighting::Effect { effect } => from_effect(effect),
        Lighting::PerKey { keys, brightness } => {
            SideLight { brightness: *brightness, ..SideLight::new(SideMode::Static, dominant(keys.values().copied())) }
        }
        Lighting::Live { live } => match live.primary_color() {
            Some(c) => SideLight { speed: 3, ..SideLight::new(SideMode::Breathing, c) },
            None => SideLight { speed: 2, ..SideLight::new(SideMode::Neon, Rgb(255, 255, 255)) },
        },
        Lighting::Spell { words, .. } => SideLight::new(SideMode::Static, words.first().map(|w| w.color).unwrap_or(Rgb(0x1F, 0x45, 0xFF))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{builtin, Layout};

    #[test]
    fn every_builtin_profile_has_a_valid_side_look() {
        for p in builtin(&Layout::tk68()) {
            matching(&p.lighting).validate().unwrap_or_else(|e| panic!("{}: {e}", p.id));
        }
    }

    #[test]
    fn follows_the_effect() {
        let wave = Effect { direction: 1, color: Rgb(0, 0, 255), ..Effect::new(Mode::Wave) };
        let s = matching(&Lighting::Effect { effect: wave });
        assert_eq!((s.mode, s.direction, s.color), (SideMode::Wave, 1, Rgb(0, 0, 255)));
        assert_eq!(matching(&Lighting::Effect { effect: Effect::new(Mode::Spectrum) }).mode, SideMode::Neon);
        assert_eq!(matching(&Lighting::Effect { effect: Effect::new(Mode::Off) }).mode, SideMode::Off);
    }

    #[test]
    fn dominant_colour_prefers_the_design_over_the_background() {
        // mostly dim navy background, a few bright cyan keys
        let mut keys = vec![Rgb(2, 6, 40); 60];
        keys.extend(vec![Rgb(0, 220, 255); 8]);
        let d = dominant(keys);
        assert!(d.1 > 150 && d.2 == 255, "{d:?}");
        assert_eq!(dominant(vec![Rgb::BLACK; 3]), Rgb::BLACK);
    }
}
