//! Tiny deterministic noise helpers.

/// Hash an integer to 0..1.
pub fn hash01(x: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    h ^= h >> 15;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 13;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// Smooth 1-D value noise in 0..1.
pub fn value_noise(x: f32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash01(i as i64 as u32);
    let b = hash01((i as i64 + 1) as u32);
    a + (b - a) * u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_bounded_and_continuous() {
        let mut prev = value_noise(0.0);
        for i in 1..2000 {
            let v = value_noise(i as f32 * 0.01);
            assert!((0.0..=1.0).contains(&v));
            assert!((v - prev).abs() < 0.05);
            prev = v;
        }
    }
}
