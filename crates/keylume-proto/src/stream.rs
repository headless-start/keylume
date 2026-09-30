//! Host-streamed lighting: the firmware renders frames the PC sends in real time.
//!
//! - In `Mode::MusicBars` / `Mode::MusicPulse` the keyboard draws 32 "bands" of
//!   levels `0..=MAX_LEVEL` (the vendor app feeds an audio FFT; we can feed anything).
//! - Frames are RAM-only: unlike config writes they need no settle time, so they can
//!   be sent as fast as the previous one completes.

use crate::packet::{ck7, Packet, REPORT_LEN};

pub const BANDS: usize = 32;
pub const MAX_LEVEL: u8 = 6;
pub const AUDIO: u8 = 0x0D;
pub const SCREEN: u8 = 0x0E;

/// One frame of band levels. Values above `MAX_LEVEL` are clamped.
pub fn audio_frame(levels: &[u8; BANDS]) -> Packet {
    let mut p = [0u8; REPORT_LEN];
    p[0] = AUDIO;
    for (i, l) in levels.iter().enumerate() {
        p[8 + i] = (*l).min(MAX_LEVEL);
    }
    ck7(&mut p);
    p
}

/// Screen-sync frame (`Mode::ScreenSync`): one colour for the whole board.
/// The vendor app sends the 1×1-pixel average of the screen as RGBA every 40 ms.
pub fn color_frame(c: crate::Rgb) -> Packet {
    let mut p = [0u8; REPORT_LEN];
    p[..5].copy_from_slice(&[SCREEN, c.0, c.1, c.2, 0xFF]);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_frame_layout() {
        let mut lv = [0u8; BANDS];
        lv[0] = 3;
        lv[31] = 99;
        let p = audio_frame(&lv);
        assert_eq!(&p[..8], &[0x0D, 0, 0, 0, 0, 0, 0, 0xF2]); // vendor: n[7] = 255 - 0x0D
        assert_eq!(p[8], 3);
        assert_eq!(p[39], MAX_LEVEL);
        assert_eq!(p[40], 0);
    }

    #[test]
    fn color_frame_layout() {
        let p = color_frame(crate::Rgb(1, 2, 3));
        assert_eq!(&p[..6], &[0x0E, 1, 2, 3, 0xFF, 0]);
    }
}
