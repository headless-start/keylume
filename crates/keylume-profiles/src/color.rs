//! Colour maths for pattern rendering. Mixing happens in linear light so gradients
//! don't go muddy in the middle, and a small deterministic PRNG for "random" patterns.

use keylume_proto::Rgb;

fn to_lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_lin(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// Blend `a` → `b` by `t` (0..=1) in linear light.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| from_lin(to_lin(x) + (to_lin(y) - to_lin(x)) * t);
    Rgb(f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

/// Sample an evenly spaced colour ramp at `t` (0..=1).
pub fn ramp(stops: &[Rgb], t: f32) -> Rgb {
    match stops.len() {
        0 => Rgb::BLACK,
        1 => stops[0],
        n => {
            let t = t.clamp(0.0, 1.0) * (n - 1) as f32;
            let i = (t.floor() as usize).min(n - 2);
            mix(stops[i], stops[i + 1], t - i as f32)
        }
    }
}

/// Scale brightness (0..=1) in linear light.
pub fn dim(c: Rgb, k: f32) -> Rgb {
    mix(Rgb::BLACK, c, k)
}

/// Deterministic xorshift PRNG so "random" patterns render identically every time.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Stable hash of a string, for seeding per-palette randomness.
pub fn seed(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_endpoints_and_midpoint() {
        let (a, b) = (Rgb(0, 0, 255), Rgb(255, 255, 255));
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        // linear-light midpoint is brighter than the naive sRGB midpoint (127)
        assert!(mix(a, b, 0.5).0 > 150);
        assert_eq!(ramp(&[a, b], 1.0), b);
    }

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let (mut r1, mut r2) = (Rng::new(7), Rng::new(7));
        for _ in 0..1000 {
            let v = r1.next_f32();
            assert!((0.0..1.0).contains(&v));
            assert_eq!(v, r2.next_f32());
        }
    }
}
