//! Host-driven ("live") lighting.
//!
//! The TK68 firmware exposes two real-time channels (see `keylume_proto::stream`):
//! a whole-board colour (screen-sync mode) and 32 band levels (music modes). A
//! [`LiveEffect`] describes an animation; [`Animator`] turns it into a stream of
//! [`LiveFrame`]s given the time and optional inputs (audio spectrum, CPU load, the
//! screen's average colour). Everything here is deterministic and unit-tested; the
//! app supplies the inputs and pushes frames to the keyboard.

use keylume_proto::stream::{BANDS, MAX_LEVEL};
use keylume_proto::{Effect, Mode, Rgb};
use serde::{Deserialize, Serialize};

pub mod audio;
pub mod bars;
pub mod effects;
mod morse;
mod noise;

pub use bars::BarStyle;

use noise::{hash01, value_noise};

/// What the keyboard should show right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LiveFrame {
    Color(Rgb),
    Levels([u8; BANDS]),
}

/// Live data the app feeds in each tick. All optional; animators degrade gracefully.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    /// 32 audio bands, each 0..=1 (see [`audio::Analyzer`]).
    pub bands: Option<[f32; BANDS]>,
    /// Overall loudness 0..=1.
    pub loudness: Option<f32>,
    /// CPU load 0..=1.
    pub cpu: Option<f32>,
    /// Average colour of the screen.
    pub screen: Option<Rgb>,
}

/// Most colours a live effect's palette may hold.
pub const MAX_COLORS: usize = 16;
/// Longest Morse message, in characters.
pub const MAX_MESSAGE: usize = 64;

