//! Procedural patterns for the 32-bar channel (the firmware's music modes).
//!
//! Each style is a pure function of time and bar index returning a height 0..=1.

use std::f32::consts::PI;

use keylume_proto::stream::BANDS;
use serde::{Deserialize, Serialize};

use crate::noise::{hash01, value_noise};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarStyle {
    /// Two interfering waves, endlessly morphing.
    Plasma,
    /// Balls bouncing across the board.
    Bounce,
    /// Flames licking up from the bottom, hottest in the middle.
    Fire,
    /// Ripples spreading out from the centre.
    Fountain,
    /// A twisting double helix.
    Helix,
    /// A heartbeat trace scrolling across, like a hospital monitor.
    Ecg,
    /// Blocks stacking up until the board is full, then clearing.
    Stacker,
    /// Calm, then sudden bursts of digital noise.
    Glitch,
    /// Everything thumps together to a kick drum.
    Pump,
    /// Random sparks that flare and fade.
    Fireflies,
    /// A mountain range scrolling past.
    Terrain,
    /// Shells rising and bursting into falling sparks.
    Fireworks,
    /// A bright head sweeping past, trailing a fading tail.
    Comet,
    /// Columns of falling code, like a terminal rain.
    Matrix,
    /// A short run of bars sliding around the board.
    Snake,
    /// A VU meter: bars rise with the beat and fall back slowly.
    Vu,
    /// Drops landing at random, their rings spreading outwards.
    Ripple,
    /// Each bar swings at its own period, like the pendulum-wave toy: patterns form and dissolve.
    Pendulum,
    /// Dark, then a strike branches to its neighbours and fades.
    Lightning,
    /// A ball volleys between two paddles at the ends, each tracking its return.
    Rally,
    /// Grains piling up from one end, then draining away, over and over.
    Hourglass,
    /// A progress fill sweeping left to right with a bright leading edge, then resetting.
    Loading,
    /// Two pulses run in from both ends, meet in the middle and burst outward.
    Collide,
}

impl BarStyle {
    pub const ALL: [BarStyle; 23] = [
        BarStyle::Plasma,
        BarStyle::Bounce,
        BarStyle::Fire,
        BarStyle::Fountain,
        BarStyle::Helix,
        BarStyle::Ecg,
        BarStyle::Stacker,
        BarStyle::Glitch,
        BarStyle::Pump,
        BarStyle::Fireflies,
        BarStyle::Terrain,
        BarStyle::Fireworks,
        BarStyle::Comet,
        BarStyle::Matrix,
        BarStyle::Snake,
        BarStyle::Vu,
        BarStyle::Ripple,
        BarStyle::Pendulum,
        BarStyle::Lightning,
        BarStyle::Rally,
        BarStyle::Hourglass,
        BarStyle::Loading,
        BarStyle::Collide,
    ];
}

impl BarStyle {
    /// Name for the UI and for generated profile names.
    pub fn label(self) -> &'static str {
        match self {
            BarStyle::Plasma => "Plasma",
            BarStyle::Bounce => "Bouncing Balls",
            BarStyle::Fire => "Fire",
            BarStyle::Fountain => "Fountain",
            BarStyle::Helix => "DNA Helix",
            BarStyle::Ecg => "Heart Monitor",
            BarStyle::Stacker => "Stacker",
            BarStyle::Glitch => "Glitch",
            BarStyle::Pump => "Kick Drum",
            BarStyle::Fireflies => "Fireflies",
            BarStyle::Terrain => "Terrain",
            BarStyle::Fireworks => "Fireworks",
            BarStyle::Comet => "Comet",
            BarStyle::Matrix => "Matrix Rain",
            BarStyle::Snake => "Snake",
            BarStyle::Vu => "VU Meter",
            BarStyle::Ripple => "Ripples",
            BarStyle::Pendulum => "Pendulum",
            BarStyle::Lightning => "Lightning",
            BarStyle::Rally => "Rally",
            BarStyle::Hourglass => "Hourglass",
            BarStyle::Loading => "Loading",
            BarStyle::Collide => "Collide",
        }
    }
}

