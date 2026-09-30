//! Per-key colour frames stored in the keyboard's "user picture" layers (0x0C / 0x8C).
//!
//! A frame is 128 matrix slots × RGB = 384 bytes, slot order = key matrix order.
//! Write: select the layer with an LED `UserPicture` effect, then 7 pages.
//! Read: pages 0..6 of `8C 00 <page>`, each reply is 64 raw bytes.

use serde::{Deserialize, Serialize};

use crate::packet::{cmd, paged_write, read_request, Packet};
use crate::Rgb;

pub const SLOTS: usize = 128;
pub const FRAME_BYTES: usize = SLOTS * 3;
pub const WRITE_PAGES: usize = 7;
pub const READ_PAGES: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame(pub Vec<Rgb>);

impl Default for Frame {
    fn default() -> Self {
        Frame(vec![Rgb::BLACK; SLOTS])
    }
}

impl Frame {
    pub fn filled(c: Rgb) -> Self {
        Frame(vec![c; SLOTS])
    }

    pub fn set(&mut self, slot: usize, c: Rgb) {
        if let Some(s) = self.0.get_mut(slot) {
            *s = c;
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(FRAME_BYTES);
        for i in 0..SLOTS {
            let c = self.0.get(i).copied().unwrap_or_default();
            out.extend_from_slice(&[c.0, c.1, c.2]);
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Frame(
            (0..SLOTS)
                .map(|i| match bytes.get(i * 3..i * 3 + 3) {
                    Some(b) => Rgb(b[0], b[1], b[2]),
                    None => Rgb::BLACK,
                })
                .collect(),
        )
    }

    /// The 7 packets that store this frame into the currently selected layer.
    pub fn write_packets(&self) -> Vec<Packet> {
        paged_write(cmd::USER_PICTURE, 0, FRAME_BYTES as u16, WRITE_PAGES, &self.to_bytes())
    }

    pub fn read_requests() -> Vec<Packet> {
        (0..READ_PAGES as u8).map(|p| read_request(cmd::USER_PICTURE, 0, p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_matches_capture() {
        // Captured page 0 of "Deep Ocean": header, then slot0 black, Esc #00f7f2, Tab #00beff
        let mut f = Frame::default();
        f.set(1, Rgb(0x00, 0xF7, 0xF2));
        f.set(2, Rgb(0x00, 0xBE, 0xFF));
        let pages = f.write_packets();
        assert_eq!(pages.len(), 7);
        assert_eq!(&pages[0][..5], &[0x0C, 0x00, 0x80, 0x01, 0x00]);
        assert_eq!(&pages[0][8..17], &[0, 0, 0, 0x00, 0xF7, 0xF2, 0x00, 0xBE, 0xFF]);
        assert_eq!(pages[6][4], 6);
    }

    #[test]
    fn bytes_round_trip() {
        let mut f = Frame::default();
        for i in 0..SLOTS {
            f.set(i, Rgb(i as u8, 255 - i as u8, 7));
        }
        assert_eq!(Frame::from_bytes(&f.to_bytes()), f);
        // pages concatenated back = frame bytes
        let data: Vec<u8> = f.write_packets().iter().flat_map(|p| p[8..].to_vec()).collect();
        assert_eq!(&data[..FRAME_BYTES], &f.to_bytes()[..]);
    }
}