/// Declarative description of a live animation (stored in profiles, edited in the UI).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LiveEffect {
    /// Smoothly cycle through colours.
    PaletteFlow {
        colors: Vec<Rgb>,
        #[serde(default = "d_period")]
        period: f32,
    },
    /// Lub-dub double pulse.
    Heartbeat {
        color: Rgb,
        #[serde(default = "d_bpm")]
        bpm: f32,
    },
    /// Organic flicker around a base colour (candle, flame, plasma).
    Flicker {
        color: Rgb,
        #[serde(default = "d_half")]
        intensity: f32,
    },
    /// Slow swell between two colours.
    Tide {
        a: Rgb,
        b: Rgb,
        #[serde(default = "d_period")]
        period: f32,
    },
    /// Dim sky with random lightning flashes.
    Storm {
        sky: Rgb,
        flash: Rgb,
        #[serde(default = "d_rate")]
        rate: f32,
    },
    /// Hard alternation between two colours.
    Strobe {
        a: Rgb,
        b: Rgb,
        #[serde(default = "d_hz")]
        hz: f32,
    },
    /// Colour and brightness follow the music's loudness.
    AudioGlow { quiet: Rgb, loud: Rgb },
    /// Colour follows CPU load.
    CpuHeat { cool: Rgb, hot: Rgb },
    /// Mirror the screen's average colour.
    ScreenAmbient {
        #[serde(default = "d_one")]
        saturation: f32,
    },
    /// Real spectrum analyser (music modes). `mirror` puts the bass in the middle.
    Spectrum {
        color: Rgb,
        #[serde(default)]
        rainbow: bool,
        #[serde(default)]
        mirror: bool,
    },
    /// Rolling sine wave across the bars.
    SineBars {
        color: Rgb,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// Drops falling down the bars.
    RainBars {
        color: Rgb,
        #[serde(default = "d_rate")]
        rate: f32,
    },
    /// A bright bar sweeping left-right (scanner).
    Scanner {
        color: Rgb,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// Plausible fake equaliser for when nothing is playing.
    Equalizer {
        color: Rgb,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// Colours drifting on several out-of-sync oscillators: hypnotic, never quite repeating.
    Trip {
        colors: Vec<Rgb>,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// Slow blobs of colour welling up like a lava lamp.
    Lava {
        colors: Vec<Rgb>,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// Deep breaths, moving to the next colour on every breath.
    Breathe {
        colors: Vec<Rgb>,
        #[serde(default = "d_breaths")]
        bpm: f32,
    },
    /// Hard cuts between colours on the beat, each hit decaying.
    Rave {
        colors: Vec<Rgb>,
        #[serde(default = "d_rave")]
        bpm: f32,
    },
    /// Blink a message in Morse code.
    Morse {
        message: String,
        color: Rgb,
        background: Rgb,
        #[serde(default = "d_wpm")]
        wpm: f32,
    },
    /// A planted C4: beeps that speed up, then the blast.
    Bomb {
        color: Rgb,
        blast: Rgb,
        #[serde(default = "d_fuse")]
        fuse: f32,
    },
    /// Every so often a flashbang whites everything out, then it fades back.
    Flashbang {
        color: Rgb,
        #[serde(default = "d_every")]
        every: f32,
    },
    /// Pomodoro timer: `work` colour for `minutes`, then `rest` colour for the break.
    Focus {
        work: Rgb,
        rest: Rgb,
        #[serde(default = "d_focus")]
        minutes: f32,
        #[serde(default = "d_break", rename = "breakMinutes")]
        break_minutes: f32,
    },
    /// Flash on every beat of the music, stepping through the colours.
    BeatFlash { colors: Vec<Rgb> },
    /// Procedural patterns on the bars.
    Bars {
        style: bars::BarStyle,
        color: Rgb,
        #[serde(default)]
        rainbow: bool,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// Slow curtains of colour drifting like the northern lights.
    Aurora {
        colors: Vec<Rgb>,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// A day in fast forward: night, dawn, daylight, dusk and back.
    Sunrise {
        #[serde(default = "d_day")]
        minutes: f32,
    },
    /// A timer: the colour walks from `color` to `warn`, blinks for the last ten
    /// seconds, then pulses until you stop it.
    Countdown {
        color: Rgb,
        warn: Rgb,
        #[serde(default = "d_focus")]
        minutes: f32,
    },
    /// Drifts through the colours, faster the louder the music.
    MusicFlow { colors: Vec<Rgb> },
    /// Two colours in a police-style double-flash pattern.
    Siren {
        a: Rgb,
        b: Rgb,
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// A base colour with short random sparkles.
    Glitter {
        base: Rgb,
        spark: Rgb,
        #[serde(default = "d_density")]
        density: f32,
    },
    /// A guided breath: rises on the inhale, holds, falls on the exhale, rests.
    PacedBreathing {
        color: Rgb,
        #[serde(default = "d_breath_inhale")]
        inhale: f32,
        #[serde(default = "d_breath_hold")]
        hold: f32,
        #[serde(default = "d_breath_exhale")]
        exhale: f32,
        #[serde(default = "d_breath_rest")]
        rest: f32,
    },
    /// Hard cuts between colours, no fade, each held for a while.
    Steps {
        colors: Vec<Rgb>,
        #[serde(default = "d_step_hold")]
        hold: f32,
    },
}

fn d_day() -> f32 {
    10.0
}

fn d_breaths() -> f32 {
    10.0
}
fn d_rave() -> f32 {
    128.0
}
fn d_wpm() -> f32 {
    10.0
}
fn d_fuse() -> f32 {
    40.0
}
fn d_every() -> f32 {
    12.0
}
fn d_focus() -> f32 {
    25.0
}
fn d_break() -> f32 {
    5.0
}
fn d_period() -> f32 {
    8.0
}
fn d_bpm() -> f32 {
    60.0
}
fn d_half() -> f32 {
    0.5
}
fn d_rate() -> f32 {
    1.0
}
fn d_hz() -> f32 {
    2.0
}
fn d_one() -> f32 {
    1.0
}
fn d_speed() -> f32 {
    1.0
}
fn d_density() -> f32 {
    0.5
}
fn d_breath_inhale() -> f32 {
    4.0
}
fn d_breath_hold() -> f32 {
    4.0
}
fn d_breath_exhale() -> f32 {
    4.0
}
fn d_breath_rest() -> f32 {
    4.0
}
fn d_step_hold() -> f32 {
    2.0
}

impl LiveEffect {
    /// The firmware effect that must be active for this animation's frames to show.
    pub fn base_effect(&self) -> Effect {
        match self {
            LiveEffect::Spectrum { color, rainbow, .. } | LiveEffect::Bars { color, rainbow, .. } => {
                Effect { color: *color, rainbow: *rainbow, ..Effect::new(Mode::MusicBars) }
            }
            LiveEffect::SineBars { color, .. }
            | LiveEffect::RainBars { color, .. }
            | LiveEffect::Scanner { color, .. }
            | LiveEffect::Equalizer { color, .. } => Effect { color: *color, ..Effect::new(Mode::MusicBars) },
            _ => Effect::new(Mode::ScreenSync),
        }
    }

    /// The colour that best represents this effect (thumbnails, side strip), or None
    /// when it's multicoloured by nature (rainbow, screen-driven).
    pub fn primary_color(&self) -> Option<Rgb> {
        use LiveEffect::*;
        match self {
            PaletteFlow { colors, .. }
            | Trip { colors, .. }
            | Lava { colors, .. }
            | Breathe { colors, .. }
            | Rave { colors, .. }
            | BeatFlash { colors } => {
                // a palette whose colours are all blue-ish still has a primary colour
                colors.first().copied().filter(|_| colors.len() <= 5)
            }
            Heartbeat { color, .. }
            | Flicker { color, .. }
            | SineBars { color, .. }
            | RainBars { color, .. }
            | Scanner { color, .. }
            | Equalizer { color, .. }
            | Morse { color, .. }
            | Bomb { color, .. }
            | Flashbang { color, .. }
            | PacedBreathing { color, .. } => Some(*color),
            Spectrum { color, rainbow, .. } | Bars { color, rainbow, .. } => (!rainbow).then_some(*color),
            Tide { b, .. } => Some(*b),
            Storm { flash, .. } => Some(*flash),
            Strobe { a, .. } | Siren { a, .. } => Some(*a),
            AudioGlow { loud, .. } => Some(*loud),
            CpuHeat { cool, .. } => Some(*cool),
            Focus { work, .. } => Some(*work),
            Aurora { colors, .. } | MusicFlow { colors } | Steps { colors, .. } => colors.first().copied().filter(|_| colors.len() <= 5),
            Countdown { color, .. } => Some(*color),
            Glitter { base, .. } => Some(*base),
            Sunrise { .. } | ScreenAmbient { .. } => None,
        }
    }

    /// Which live inputs this effect wants the app to collect.
    pub fn needs(&self) -> Needs {
        match self {
            LiveEffect::Spectrum { .. } | LiveEffect::AudioGlow { .. } | LiveEffect::BeatFlash { .. } | LiveEffect::MusicFlow { .. } => {
                Needs { audio: true, ..Needs::default() }
            }
            LiveEffect::CpuHeat { .. } => Needs { cpu: true, ..Needs::default() },
            LiveEffect::ScreenAmbient { .. } => Needs { screen: true, ..Needs::default() },
            _ => Needs::default(),
        }
    }

    /// Is this something the animator can run sensibly? Checked wherever an effect comes
    /// in from outside (uploads, saves, previews): palettes of 1 to [`MAX_COLORS`], a
    /// Morse message of up to [`MAX_MESSAGE`] characters, and every number finite and in
    /// a sane range (generous next to the editor's sliders).
    pub fn validate(&self) -> Result<(), String> {
        use LiveEffect::*;
        let within = |name: &str, v: f32, lo: f32, hi: f32| -> Result<(), String> {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!("{name} must be between {lo} and {hi}"))
            }
        };
        let palette = |colors: &[Rgb]| -> Result<(), String> {
            if (1..=MAX_COLORS).contains(&colors.len()) {
                Ok(())
            } else {
                Err(format!("it needs 1 to {MAX_COLORS} colours"))
            }
        };
        let speed = |v: f32| within("speed", v, 0.01, 10.0);
        match self {
            PaletteFlow { colors, period } => palette(colors).and(within("period", *period, 0.5, 600.0)),
            Heartbeat { bpm, .. } => within("bpm", *bpm, 1.0, 600.0),
            Flicker { intensity, .. } => within("intensity", *intensity, 0.0, 1.0),
            Tide { period, .. } => within("period", *period, 0.5, 600.0),
            Storm { rate, .. } | RainBars { rate, .. } => within("rate", *rate, 0.01, 20.0),
            Strobe { hz, .. } => within("hz", *hz, 0.05, 15.0),
            AudioGlow { .. } | CpuHeat { .. } => Ok(()),
            ScreenAmbient { saturation } => within("saturation", *saturation, 0.0, 4.0),
            Spectrum { .. } => Ok(()),
            SineBars { speed: v, .. } | Scanner { speed: v, .. } | Equalizer { speed: v, .. } | Bars { speed: v, .. } | Siren { speed: v, .. } => {
                speed(*v)
            }
            Trip { colors, speed: v } | Lava { colors, speed: v } | Aurora { colors, speed: v } => palette(colors).and(speed(*v)),
            Breathe { colors, bpm } | Rave { colors, bpm } => palette(colors).and(within("bpm", *bpm, 1.0, 600.0)),
            BeatFlash { colors } | MusicFlow { colors } => palette(colors),
            Morse { message, wpm, .. } => {
                if message.chars().count() > MAX_MESSAGE || message.chars().any(char::is_control) {
                    return Err(format!("a Morse message has up to {MAX_MESSAGE} plain characters"));
                }
                within("wpm", *wpm, 1.0, 60.0)
            }
            Bomb { fuse, .. } => within("fuse", *fuse, 1.0, 600.0),
            Flashbang { every, .. } => within("every", *every, 0.5, 3600.0),
            Focus { minutes, break_minutes, .. } => {
                within("minutes", *minutes, 0.05, 1440.0).and(within("breakMinutes", *break_minutes, 0.05, 1440.0))
            }
            Sunrise { minutes } | Countdown { minutes, .. } => within("minutes", *minutes, 0.05, 1440.0),
            Glitter { density, .. } => within("density", *density, 0.01, 10.0),
            PacedBreathing { inhale, hold, exhale, rest, .. } => within("inhale", *inhale, 0.1, 60.0)
                .and(within("hold", *hold, 0.0, 60.0))
                .and(within("exhale", *exhale, 0.1, 60.0))
                .and(within("rest", *rest, 0.0, 60.0)),
            Steps { colors, hold } => palette(colors).and(within("hold", *hold, 0.05, 600.0)),
        }
    }

    /// How much to fast-forward time when rendering a [`preview`], given the preview's
    /// length in seconds. Effects timed to the real world (a whole day, a multi-minute
    /// timer) would look all but frozen over a short preview, so previews compress a
    /// full cycle (or the interesting part of one) into it. `Animator::frame` itself
    /// never sees this: the live effect always runs at real speed.
    fn preview_speed(&self, seconds: f32) -> f32 {
        let seconds = seconds.max(0.1);
        match self {
            LiveEffect::Sunrise { minutes } => (minutes.max(0.2) * 60.0 / seconds).max(1.0),
            // run a few seconds past zero so the preview reaches the end-of-timer pulse
            LiveEffect::Countdown { minutes, .. } => ((minutes.max(0.1) * 60.0 + 3.0) / seconds).max(1.0),
            LiveEffect::Focus { minutes, break_minutes, .. } => {
                // one full work+break cycle, so the preview crosses into the break
                ((minutes.max(0.1) + break_minutes.max(0.1)) * 60.0 / seconds).max(1.0)
            }
            // one full fuse-to-blast cycle, so the preview reaches the detonation
            LiveEffect::Bomb { fuse, .. } => ((fuse.max(5.0) + 4.0) / seconds).max(1.0),
            _ => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Needs {
    pub audio: bool,
    pub cpu: bool,
    pub screen: bool,
}

/// Stateful renderer for one [`LiveEffect`].
pub struct Animator {
    effect: LiveEffect,
    /// smoothed values for audio-driven effects
    smooth_bands: [f32; BANDS],
    smooth_level: f32,
    peaks: [f32; BANDS],
    /// Morse message as on/off units (precomputed).
    morse: Vec<bool>,
    /// Beat detector: slow loudness average, time and index of the last beat.
    beat_avg: f32,
    last_beat: Option<f32>,
    beat_idx: usize,
    /// Where a music-driven drift has got to, and when it was last advanced.
    phase: f32,
    last_t: f32,
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgb(f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

fn scale(c: Rgb, k: f32) -> Rgb {
    mix(Rgb::BLACK, c, k)
}

fn ramp(stops: &[Rgb], t: f32) -> Rgb {
    match stops.len() {
        0 => Rgb::BLACK,
        1 => stops[0],
        n => {
            let x = t.rem_euclid(1.0) * n as f32; // wraps: last colour blends back to first
            let i = x.floor() as usize % n;
            let f = x - x.floor();
            let smooth = f * f * (3.0 - 2.0 * f);
            mix(stops[i], stops[(i + 1) % n], smooth)
        }
    }
}

fn quantize(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * MAX_LEVEL as f32).round() as u8
}

impl Animator {
    pub fn new(effect: LiveEffect) -> Self {
        let morse = match &effect {
            LiveEffect::Morse { message, .. } => morse::units(message),
            _ => Vec::new(),
        };
        Animator {
            effect,
            smooth_bands: [0.0; BANDS],
            smooth_level: 0.0,
            peaks: [0.0; BANDS],
            morse,
            beat_avg: 0.0,
            last_beat: None,
            beat_idx: 0,
            phase: 0.0,
            last_t: 0.0,
        }
    }

    pub fn effect(&self) -> &LiveEffect {
        &self.effect
    }

    /// Render the frame for time `t` (seconds since start).
    pub fn frame(&mut self, t: f32, input: &Inputs) -> LiveFrame {
        use LiveEffect::*;
        match &self.effect {
            PaletteFlow { colors, period } => LiveFrame::Color(ramp(colors, t / period.max(0.5))),
            Heartbeat { color, bpm } => {
                let beat = (t * bpm / 60.0).fract();
                let pulse = |c: f32, w: f32| (-((beat - c) / w).powi(2)).exp();
                let k = 0.28 + 0.72 * (pulse(0.08, 0.05) + 0.6 * pulse(0.28, 0.06)).min(1.0);
                LiveFrame::Color(scale(*color, k))
            }
            Flicker { color, intensity } => {
                let n = 0.6 * value_noise(t * 7.0) + 0.4 * value_noise(t * 19.0 + 11.0);
                LiveFrame::Color(scale(*color, 1.0 - intensity.clamp(0.0, 1.0) * 0.8 * n))
            }
            Tide { a, b, period } => {
                let x = 0.5 - 0.5 * (t * std::f32::consts::TAU / period.max(0.5)).cos();
                LiveFrame::Color(mix(*a, *b, x))
            }
            Storm { sky, flash, rate } => {
                // Each 1/rate-second slot may hold a flash with a quick double strike,
                // over a faint ambient shimmer so the sky is never perfectly flat.
                let slot_len = 1.0 / rate.max(0.05);
                let slot = (t / slot_len).floor();
                let within = t - slot * slot_len;
                let strikes = hash01(slot as u32) < 0.45;
                let ambient = 0.05 + 0.05 * value_noise(t * 1.7);
                let k = if strikes {
                    let d = within - hash01(slot as u32 + 7) * slot_len * 0.5;
                    let hit = |c: f32| if (0.0..0.06).contains(&(d - c)) { 1.0 } else { 0.0 };
                    f32::max(hit(0.0), 0.7 * hit(0.12)).max(ambient)
                } else {
                    ambient
                };
                LiveFrame::Color(mix(*sky, *flash, k))
            }
            Strobe { a, b, hz } => LiveFrame::Color(if (t * hz * 2.0).floor() as i64 % 2 == 0 { *a } else { *b }),
            AudioGlow { quiet, loud } => {
                let l = input.loudness.unwrap_or(0.0);
                self.smooth_level += (l - self.smooth_level) * if l > self.smooth_level { 0.6 } else { 0.12 };
                let k = self.smooth_level.clamp(0.0, 1.0);
                LiveFrame::Color(scale(mix(*quiet, *loud, k), 0.25 + 0.75 * k))
            }
            Aurora { colors, speed } => {
                // two slow, out-of-step drifts, with a shimmer on top
                let s = t * speed.max(0.05) * 0.06;
                let x = 0.65 * value_noise(s) + 0.35 * (0.5 + 0.5 * (s * 4.3).sin());
                LiveFrame::Color(scale(ramp(colors, x), 0.72 + 0.28 * value_noise(t * 1.3 + 9.0)))
            }
            Sunrise { minutes } => {
                const DAY: [Rgb; 8] = [
                    Rgb(0x05, 0x08, 0x2a), // deep night
                    Rgb(0x2a, 0x18, 0x7a), // first light
                    Rgb(0xff, 0x6a, 0x20), // sunrise
                    Rgb(0xff, 0xc8, 0x60), // morning
                    Rgb(0xff, 0xff, 0xff), // noon
                    Rgb(0xa0, 0xd0, 0xff), // afternoon
                    Rgb(0xff, 0x7a, 0x30), // sunset
                    Rgb(0x30, 0x10, 0x60), // dusk
                ];
                LiveFrame::Color(ramp(&DAY, t / (minutes.max(0.2) * 60.0)))
            }
            Countdown { color, warn, minutes } => {
                let left = (minutes.max(0.1) * 60.0 - t).max(0.0);
                if left <= 0.0 {
                    // time's up: a slow pulse until you stop it
                    LiveFrame::Color(scale(*warn, 0.3 + 0.7 * (0.5 - 0.5 * (t * std::f32::consts::TAU / 1.5).cos())))
                } else {
                    let c = mix(*color, *warn, 1.0 - left / (minutes.max(0.1) * 60.0));
                    // the last ten seconds blink once a second
                    LiveFrame::Color(scale(c, if left <= 10.0 && left.fract() < 0.5 { 0.2 } else { 1.0 }))
                }
            }
            MusicFlow { colors } => {
                let l = input.loudness.unwrap_or(0.0);
                self.smooth_level += (l - self.smooth_level) * if l > self.smooth_level { 0.5 } else { 0.1 };
                // quiet music drifts, loud music races (the phase carries between frames)
                self.phase += (0.02 + 0.5 * self.smooth_level.clamp(0.0, 1.0)) * (t - self.last_t).clamp(0.0, 0.5);
                self.last_t = t;
                LiveFrame::Color(ramp(colors, self.phase))
            }
            CpuHeat { cool, hot } => LiveFrame::Color(mix(*cool, *hot, input.cpu.unwrap_or(0.0))),
            ScreenAmbient { saturation } => {
                let c = input.screen.unwrap_or(Rgb::BLACK);
                // boost saturation a little: averaged screens are greyish
                let avg = (c.0 as f32 + c.1 as f32 + c.2 as f32) / 3.0;
                let s = |v: u8| (avg + (v as f32 - avg) * saturation.max(0.0)).clamp(0.0, 255.0) as u8;
                LiveFrame::Color(Rgb(s(c.0), s(c.1), s(c.2)))
            }
            Spectrum { mirror, .. } => {
                let mirror = *mirror;
                let bands = input.bands.unwrap_or([0.0; BANDS]);
                for ((&v, s), peak) in bands.iter().zip(self.smooth_bands.iter_mut()).zip(self.peaks.iter_mut()) {
                    // fast attack, slow release, like a real VU meter
                    *s += (v - *s) * if v > *s { 0.7 } else { 0.18 };
                    *peak = f32::max(*peak - 0.02, *s);
                }
                let mut out = [0u8; BANDS];
                for (i, o) in out.iter_mut().enumerate() {
                    // mirrored: bass in the middle, highs at both edges
                    let src = if mirror { (i as f32 - (BANDS as f32 - 1.0) / 2.0).abs().floor() as usize * 2 } else { i };
                    *o = quantize(self.smooth_bands[src.min(BANDS - 1)]);
                }
                LiveFrame::Levels(out)
            }
            SineBars { speed, .. } => {
                let mut out = [0u8; BANDS];
                for (i, o) in out.iter_mut().enumerate() {
                    *o = quantize(0.5 + 0.5 * ((i as f32 * 0.4) - t * 3.0 * speed).sin());
                }
                LiveFrame::Levels(out)
            }
            RainBars { rate, .. } => {
                let mut out = [0u8; BANDS];
                for (i, o) in out.iter_mut().enumerate() {
                    // each band has its own drop phase
                    let phase = hash01(i as u32 * 31 + 5);
                    let x = (t * rate * (0.6 + hash01(i as u32) * 0.8) + phase).fract();
                    *o = quantize(1.0 - x);
                }
                LiveFrame::Levels(out)
            }
            Scanner { speed, .. } => {
                let pos = {
                    let p = (t * 0.5 * speed).fract() * 2.0;
                    if p > 1.0 {
                        2.0 - p
                    } else {
                        p
                    } // ping-pong
                } * (BANDS - 1) as f32;
                let mut out = [0u8; BANDS];
                for (i, o) in out.iter_mut().enumerate() {
                    *o = quantize(1.0 - ((i as f32 - pos).abs() / 3.0));
                }
                LiveFrame::Levels(out)
            }
            Equalizer { speed, .. } => {
                let mut out = [0u8; BANDS];
                for (i, o) in out.iter_mut().enumerate() {
                    let tilt = 1.0 - i as f32 / BANDS as f32 * 0.5; // bass heavier
                    *o = quantize(tilt * value_noise(t * 4.0 * speed + i as f32 * 3.7));
                }
                LiveFrame::Levels(out)
            }
            Trip { colors, speed } => {
                let s = t * speed;
                // incommensurate oscillators: the colour wanders, brightness breathes on two clocks
                let pos = s * 0.07 + 0.22 * (s * 0.31).sin() + 0.11 * (s * 0.83 + 2.0).sin();
                let glow = 0.5 + 0.5 * (s * 1.7).sin() * (s * 0.61 + 0.5).cos();
                LiveFrame::Color(scale(ramp(colors, pos), 0.4 + 0.6 * glow))
            }
            Lava { colors, speed } => {
                let s = t * speed;
                let pos = 0.7 * value_noise(s * 0.35) + 0.3 * value_noise(s * 0.9 + 50.0);
                let k = 0.65 + 0.35 * value_noise(s * 0.6 + 100.0);
                LiveFrame::Color(scale(ramp(colors, pos * 1.6), k))
            }
            Breathe { colors, bpm } => {
                let b = t * bpm.max(1.0) / 60.0;
                let c = colors.get(b.floor() as usize % colors.len().max(1)).copied().unwrap_or(Rgb::BLACK);
                let k = (b.fract() * std::f32::consts::PI).sin().powi(2);
                LiveFrame::Color(scale(c, 0.03 + 0.97 * k))
            }
            Rave { colors, bpm } => {
                let b = t * bpm.max(1.0) / 60.0;
                let pick = |n: u32| (hash01(n.wrapping_mul(2_654_435)) * colors.len() as f32) as usize;
                let n = b.floor() as u32;
                // never the same colour twice in a row
                let mut i = pick(n);
                if colors.len() > 1 && i == pick(n.wrapping_sub(1)) {
                    i = (i + 1) % colors.len();
                }
                let c = colors.get(i).copied().unwrap_or(Rgb::BLACK);
                LiveFrame::Color(scale(c, 0.08 + 0.92 * (-b.fract() * 4.0).exp()))
            }
            Morse { color, background, wpm, .. } => {
                if self.morse.is_empty() {
                    return LiveFrame::Color(*background);
                }
                let unit = 1.2 / wpm.clamp(2.0, 40.0); // PARIS standard
                let u = (t / unit).floor() as usize % self.morse.len();
                LiveFrame::Color(if self.morse[u] { *color } else { *background })
            }
            Bomb { color, blast, fuse } => {
                let fuse = fuse.max(5.0);
                let cycle = fuse + 4.0;
                let x = t.rem_euclid(cycle);
                let k_blast;
                let beep;
                if x < fuse - 1.0 {
                    // beep interval shrinks from 1 s to 0.12 s; phase is its integral
                    let (a, b) = (0.12f32, 0.88f32);
                    let interval = a + b * (1.0 - x / fuse);
                    let phase = -(fuse / b) * (interval / (a + b)).ln();
                    let on = (0.08 / interval).min(0.5);
                    beep = if phase.fract() < on { 1.0 } else { 0.0 };
                    k_blast = 0.0;
                } else if x < fuse {
                    beep = x - (fuse - 1.0); // the final rising tone
                    k_blast = 0.0;
                } else {
                    beep = 0.0;
                    let d = x - fuse;
                    k_blast = if d < 0.2 { 1.0 } else { (-(d - 0.2) * 1.4).exp() };
                }
                let base = mix(scale(*color, 0.12), *color, beep);
                LiveFrame::Color(mix(base, *blast, k_blast))
            }
            Flashbang { color, every } => {
                let slot_len = every.max(1.0);
                let slot = (t / slot_len).floor();
                let at = hash01(slot as u32 * 31 + 3) * slot_len * 0.6;
                let d = t - slot * slot_len - at;
                let k = if d < 0.0 {
                    0.0
                } else if d < 0.35 {
                    1.0
                } else {
                    (-(d - 0.35) * 0.9).exp()
                };
                LiveFrame::Color(mix(*color, Rgb(255, 255, 255), k))
            }
            Focus { work, rest, minutes, break_minutes } => {
                let (w, r) = (minutes.max(0.1) * 60.0, break_minutes.max(0.1) * 60.0);
                let x = t.rem_euclid(w + r);
                let (c, left) = if x < w { (*work, w - x) } else { (*rest, w + r - x) };
                // steady, with a slow pulse in the last minute of each phase
                let k = if left < 60.0 { 0.55 + 0.45 * (0.5 + 0.5 * (t * std::f32::consts::TAU / 2.0).cos()) } else { 0.85 };
                LiveFrame::Color(scale(c, k))
            }
            BeatFlash { colors } => {
                let l = input.loudness.unwrap_or(0.0);
                let since = self.last_beat.map(|b| t - b).unwrap_or(f32::MAX);
                // an onset: clearly louder than the recent average, and not right after the last one
                if l > self.beat_avg * 1.25 + 0.05 && since > 0.18 {
                    self.last_beat = Some(t);
                    self.beat_idx = self.beat_idx.wrapping_add(1);
                }
                // the average rises quickly (sustained loudness isn't a beat) and falls slowly
                self.beat_avg += (l - self.beat_avg) * if l > self.beat_avg { 0.25 } else { 0.1 };
                let since = self.last_beat.map(|b| t - b).unwrap_or(f32::MAX);
                let c = colors.get(self.beat_idx % colors.len().max(1)).copied().unwrap_or(Rgb::BLACK);
                LiveFrame::Color(scale(c, 0.06 + 0.94 * (-since * 5.0).exp()))
            }
            Siren { a, b, speed } => {
                let period = 1.0 / speed.max(0.1);
                let cyc = t.rem_euclid(period * 2.0);
                let (color, x) = if cyc < period { (*a, cyc / period) } else { (*b, (cyc - period) / period) };
                let flash = |c: f32, w: f32| x >= c && x < c + w;
                let on = flash(0.0, 0.18) || flash(0.28, 0.18);
                LiveFrame::Color(if on { color } else { Rgb::BLACK })
            }
            Glitter { base, spark, density } => {
                let rate = density.max(0.05) * 6.0;
                let slot_len = 1.0 / rate;
                let slot = (t / slot_len).floor();
                let within = t - slot * slot_len;
                let has_spark = hash01(slot as u32 * 977 + 13) < 0.5;
                let dur = 0.12; // three frames at the 25 fps live tick
                let k = if has_spark && within < dur { 1.0 - within / dur } else { 0.0 };
                LiveFrame::Color(mix(*base, *spark, k))
            }
            PacedBreathing { color, inhale, hold, exhale, rest } => {
                let (i, h, e, r) = (inhale.max(0.1), hold.max(0.0), exhale.max(0.1), rest.max(0.0));
                let x = t.rem_euclid((i + h + e + r).max(0.2));
                let smooth = |p: f32| p * p * (3.0 - 2.0 * p);
                let k = if x < i {
                    smooth(x / i)
                } else if x < i + h {
                    1.0
                } else if x < i + h + e {
                    1.0 - smooth((x - i - h) / e)
                } else {
                    0.0
                };
                LiveFrame::Color(scale(*color, 0.08 + 0.92 * k))
            }
            Steps { colors, hold } => {
                let n = colors.len().max(1);
                let i = (t / hold.max(0.1)).floor() as usize % n;
                LiveFrame::Color(colors.get(i).copied().unwrap_or(Rgb::BLACK))
            }
            Bars { style, speed, .. } => {
                let v = bars::render(*style, t, *speed);
                let mut out = [0u8; BANDS];
                for (o, x) in out.iter_mut().zip(v) {
                    *o = quantize(x);
                }
                LiveFrame::Levels(out)
            }
        }
    }
}

/// Stand-in inputs for previews: a 120 bpm beat, a CPU swing and a cycling screen colour.
pub fn demo_inputs(t: f32) -> Inputs {
    let beat = (t * 2.0).fract();
    let mut bands = [0.0; BANDS];
    for (i, b) in bands.iter_mut().enumerate() {
        let bass = (1.0 - i as f32 / BANDS as f32).powi(2);
        *b = ((1.0 - beat).powi(2) * bass + 0.35 * value_noise(t * 6.0 + i as f32 * 1.7)).min(1.0);
    }
    Inputs {
        bands: Some(bands),
        loudness: Some((1.0 - beat).powi(2)),
        cpu: Some(0.5 + 0.5 * (t * 0.8).sin()),
        screen: Some(ramp(&[Rgb(0, 80, 255), Rgb(255, 60, 0), Rgb(0, 200, 120)], t / 8.0)),
    }
}

/// `seconds` of frames at `fps`, rendered with [`demo_inputs`] (for UI previews). Time
/// is fast-forwarded per `LiveEffect::preview_speed` for effects with a real-world
/// duration far longer than the preview; the live effect itself is never sped up.
pub fn preview(effect: LiveEffect, seconds: f32, fps: f32) -> Vec<LiveFrame> {
    let speed = effect.preview_speed(seconds);
    let mut a = Animator::new(effect);
    let n = (seconds * fps).clamp(1.0, 600.0) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / fps;
            a.frame(t * speed, &demo_inputs(t))
        })
        .collect()
}

/// A curated set of live presets for the UI's "Live" tab.
pub fn presets() -> Vec<(&'static str, &'static str, LiveEffect)> {
    let h = |s: &str| -> Rgb { s.parse().expect("valid preset colour") };
    vec![
        (
            "Ocean Flow",
            "Endless drift through ocean blues",
            LiveEffect::PaletteFlow { colors: vec![h("#00fff0"), h("#0080ff"), h("#0020ff"), h("#00c8ff")], period: 10.0 },
        ),
        (
            "Aurora Flow",
            "Teal, blue and violet like the northern lights",
            LiveEffect::PaletteFlow { colors: vec![h("#00ffc8"), h("#0090ff"), h("#6a2cff")], period: 14.0 },
        ),
        (
            "Rainbow Flow",
            "Silky smooth full-spectrum cycle",
            LiveEffect::PaletteFlow {
                colors: vec![h("#ff0000"), h("#ffff00"), h("#00ff00"), h("#00ffff"), h("#0000ff"), h("#ff00ff")],
                period: 12.0,
            },
        ),
        ("Deep Sea Pulse", "Very slow heartbeat in abyssal blue", LiveEffect::Heartbeat { color: h("#0a2aff"), bpm: 42.0 }),
        ("Glacier Tide", "Ice white swelling into glacier blue", LiveEffect::Tide { a: h("#1060ff"), b: h("#d8fbff"), period: 9.0 }),
        ("Blue Lightning", "Midnight sky with electric-blue strikes", LiveEffect::Storm { sky: h("#030a3a"), flash: h("#00e5ff"), rate: 0.6 }),
        ("Sapphire Scanner", "A slow sapphire beam, back and forth", LiveEffect::Scanner { color: h("#1e50ff"), speed: 0.6 }),
        ("Blue Heartbeat", "Calm double pulse at 60 bpm", LiveEffect::Heartbeat { color: h("#0050ff"), bpm: 60.0 }),
        ("Blue Flame", "Gas-flame flicker", LiveEffect::Flicker { color: h("#1e6bff"), intensity: 0.45 }),
        ("Candle", "Warm candlelight flicker", LiveEffect::Flicker { color: h("#ff8a20"), intensity: 0.6 }),
        ("Tide", "Slow swell between deep blue and cyan", LiveEffect::Tide { a: h("#001aa0"), b: h("#00e5ff"), period: 7.0 }),
        ("Thunderstorm", "Dark sky with lightning strikes", LiveEffect::Storm { sky: h("#050a30"), flash: h("#e0f0ff"), rate: 0.8 }),
        ("Police", "Red / blue strobe", LiveEffect::Strobe { a: h("#ff0010"), b: h("#0030ff"), hz: 2.0 }),
        ("Music Glow", "Brightens and shifts colour with the music", LiveEffect::AudioGlow { quiet: h("#0020a0"), loud: h("#00f0ff") }),
        ("CPU Heat", "Blue when idle, red under load", LiveEffect::CpuHeat { cool: h("#0040ff"), hot: h("#ff2000") }),
        ("Screen Ambient", "Matches the colour of your screen", LiveEffect::ScreenAmbient { saturation: 1.4 }),
        ("Spectrum", "Real audio visualiser", LiveEffect::Spectrum { color: h("#00a8ff"), rainbow: false, mirror: false }),
        ("Rainbow Spectrum", "Audio visualiser in full colour", LiveEffect::Spectrum { color: h("#ff0000"), rainbow: true, mirror: false }),
        ("Mirror Spectrum", "Visualiser with the bass in the middle", LiveEffect::Spectrum { color: h("#0070ff"), rainbow: false, mirror: true }),
        ("Rainbow Mirror", "Mirrored visualiser in full colour", LiveEffect::Spectrum { color: h("#ff0000"), rainbow: true, mirror: true }),
        ("Sine Waves", "Rolling waves across the board", LiveEffect::SineBars { color: h("#0060ff"), speed: 1.0 }),
        ("Rainfall", "Drops falling down the keys", LiveEffect::RainBars { color: h("#00c8ff"), rate: 0.9 }),
        ("Scanner", "A sweeping beam, back and forth", LiveEffect::Scanner { color: h("#00e5ff"), speed: 1.0 }),
        ("Equalizer", "Lively fake EQ, no audio needed", LiveEffect::Equalizer { color: h("#2962ff"), speed: 1.0 }),
        // ---- Trippy colour trips ----------------------------------------------
        (
            "Acid Trip",
            "Colours melting into each other on out-of-sync clocks",
            LiveEffect::Trip { colors: vec![h("#ff00c8"), h("#00ffe0"), h("#fff200"), h("#7a00ff")], speed: 1.2 },
        ),
        (
            "Blue Dream",
            "A hypnotic drift through every shade of blue",
            LiveEffect::Trip { colors: vec![h("#00fff0"), h("#0060ff"), h("#6a2cff"), h("#00b0ff")], speed: 0.8 },
        ),
        (
            "Kaleido",
            "Prismatic, restless, never quite repeating",
            LiveEffect::Trip { colors: vec![h("#ff0000"), h("#ffff00"), h("#00ff00"), h("#00ffff"), h("#0000ff"), h("#ff00ff")], speed: 1.6 },
        ),
        (
            "Lava Lamp",
            "Slow molten blobs of orange and magenta",
            LiveEffect::Lava { colors: vec![h("#ff3000"), h("#ff9000"), h("#ff0070"), h("#a000ff")], speed: 1.0 },
        ),
        (
            "Blue Lava",
            "Slow blobs of cobalt and cyan",
            LiveEffect::Lava { colors: vec![h("#0010d0"), h("#00c8ff"), h("#2f55ff"), h("#00fff0")], speed: 1.0 },
        ),
        (
            "Deep Breath",
            "Ten slow breaths a minute, each a new blue",
            LiveEffect::Breathe { colors: vec![h("#0040ff"), h("#00c8ff"), h("#6a2cff"), h("#00fff0")], bpm: 10.0 },
        ),
        (
            "Rainbow Breath",
            "Breathing through the rainbow",
            LiveEffect::Breathe {
                colors: vec![h("#ff0000"), h("#ff8000"), h("#ffff00"), h("#00ff00"), h("#00ffff"), h("#0040ff"), h("#a000ff")],
                bpm: 14.0,
            },
        ),
        (
            "Rave",
            "Hard colour cuts at 128 bpm",
            LiveEffect::Rave { colors: vec![h("#ff00c8"), h("#00ffff"), h("#fff200"), h("#0040ff"), h("#00ff60")], bpm: 128.0 },
        ),
        (
            "Blue Rave",
            "Club-night blues at 128 bpm",
            LiveEffect::Rave { colors: vec![h("#0040ff"), h("#00ffff"), h("#6a2cff"), h("#ffffff")], bpm: 128.0 },
        ),
        (
            "Beat Flash",
            "Flashes a new colour on every beat of your music",
            LiveEffect::BeatFlash { colors: vec![h("#0040ff"), h("#00ffff"), h("#ff00c8"), h("#fff200")] },
        ),
        ("Blue Beat", "Flashes blue on every beat of your music", LiveEffect::BeatFlash { colors: vec![h("#0040ff"), h("#00c8ff"), h("#6a2cff")] }),
        ("SOS", "Morse distress call in red", LiveEffect::Morse { message: "SOS".into(), color: h("#ff1020"), background: h("#100000"), wpm: 12.0 }),
        // ---- Counter-Strike ---------------------------------------------------
        (
            "C4 Countdown",
            "The bomb has been planted: beeps speed up, then boom",
            LiveEffect::Bomb { color: h("#ff1a1a"), blast: h("#ffb040"), fuse: 40.0 },
        ),
        ("Flashbang", "Every so often, a white-out that slowly fades back to blue", LiveEffect::Flashbang { color: h("#0030c0"), every: 12.0 }),
        // ---- Bars -------------------------------------------------------------
        (
            "Plasma Bars",
            "Two interfering waves, endlessly morphing",
            LiveEffect::Bars { style: BarStyle::Plasma, color: h("#0060ff"), rainbow: false, speed: 1.0 },
        ),
        (
            "Rainbow Plasma",
            "Morphing plasma in full colour",
            LiveEffect::Bars { style: BarStyle::Plasma, color: h("#ff0000"), rainbow: true, speed: 1.2 },
        ),
        (
            "Bouncing Balls",
            "Three balls bouncing across the board",
            LiveEffect::Bars { style: BarStyle::Bounce, color: h("#00e5ff"), rainbow: false, speed: 1.0 },
        ),
        ("Bonfire", "Flames licking up from the bottom", LiveEffect::Bars { style: BarStyle::Fire, color: h("#ff5000"), rainbow: false, speed: 1.0 }),
        (
            "Blue Fire",
            "Gas-blue flames licking up the keys",
            LiveEffect::Bars { style: BarStyle::Fire, color: h("#1e6bff"), rainbow: false, speed: 1.0 },
        ),
        (
            "Fountain",
            "Ripples spreading from the centre",
            LiveEffect::Bars { style: BarStyle::Fountain, color: h("#00c8ff"), rainbow: false, speed: 1.0 },
        ),
        (
            "Rainbow Fountain",
            "Colourful ripples from the centre",
            LiveEffect::Bars { style: BarStyle::Fountain, color: h("#ff0000"), rainbow: true, speed: 1.0 },
        ),
        ("DNA Helix", "A twisting double helix", LiveEffect::Bars { style: BarStyle::Helix, color: h("#00ffc8"), rainbow: false, speed: 1.0 }),
        (
            "Heart Monitor",
            "A heartbeat trace scrolling across",
            LiveEffect::Bars { style: BarStyle::Ecg, color: h("#00ff60"), rainbow: false, speed: 1.0 },
        ),
        (
            "Stacker",
            "Blocks stack up until the board is full, then clear",
            LiveEffect::Bars { style: BarStyle::Stacker, color: h("#2962ff"), rainbow: true, speed: 1.0 },
        ),
        (
            "Glitch",
            "Calm, then sudden bursts of digital noise",
            LiveEffect::Bars { style: BarStyle::Glitch, color: h("#00ffff"), rainbow: false, speed: 1.0 },
        ),
        (
            "Kick Drum",
            "Everything thumps together at 120 bpm",
            LiveEffect::Bars { style: BarStyle::Pump, color: h("#ff00c8"), rainbow: false, speed: 1.0 },
        ),
        (
            "Fireflies",
            "Random sparks flare and fade",
            LiveEffect::Bars { style: BarStyle::Fireflies, color: h("#c8ff40"), rainbow: false, speed: 1.0 },
        ),
        (
            "Fireworks",
            "Shells rise and burst into falling sparks",
            LiveEffect::Bars { style: BarStyle::Fireworks, color: h("#ff0000"), rainbow: true, speed: 1.0 },
        ),
        (
            "Gold Fireworks",
            "A golden display, one shell after another",
            LiveEffect::Bars { style: BarStyle::Fireworks, color: h("#ffc040"), rainbow: false, speed: 0.9 },
        ),
        (
            "Comet",
            "A bright head sweeping past with a long tail",
            LiveEffect::Bars { style: BarStyle::Comet, color: h("#00e5ff"), rainbow: false, speed: 1.0 },
        ),
        ("Matrix Rain", "Columns of falling code", LiveEffect::Bars { style: BarStyle::Matrix, color: h("#00ff40"), rainbow: false, speed: 1.0 }),
        (
            "Snake Run",
            "A run of light sliding around the board",
            LiveEffect::Bars { style: BarStyle::Snake, color: h("#a0ff00"), rainbow: false, speed: 1.0 },
        ),
        (
            "VU Meter",
            "Bars rise with the beat and fall back",
            LiveEffect::Bars { style: BarStyle::Vu, color: h("#ff2a6d"), rainbow: false, speed: 1.0 },
        ),
        (
            "Ripples",
            "Drops landing, their rings spreading out",
            LiveEffect::Bars { style: BarStyle::Ripple, color: h("#40c8ff"), rainbow: false, speed: 1.0 },
        ),
        (
            "Pendulum Swing",
            "Bars swing like the pendulum-wave toy, in and out of step",
            LiveEffect::Bars { style: BarStyle::Pendulum, color: h("#00e0ff"), rainbow: false, speed: 1.0 },
        ),
        (
            "Chain Lightning",
            "Dark skies, then a strike branches across the bars and fades",
            LiveEffect::Bars { style: BarStyle::Lightning, color: h("#c8e8ff"), rainbow: false, speed: 1.0 },
        ),
        (
            "Board Rally",
            "A ball volleys between two paddles, each tracking the return",
            LiveEffect::Bars { style: BarStyle::Rally, color: h("#40ff80"), rainbow: false, speed: 1.0 },
        ),
        (
            "Falling Sands",
            "Grains piling up from one end, then draining away",
            LiveEffect::Bars { style: BarStyle::Hourglass, color: h("#ffb000"), rainbow: false, speed: 1.0 },
        ),
        (
            "Loading Bar",
            "A progress fill sweeps across with a bright leading edge",
            LiveEffect::Bars { style: BarStyle::Loading, color: h("#00ffa0"), rainbow: false, speed: 1.0 },
        ),
        (
            "Head-On Collision",
            "Two pulses run in from the ends and burst apart in the middle",
            LiveEffect::Bars { style: BarStyle::Collide, color: h("#ff5030"), rainbow: false, speed: 1.0 },
        ),
        // ---- Slow, ambient and useful -----------------------------------------
        (
            "Northern Curtain",
            "Green and violet curtains drifting overhead",
            LiveEffect::Aurora { colors: vec![h("#00ff9c"), h("#00c8ff"), h("#7a30ff"), h("#00ffd0")], speed: 1.0 },
        ),
        (
            "Solar Aurora",
            "Gold and rose curtains, slow as weather",
            LiveEffect::Aurora { colors: vec![h("#ffd060"), h("#ff5a8a"), h("#a040ff")], speed: 0.8 },
        ),
        ("Day Cycle", "Night, dawn, daylight and dusk, every ten minutes", LiveEffect::Sunrise { minutes: 10.0 }),
        ("Quick Day", "The same day in four minutes", LiveEffect::Sunrise { minutes: 4.0 }),
        (
            "Five Minutes",
            "A five-minute timer: blue to red, blinking at the end",
            LiveEffect::Countdown { color: h("#0060ff"), warn: h("#ff1020"), minutes: 5.0 },
        ),
        (
            "Twenty Minutes",
            "A twenty-minute timer for a focused stretch",
            LiveEffect::Countdown { color: h("#00ff90"), warn: h("#ff6a00"), minutes: 20.0 },
        ),
        (
            "Music Drift",
            "Drifts through blues, faster the louder the music",
            LiveEffect::MusicFlow { colors: vec![h("#0040ff"), h("#00c8ff"), h("#6a2cff"), h("#00fff0")] },
        ),
        (
            "Rainbow Drift",
            "The whole spectrum, driven by your music",
            LiveEffect::MusicFlow { colors: vec![h("#ff0000"), h("#ffff00"), h("#00ff00"), h("#00ffff"), h("#0040ff"), h("#ff00ff")] },
        ),
        (
            "Blue Fireflies",
            "Blue sparks flare and fade",
            LiveEffect::Bars { style: BarStyle::Fireflies, color: h("#00c8ff"), rainbow: false, speed: 1.0 },
        ),
        ("Terrain", "A mountain range scrolling past", LiveEffect::Bars { style: BarStyle::Terrain, color: h("#40e040"), rainbow: true, speed: 1.0 }),
        // ---- more colour --------------------------------------------------------
        (
            "Sunset Flow",
            "Gold, coral and violet drifting like a sunset",
            LiveEffect::PaletteFlow { colors: vec![h("#ffd000"), h("#ff7a00"), h("#ff2a6d"), h("#7a00ff")], period: 12.0 },
        ),
        (
            "Forest Breath",
            "Breathing through spring greens",
            LiveEffect::Breathe { colors: vec![h("#40ff60"), h("#00c878"), h("#a0ff00"), h("#00ffb0")], bpm: 10.0 },
        ),
        (
            "Candy Rave",
            "Pink, lemon, mint and grape at 128 bpm",
            LiveEffect::Rave { colors: vec![h("#ff3fc8"), h("#ffe600"), h("#40ffb0"), h("#b000ff")], bpm: 128.0 },
        ),
        (
            "Ember Trip",
            "Reds and oranges smouldering on out-of-sync clocks",
            LiveEffect::Trip { colors: vec![h("#ff1010"), h("#ff7a00"), h("#ffb000"), h("#ff0040")], speed: 1.0 },
        ),
        (
            "Rainbow Helix",
            "A twisting double helix in every colour",
            LiveEffect::Bars { style: BarStyle::Helix, color: h("#ff0000"), rainbow: true, speed: 1.0 },
        ),
        ("Magenta Pulse", "A hot-pink heartbeat", LiveEffect::Heartbeat { color: h("#ff00c8"), bpm: 66.0 }),
        (
            "Aurora Borealis",
            "Green curtains shifting into violet",
            LiveEffect::Lava { colors: vec![h("#40ff80"), h("#00ffc8"), h("#00a0ff"), h("#8040ff")], speed: 0.8 },
        ),
        (
            "Party Beat",
            "A new bright colour on every beat of your music",
            LiveEffect::BeatFlash { colors: vec![h("#ff1010"), h("#ffe600"), h("#00ff60"), h("#00e5ff"), h("#b000ff")] },
        ),
        // ---- Siren, sparkle, breathing and steps -------------------------------
        ("Siren", "Red and blue in a police-style double flash", LiveEffect::Siren { a: h("#ff0010"), b: h("#0030ff"), speed: 1.0 }),
        ("Hazard Lights", "Amber double-flash against the dark", LiveEffect::Siren { a: h("#ff9900"), b: h("#100800"), speed: 0.8 }),
        ("Glitter", "White sparkles over a deep blue base", LiveEffect::Glitter { base: h("#0a1050"), spark: h("#ffffff"), density: 0.6 }),
        ("Frost Sparkle", "Icy sparkles over a frozen blue base", LiveEffect::Glitter { base: h("#08182e"), spark: h("#b8f0ff"), density: 0.5 }),
        (
            "Box Breathing",
            "A guided 4-4-4-4 breath: in, hold, out, hold",
            LiveEffect::PacedBreathing { color: h("#00c8a0"), inhale: 4.0, hold: 4.0, exhale: 4.0, rest: 4.0 },
        ),
        (
            "Deep Calm",
            "A slow 4-7-8 breath, the long exhale that settles you",
            LiveEffect::PacedBreathing { color: h("#8a7bff"), inhale: 4.0, hold: 7.0, exhale: 8.0, rest: 0.0 },
        ),
        (
            "Signal Cycle",
            "Hard cuts between green, amber and red, like a traffic signal",
            LiveEffect::Steps { colors: vec![h("#00ff40"), h("#ffb000"), h("#ff1020")], hold: 2.0 },
        ),
        // ---- CS2 ---------------------------------------------------------------------
        ("Blue Defuse", "CT side: the C4 countdown in blue", LiveEffect::Bomb { color: h("#1f45ff"), blast: h("#bff4ff"), fuse: 40.0 }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_validate_and_nonsense_does_not() {
        for (name, _, fx) in presets() {
            assert_eq!(fx.validate(), Ok(()), "{name}");
        }
        let blue = Rgb(0, 0, 255);
        for bad in [
            LiveEffect::PaletteFlow { colors: vec![], period: 8.0 },
            LiveEffect::Trip { colors: vec![blue; MAX_COLORS + 1], speed: 1.0 },
            LiveEffect::Strobe { a: blue, b: blue, hz: f32::INFINITY },
            LiveEffect::Heartbeat { color: blue, bpm: f32::NAN },
            LiveEffect::Morse { message: "S".repeat(MAX_MESSAGE + 1), color: blue, background: blue, wpm: 10.0 },
            LiveEffect::Morse { message: "SOS".into(), color: blue, background: blue, wpm: 0.0 },
            LiveEffect::Countdown { color: blue, warn: blue, minutes: -1.0 },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }

    fn color(f: LiveFrame) -> Rgb {
        match f {
            LiveFrame::Color(c) => c,
            _ => panic!("expected colour"),
        }
    }
    fn levels(f: LiveFrame) -> [u8; BANDS] {
        match f {
            LiveFrame::Levels(l) => l,
            _ => panic!("expected levels"),
        }
    }

    #[test]
    fn every_preset_renders_valid_frames_on_the_right_channel() {
        for (name, _, fx) in presets() {
            let base = fx.base_effect();
            base.validate().unwrap();
            let mut a = Animator::new(fx);
            let inputs = Inputs { bands: Some([0.5; BANDS]), loudness: Some(0.5), cpu: Some(0.5), screen: Some(Rgb(10, 20, 200)) };
            for i in 0..200 {
                let f = a.frame(i as f32 * 0.05, &inputs);
                match (base.mode, f) {
                    (Mode::ScreenSync, LiveFrame::Color(_)) => {}
                    (Mode::MusicBars, LiveFrame::Levels(l)) => assert!(l.iter().all(|v| *v <= MAX_LEVEL), "{name}"),
                    other => panic!("{name}: frame {other:?} doesn't match base mode"),
                }
            }
        }
    }

    #[test]
    fn palette_flow_is_continuous_and_loops() {
        let mut a = Animator::new(LiveEffect::PaletteFlow { colors: vec![Rgb(0, 0, 255), Rgb(0, 255, 255)], period: 4.0 });
        let start = color(a.frame(0.0, &Inputs::default()));
        assert_eq!(start, Rgb(0, 0, 255));
        assert_eq!(color(a.frame(4.0, &Inputs::default())), start, "loops after one period");
        // no big jumps between 20 fps frames
        let mut prev = start;
        for i in 1..=80 {
            let c = color(a.frame(i as f32 * 0.05, &Inputs::default()));
            assert!((c.1 as i32 - prev.1 as i32).abs() <= 25, "jump at frame {i}");
            prev = c;
        }
    }

    #[test]
    fn heartbeat_has_two_peaks_per_beat() {
        let mut a = Animator::new(LiveEffect::Heartbeat { color: Rgb(0, 0, 255), bpm: 60.0 });
        let v: Vec<u8> = (0..100).map(|i| color(a.frame(i as f32 * 0.01, &Inputs::default())).2).collect();
        let peaks = (1..99).filter(|&i| v[i] > v[i - 1] && v[i] >= v[i + 1] && v[i] > 120).count();
        assert_eq!(peaks, 2, "{v:?}");
    }

    #[test]
    fn spectrum_follows_input_with_smoothing() {
        let mut a = Animator::new(LiveEffect::Spectrum { color: Rgb(0, 0, 255), rainbow: false, mirror: false });
        let mut loud = [0.0; BANDS];
        loud[3] = 1.0;
        let first = levels(a.frame(0.0, &Inputs { bands: Some(loud), ..Default::default() }));
        assert!(first[3] >= 4, "fast attack: {first:?}");
        assert_eq!(first[20], 0);
        let mut last = first[3];
        for i in 1..6 {
            let l = levels(a.frame(i as f32 * 0.05, &Inputs { bands: Some([0.0; BANDS]), ..Default::default() }))[3];
            assert!(l <= last, "slow release decays monotonically");
            last = l;
        }
        assert!(last < first[3]);
    }

    #[test]
    fn cpu_heat_maps_load_to_colour() {
        let mut a = Animator::new(LiveEffect::CpuHeat { cool: Rgb(0, 0, 255), hot: Rgb(255, 0, 0) });
        assert_eq!(color(a.frame(0.0, &Inputs { cpu: Some(0.0), ..Default::default() })), Rgb(0, 0, 255));
        assert_eq!(color(a.frame(0.0, &Inputs { cpu: Some(1.0), ..Default::default() })), Rgb(255, 0, 0));
    }

    #[test]
    fn storm_is_mostly_dark_with_flashes() {
        let mut a = Animator::new(LiveEffect::Storm { sky: Rgb(5, 10, 48), flash: Rgb(224, 240, 255), rate: 1.0 });
        let frames: Vec<Rgb> = (0..2000).map(|i| color(a.frame(i as f32 * 0.02, &Inputs::default()))).collect();
        let bright = frames.iter().filter(|c| c.0 > 150).count();
        assert!(bright > 0 && bright < frames.len() / 10, "{bright} bright frames");
    }

    #[test]
    fn morse_blinks_the_message() {
        let (on, off) = (Rgb(0, 200, 255), Rgb(2, 10, 58));
        let mut a = Animator::new(LiveEffect::Morse { message: "NET".into(), color: on, background: off, wpm: 10.0 });
        let unit = 1.2 / 10.0;
        // sample the middle of each unit: N = "-." -> on on on, off, on
        let got: Vec<bool> = (0..5).map(|u| color(a.frame((u as f32 + 0.5) * unit, &Inputs::default())) == on).collect();
        assert_eq!(got, [true, true, true, false, true]);
        // N(5) gap(3) E(1) gap(3) T(3) end(7) = 22 units, then it repeats
        assert_eq!(color(a.frame(22.5 * unit, &Inputs::default())), on);
    }

    #[test]
    fn bomb_beeps_speed_up_then_blast() {
        let mut a = Animator::new(LiveEffect::Bomb { color: Rgb(255, 0, 0), blast: Rgb(255, 255, 255), fuse: 40.0 });
        let beeps = |a: &mut Animator, from: f32, to: f32| {
            let mut n = 0;
            let mut prev = false;
            let mut t = from;
            while t < to {
                let on = color(a.frame(t, &Inputs::default())).0 > 200;
                if on && !prev {
                    n += 1;
                }
                prev = on;
                t += 0.01;
            }
            n
        };
        let early = beeps(&mut a, 0.0, 5.0);
        let late = beeps(&mut a, 33.0, 38.0);
        assert!((4..=6).contains(&early), "early {early}");
        assert!(late > early * 3, "late {late} vs early {early}");
        assert_eq!(color(a.frame(40.1, &Inputs::default())), Rgb(255, 255, 255), "blast");
        assert!(color(a.frame(43.9, &Inputs::default())).1 < 10, "fades out before the next round");
    }

    #[test]
    fn beat_flash_steps_colour_on_onsets() {
        let (c1, c2) = (Rgb(0, 0, 255), Rgb(0, 255, 0));
        let mut a = Animator::new(LiveEffect::BeatFlash { colors: vec![c1, c2] });
        let quiet = Inputs { loudness: Some(0.05), ..Default::default() };
        let loud = Inputs { loudness: Some(0.9), ..Default::default() };
        for i in 0..20 {
            a.frame(i as f32 * 0.04, &quiet);
        }
        let hit = color(a.frame(0.8, &loud));
        assert_eq!(hit, c2, "first beat moves to the next colour at full brightness");
        let later = color(a.frame(1.3, &quiet));
        assert!(later.1 < 40, "decays: {later:?}");
        // sustained loudness is not a new beat
        for i in 0..10 {
            a.frame(1.4 + i as f32 * 0.04, &loud);
        }
        assert_eq!(a.beat_idx, 2, "one more onset when it got loud again, not one per frame");
    }

    #[test]
    fn focus_timer_switches_to_break() {
        let (w, r) = (Rgb(0, 0, 255), Rgb(0, 255, 0));
        let mut a = Animator::new(LiveEffect::Focus { work: w, rest: r, minutes: 25.0, break_minutes: 5.0 });
        assert!(color(a.frame(60.0, &Inputs::default())).2 > 200);
        assert!(color(a.frame(26.0 * 60.0, &Inputs::default())).1 > 150);
        assert!(color(a.frame(31.0 * 60.0, &Inputs::default())).2 > 150, "loops back to work");
    }

    #[test]
    fn mirror_spectrum_is_symmetric() {
        let mut a = Animator::new(LiveEffect::Spectrum { color: Rgb(0, 0, 255), rainbow: false, mirror: true });
        let mut b = [0.0; BANDS];
        b[0] = 1.0; // bass
        let l = levels(a.frame(0.0, &Inputs { bands: Some(b), ..Default::default() }));
        assert!(l[15] > 0 && l[16] > 0, "bass in the middle: {l:?}");
        assert_eq!(l[0], 0);
        for i in 0..BANDS {
            assert_eq!(l[i], l[BANDS - 1 - i]);
        }
    }

    #[test]
    fn previews_move_and_serialize_for_the_ui() {
        for (name, _, fx) in presets() {
            let frames = preview(fx, 6.0, 20.0);
            assert_eq!(frames.len(), 120, "{name}");
            assert!(frames.iter().any(|f| *f != frames[0]), "{name}: preview never changes");
        }
        let json = serde_json::to_string(&preview(LiveEffect::Heartbeat { color: Rgb(0, 0, 255), bpm: 60.0 }, 0.1, 10.0)).unwrap();
        assert!(json.starts_with("[{\"color\":\"#"), "{json}");
    }

    #[test]
    fn effects_serialize_for_profiles() {
        for (_, _, fx) in presets() {
            let s = serde_json::to_string(&fx).unwrap();
            assert_eq!(serde_json::from_str::<LiveEffect>(&s).unwrap(), fx);
        }
    }
}