/// ECG waveform over one beat (phase 0..1): P wave, QRS spike, T wave.
fn ecg(phase: f32) -> f32 {
    let g = |c: f32, w: f32, a: f32| a * (-((phase - c) / w).powi(2)).exp();
    (0.12 + g(0.18, 0.035, 0.22) + g(0.30, 0.012, 0.95) - g(0.335, 0.01, 0.12) + g(0.55, 0.06, 0.32)).clamp(0.0, 1.0)
}

/// Heights of all bars at time `t` (seconds), already scaled by `speed`.
pub fn render(style: BarStyle, t: f32, speed: f32) -> [f32; BANDS] {
    let s = t * speed.max(0.05);
    let n = BANDS as f32;
    let mut out = [0.0f32; BANDS];
    for (i, o) in out.iter_mut().enumerate() {
        let x = i as f32;
        *o = match style {
            BarStyle::Plasma => {
                let warp = (s * 0.4).sin() * 3.0;
                0.5 + 0.28 * (x * 0.35 + s * 1.3).sin() + 0.22 * (x * 0.13 - s * 0.7 + warp).sin()
            }
            BarStyle::Bounce => {
                let mut v: f32 = 0.0;
                for b in 0..3u32 {
                    let fb = b as f32;
                    // horizontal ping-pong, each ball at its own pace
                    let p = (s * (0.11 + 0.05 * fb) + hash01(b + 3)).fract() * 2.0;
                    let bx = if p > 1.0 { 2.0 - p } else { p } * (n - 1.0);
                    // vertical bounce: |sin| is a decent parabola
                    let h = (s * (2.1 + 0.6 * fb) + fb).sin().abs();
                    v = v.max(h * (1.0 - (x - bx).abs() / 1.6).max(0.0));
                }
                v
            }
            BarStyle::Fire => {
                let centre = 1.0 - ((x - (n - 1.0) / 2.0).abs() / (n / 2.0)).powi(2) * 0.55;
                let flick = 0.6 * value_noise(s * 5.0 + x * 5.3) + 0.4 * value_noise(s * 11.0 + x * 1.7 + 40.0);
                centre * (0.3 + 0.7 * flick)
            }
            BarStyle::Fountain => {
                let d = (x - (n - 1.0) / 2.0).abs();
                (0.5 + 0.5 * (d * 0.6 - s * 4.0).sin()) * (1.0 - d / n * 0.6)
            }
            BarStyle::Helix => {
                let a = (x * 0.3 + s * 2.2).sin();
                let b = (x * 0.3 + s * 2.2 + PI * 0.66).sin();
                0.15 + 0.85 * (0.5 + 0.5 * a).max(0.5 + 0.5 * b) * (0.65 + 0.35 * (x * 0.08 - s * 0.9).cos())
            }
            BarStyle::Ecg => ecg((s * 0.9 - x * 0.028).rem_euclid(1.0)),
            BarStyle::Stacker => {
                let len = 7.0;
                let cycle = (s / len).floor();
                let f = (s / len).fract();
                if f > 0.9 {
                    // full board blinks, then clears
                    if ((f - 0.9) * 40.0) as i32 % 2 == 0 {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    let order = hash01(i as u32 * 131 + cycle as u32 * 7919);
                    ((f / 0.9) * 1.35 - order * 0.35).clamp(0.0, 1.0)
                }
            }
            BarStyle::Glitch => {
                let slot = (s * 7.0).floor() as u32;
                let burst = hash01(slot / 3 * 17 + 5) < 0.3;
                if burst {
                    let r = hash01(slot * 97 + i as u32 * 13);
                    if r < 0.55 {
                        r / 0.55
                    } else {
                        0.0
                    }
                } else {
                    0.06 + 0.12 * value_noise(s * 3.0 + x * 2.1)
                }
            }
            BarStyle::Pump => {
                let beat = (s * 2.0 - x * 0.012).fract();
                0.12 + 0.88 * (-beat * 5.0).exp()
            }
            BarStyle::Fireflies => {
                // each bar has its own irregular sparks
                let rate = 0.35 + 0.5 * hash01(i as u32 * 7 + 1);
                let local = s * rate + hash01(i as u32 * 29 + 11) * 10.0;
                let slot = local.floor() as u32;
                let d = local.fract();
                if hash01(slot * 53 + i as u32) < 0.55 {
                    (-d * 5.0).exp()
                } else {
                    0.0
                }
            }
            BarStyle::Terrain => {
                let p = x * 0.28 + s * 2.2;
                0.08 + 0.92 * (0.65 * value_noise(p) + 0.35 * value_noise(p * 2.7 + 13.0)).powf(1.3)
            }
            BarStyle::Fireworks => {
                // three shells at a time: each rises, bursts, then the sparks fall
                let mut v: f32 = 0.05 + 0.05 * value_noise(s + x * 0.3);
                for shell in 0..3u32 {
                    let local = s * 0.35 + hash01(shell * 31 + 3) * 10.0;
                    let (slot, f) = (local.floor() as u32, local.fract());
                    let at = hash01(slot * 61 + shell) * (n - 1.0);
                    let d = (x - at).abs();
                    if f < 0.45 {
                        // the shell climbing: a single narrow bar growing
                        v = v.max(if d < 0.8 { f / 0.45 } else { 0.0 });
                    } else {
                        // the burst spreading and fading
                        let age = (f - 0.45) / 0.55;
                        let radius = age * n * 0.45;
                        let near = (1.0 - (d - radius).abs() / 1.6).max(0.0);
                        v = v.max(near * (1.0 - age).powi(2));
                    }
                }
                v
            }
            BarStyle::Comet => {
                let head = (s * 0.5).fract() * (n + 8.0) - 4.0;
                let behind = head - x;
                if behind < 0.0 {
                    0.04
                } else {
                    (0.04 + (-behind / 4.5).exp()).min(1.0)
                }
            }
            BarStyle::Matrix => {
                // each column drops at its own pace, brightest at the head
                let rate = 0.5 + 1.4 * hash01(i as u32 * 17 + 1);
                let local = s * rate + hash01(i as u32 * 41 + 7) * 12.0;
                let f = local.fract();
                if hash01(local.floor() as u32 * 29 + i as u32) < 0.65 {
                    (1.0 - f).powi(2) * 0.9 + 0.05
                } else {
                    0.05
                }
            }
            BarStyle::Snake => {
                let body = 12.0;
                let head = (s * 3.0).rem_euclid(n * 2.0);
                let head = if head > n { n * 2.0 - head } else { head };
                let behind = (head - x).rem_euclid(n);
                (1.0 - behind / body).max(0.06)
            }
            BarStyle::Vu => {
                // loudness swells and drops; the bars fall away from the middle
                let level = (0.5 + 0.5 * (s * 1.7).sin() * (s * 0.37).cos()).powf(0.7);
                let reach = level * n;
                let d = (x - (n - 1.0) / 2.0).abs() * 2.0;
                (0.06 + (reach - d) / 3.0).clamp(0.0, 1.0)
            }
            BarStyle::Ripple => {
                let mut v: f32 = 0.05;
                for drop in 0..3u32 {
                    let local = s * 0.6 + hash01(drop * 13 + 5) * 7.0;
                    let (slot, f) = (local.floor() as u32, local.fract());
                    let at = hash01(slot * 71 + drop * 3) * (n - 1.0);
                    let radius = f * n * 0.5;
                    let ring = (1.0 - ((x - at).abs() - radius).abs() / 1.3).max(0.0);
                    v = v.max(ring * (1.0 - f));
                }
                v
            }
            BarStyle::Pendulum => {
                // each bar swings at its own slightly different period: in step, then
                // gradually out of step, then back in step again (the classic toy)
                let period = 1.1 + 0.5 * (x / (n - 1.0));
                0.5 + 0.5 * (s * std::f32::consts::TAU / period).sin()
            }
            BarStyle::Lightning => {
                // A faint base flicker so the board is never fully black, plus strikes
                // roughly every 0.5-1 s (slot_len / strike chance) that branch outward
                // from a random origin and take a couple of slots to fade out.
                let slot_len = 0.55;
                let strike_chance = 0.8;
                let mut v = 0.05 + 0.05 * value_noise(s * 5.0 + x * 3.1);
                let slot0 = (s / slot_len).floor() as i64;
                for back in 0..3i64 {
                    let slot = slot0 - back;
                    if slot < 0 || hash01(slot as u32 * 53 + 7) >= strike_chance {
                        continue;
                    }
                    let elapsed = s - slot as f32 * slot_len;
                    let origin = hash01(slot as u32 * 91 + 3) * (n - 1.0);
                    let d = (x - origin).abs();
                    let decay = (-elapsed / 0.55).exp();
                    let branch = (-d / 2.4).exp();
                    v = v.max(branch * decay);
                }
                v
            }
            BarStyle::Rally => {
                let period = 3.0;
                let ball_at = |ss: f32| {
                    let p = (ss / period).fract() * 2.0;
                    (if p > 1.0 { 2.0 - p } else { p }) * (n - 1.0)
                };
                let ball_x = ball_at(s);
                // the ball itself, plus a short fading trail behind where it just was
                let mut v = 0.0f32;
                for (back, w) in [(0.0, 1.0), (0.06, 0.6), (0.12, 0.35), (0.18, 0.18)] {
                    let bx = ball_at((s - back).max(0.0));
                    v = v.max(w * (1.0 - (x - bx).abs() / 1.4).max(0.0));
                }
                let paddle = |dist: f32| (1.0 - dist / (n * 0.4)).clamp(0.28, 1.0);
                let left_paddle = if x < 2.5 { paddle(ball_x) } else { 0.0 };
                let right_paddle = if x > n - 3.5 { paddle(n - 1.0 - ball_x) } else { 0.0 };
                v.max(left_paddle).max(right_paddle)
            }
            BarStyle::Hourglass => {
                let cycle = 10.0;
                let c = s.rem_euclid(cycle);
                let half = cycle / 2.0;
                // fills for one half of the cycle, empties for the other
                let f = if c < half { c / half } else { 1.0 - (c - half) / half };
                let level = f * n;
                let filled = if (n - 1.0 - x) < level { 0.85 } else { 0.05 };
                // the grain currently falling through the neck
                let grain = (-((n - 1.0 - level) - x).abs() / 0.7).exp() * (0.5 + 0.5 * (s * 16.0).sin()).powi(2);
                filled + grain
            }
            BarStyle::Loading => {
                let cycle = 2.5;
                let f = (s / cycle).fract();
                let level = f * n;
                let filled = if x < level { 0.55 } else { 0.04 };
                let edge = (-(level - x).abs() / 0.8).exp();
                filled + 0.6 * edge
            }
            BarStyle::Collide => {
                let cycle = 3.0;
                let approach = 0.6;
                let c = s.rem_euclid(cycle);
                if c < cycle * approach {
                    let f = c / (cycle * approach);
                    let pos_l = f * (n - 1.0) / 2.0;
                    let pos_r = (n - 1.0) - pos_l;
                    let left = (-(x - pos_l).abs() / 1.2).exp();
                    let right = (-(x - pos_r).abs() / 1.2).exp();
                    left.max(right)
                } else {
                    let f = (c - cycle * approach) / (cycle * (1.0 - approach));
                    let centre = (n - 1.0) / 2.0;
                    let radius = f * n * 0.6;
                    let ring = (1.0 - ((x - centre).abs() - radius).abs() / 1.6).max(0.0);
                    ring * (1.0 - f)
                }
            }
        }
        .clamp(0.0, 1.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_styles_stay_in_range_and_move() {
        for style in BarStyle::ALL {
            let mut changed = false;
            let first = render(style, 0.0, 1.0);
            for k in 0..400 {
                let f = render(style, k as f32 * 0.05, 1.0);
                assert!(f.iter().all(|v| (0.0..=1.0).contains(v)), "{style:?}");
                changed |= f != first;
            }
            assert!(changed, "{style:?} never animates");
        }
    }

    #[test]
    fn ecg_has_one_tall_spike_per_beat() {
        let v: Vec<f32> = (0..1000).map(|i| ecg(i as f32 / 1000.0)).collect();
        let spikes = (1..999).filter(|&i| v[i] > 0.8 && v[i] >= v[i - 1] && v[i] > v[i + 1]).count();
        assert_eq!(spikes, 1);
    }

    #[test]
    fn styles_round_trip_as_camel_case() {
        assert_eq!(serde_json::to_string(&BarStyle::Fireflies).unwrap(), "\"fireflies\"");
        for s in BarStyle::ALL {
            let j = serde_json::to_string(&s).unwrap();
            assert_eq!(serde_json::from_str::<BarStyle>(&j).unwrap(), s);
        }
    }
}
