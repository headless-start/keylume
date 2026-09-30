//! sRGB → linear-light colour correction for the keyboard's LEDs.
//!
//! Colours on screen are gamma-encoded (sRGB) for the eye; the keyboard's LEDs respond
//! (we expect) linearly to the value they're sent. Sent as-is, mid-tones (orange, pink,
//! pastels, dim backgrounds) would come out brighter and washed out on the keys. How well
//! this matches a real board is checked by eye (docs/CALIBRATION.md), not measured.
//! [`to_led`] decodes the sRGB curve before a colour goes to the board; [`from_led`]
//! re-encodes it, e.g. when the editor reads a picture layer back. Both go through a
//! 256-entry lookup table, built once.

use std::sync::OnceLock;

use keylume_proto::Rgb;

/// The sRGB decoding (EOTF) curve: screen value -> linear light.
fn decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// The sRGB encoding curve: linear light -> screen value (the inverse of `decode`).
fn encode(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn table(curve: fn(f32) -> f32) -> [u8; 256] {
    let mut t = [0u8; 256];
    for (i, slot) in t.iter_mut().enumerate() {
        *slot = (curve(i as f32 / 255.0).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    t
}

fn apply(c: Rgb, t: &[u8; 256]) -> Rgb {
    Rgb(t[c.0 as usize], t[c.1 as usize], t[c.2 as usize])
}

/// Correct a colour for the LEDs' linear response, per channel.
pub fn to_led(c: Rgb) -> Rgb {
    static T: OnceLock<[u8; 256]> = OnceLock::new();
    apply(c, T.get_or_init(|| table(decode)))
}

/// The inverse of [`to_led`]: what a colour already on the LEDs looks like on screen.
pub fn from_led(c: Rgb) -> Rgb {
    static T: OnceLock<[u8; 256]> = OnceLock::new();
    apply(c, T.get_or_init(|| table(encode)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_worked_example() {
        // mid-grey is the classic case: sRGB 0x80 decodes to roughly 21.5% linear light.
        assert_eq!(to_led(Rgb(0x80, 0x80, 0x80)), Rgb(0x37, 0x37, 0x37));
    }

    #[test]
    fn black_and_white_are_fixed_points() {
        for c in [Rgb::BLACK, Rgb(255, 255, 255)] {
            assert_eq!(to_led(c), c);
            assert_eq!(from_led(c), c);
        }
    }

    #[test]
    fn mid_tones_get_darker_going_to_the_leds() {
        let orange = Rgb(0xff, 0x99, 0x33);
        let led = to_led(orange);
        assert!(led.1 < 0x99 && led.2 < 0x33, "{orange} -> {led}");
        assert_eq!(led.0, 0xff, "the fully-on channel is untouched");
    }

    #[test]
    fn to_led_and_from_led_round_trip_within_one() {
        // From mid-tones up the round trip is exact or within 1. (Below this, 8-bit
        // quantization compresses a wide range of near-black screen values into a
        // narrow sliver of linear ones, so a few of the darkest shades round-trip
        // further off — expected, and not the case this correction is for.)
        for v in 50..=255u8 {
            let back = from_led(to_led(Rgb(v, v, v)));
            assert!(back.0.abs_diff(v) <= 1, "{v} round-tripped to {back}");
        }
    }
}
