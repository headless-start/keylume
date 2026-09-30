//! Audio → 32 band levels, for the spectrum visualiser and audio-reactive effects.
//!
//! Feed interleaved-to-mono samples with [`Analyzer::push`]; call [`Analyzer::analyze`]
//! once per frame. Bands are log-spaced from 40 Hz to 16 kHz (how we hear pitch), in
//! decibels, with automatic gain so quiet and loud music both fill the board.

use std::sync::Arc;

use keylume_proto::stream::BANDS;
use rustfft::{num_complex::Complex, Fft, FftPlanner};

pub const FFT_SIZE: usize = 2048;
const MIN_HZ: f32 = 40.0;
const MAX_HZ: f32 = 16_000.0;
/// Dynamic range shown on the bars.
const RANGE_DB: f32 = 42.0;

pub struct Analyzer {
    sample_rate: f32,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    ring: Vec<f32>,
    pos: usize,
    /// bin ranges per band
    edges: Vec<(usize, usize)>,
    /// automatic gain: slowly-decaying loudest level seen (dB)
    ceiling_db: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Analysis {
    pub bands: [f32; BANDS],
    pub loudness: f32,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        let window = (0..FFT_SIZE).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT_SIZE - 1) as f32).cos()).collect();
        let bin_hz = sr / FFT_SIZE as f32;
        let max_hz = MAX_HZ.min(sr / 2.0);
        let edges = (0..BANDS)
            .map(|b| {
                let f0 = MIN_HZ * (max_hz / MIN_HZ).powf(b as f32 / BANDS as f32);
                let f1 = MIN_HZ * (max_hz / MIN_HZ).powf((b + 1) as f32 / BANDS as f32);
                let lo = (f0 / bin_hz).floor() as usize;
                let hi = ((f1 / bin_hz).ceil() as usize).max(lo + 1);
                (lo.max(1), hi.min(FFT_SIZE / 2))
            })
            .collect();
        Analyzer {
            sample_rate: sr,
            fft: FftPlanner::new().plan_fft_forward(FFT_SIZE),
            window,
            ring: vec![0.0; FFT_SIZE],
            pos: 0,
            edges,
            ceiling_db: -30.0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Append mono samples (-1..1).
    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            self.ring[self.pos] = s;
            self.pos = (self.pos + 1) % FFT_SIZE;
        }
    }

    /// Analyse the most recent `FFT_SIZE` samples.
    pub fn analyze(&mut self) -> Analysis {
        let mut buf: Vec<Complex<f32>> = (0..FFT_SIZE).map(|i| Complex::new(self.ring[(self.pos + i) % FFT_SIZE] * self.window[i], 0.0)).collect();
        self.fft.process(&mut buf);
        let norm = 2.0 / FFT_SIZE as f32;
        let mut db = [0f32; BANDS];
        let mut peak = f32::MIN;
        for (b, &(lo, hi)) in self.edges.iter().enumerate() {
            let p = buf[lo..hi].iter().map(|c| (c.norm() * norm).powi(2)).fold(0.0f32, f32::max);
            db[b] = 10.0 * (p + 1e-12).log10();
            peak = peak.max(db[b]);
        }
        // AGC: jump up to new peaks, decay slowly; never amplify silence into noise.
        self.ceiling_db = if peak > self.ceiling_db { peak } else { self.ceiling_db - 0.15 };
        self.ceiling_db = self.ceiling_db.max(-60.0);
        let floor = self.ceiling_db - RANGE_DB;
        let mut bands = [0f32; BANDS];
        for b in 0..BANDS {
            bands[b] = ((db[b] - floor) / RANGE_DB).clamp(0.0, 1.0);
        }
        let rms = (self.ring.iter().map(|s| s * s).sum::<f32>() / FFT_SIZE as f32).sqrt();
        let loudness = ((20.0 * (rms + 1e-9).log10() + 50.0) / 44.0).clamp(0.0, 1.0);
        Analysis { bands, loudness }
    }

    /// Which band a frequency falls into (for tests and UI labels).
    pub fn band_of(&self, hz: f32) -> Option<usize> {
        let bin = (hz / (self.sample_rate / FFT_SIZE as f32)).round() as usize;
        self.edges.iter().position(|&(lo, hi)| bin >= lo && bin < hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, sr: f32, n: usize, amp: f32) -> Vec<f32> {
        (0..n).map(|i| amp * (std::f32::consts::TAU * hz * i as f32 / sr).sin()).collect()
    }

    #[test]
    fn pure_tone_lights_its_own_band() {
        for hz in [100.0, 1000.0, 5000.0] {
            let mut a = Analyzer::new(48_000);
            a.push(&tone(hz, 48_000.0, FFT_SIZE, 0.5));
            let r = a.analyze();
            let expected = a.band_of(hz).unwrap();
            let loudest = (0..BANDS).max_by(|&x, &y| r.bands[x].total_cmp(&r.bands[y])).unwrap();
            assert!((loudest as i32 - expected as i32).abs() <= 1, "{hz} Hz: loudest band {loudest}, expected {expected}");
            assert!(r.bands[expected] > 0.8);
            // far-away bands stay dark
            assert!(r.bands[(expected + 12) % BANDS] < 0.5);
        }
    }

    #[test]
    fn silence_is_dark_and_quiet() {
        let mut a = Analyzer::new(44_100);
        a.push(&vec![0.0; FFT_SIZE]);
        let r = a.analyze();
        assert!(r.bands.iter().all(|b| *b < 0.05), "{:?}", r.bands);
        assert!(r.loudness < 0.05);
    }

    #[test]
    fn loudness_tracks_volume() {
        let mut a = Analyzer::new(48_000);
        a.push(&tone(440.0, 48_000.0, FFT_SIZE, 0.05));
        let quiet = a.analyze().loudness;
        a.push(&tone(440.0, 48_000.0, FFT_SIZE, 0.8));
        let loud = a.analyze().loudness;
        assert!(loud > quiet + 0.3, "quiet {quiet} loud {loud}");
    }

    #[test]
    fn bands_cover_the_spectrum_in_order() {
        let a = Analyzer::new(48_000);
        let mut last = 0;
        for hz in [50.0, 200.0, 800.0, 3000.0, 12_000.0] {
            let b = a.band_of(hz).unwrap();
            assert!(b >= last);
            last = b;
        }
        assert_eq!(a.band_of(45.0), Some(0));
    }
}
