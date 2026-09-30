//! Macros (0x0B / 0x8B). 256-byte buffer: `u16 repeat`, events, zero terminator.
//!
//! Event = `code, flags` (+ `u16 delay` when the delay doesn't fit in 7 bits).
//! `flags` bit 7 = key down; low 7 bits = delay in ms when 1..=127.

use serde::{Deserialize, Serialize};

use crate::packet::{cmd, paged_write, read_request, Packet};
use crate::ProtoError;

pub const MACRO_BYTES: usize = 256;
pub const MACRO_SLOTS: u8 = 16;
const WRITE_PAGES: usize = 5;
const READ_PAGES: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MacroEvent {
    /// HID keyboard usage (0x04..=0xEF) or mouse button (0xF0..).
    Key {
        code: u8,
        down: bool,
    },
    Delay {
        ms: u16,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Macro {
    pub repeat: u16,
    pub events: Vec<MacroEvent>,
}

impl Macro {
    pub fn to_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        let mut out = Vec::with_capacity(MACRO_BYTES);
        out.extend_from_slice(&self.repeat.to_le_bytes());
        let ev = &self.events;
        let mut i = 0;
        while i < ev.len() {
            match ev[i] {
                MacroEvent::Key { code, down } => {
                    // total delay following this key (merge consecutive delays)
                    let mut delay: u32 = 0;
                    let mut j = i + 1;
                    while let Some(MacroEvent::Delay { ms }) = ev.get(j) {
                        delay += *ms as u32;
                        j += 1;
                    }
                    let delay = delay.min(u16::MAX as u32) as u16;
                    let down_bit = if down { 0x80 } else { 0 };
                    out.push(code);
                    if (1..=127).contains(&delay) {
                        out.push(down_bit | delay as u8);
                    } else {
                        out.push(down_bit);
                        out.extend_from_slice(&delay.to_le_bytes());
                    }
                    i = j;
                }
                // A delay before any key has nothing to attach to; skip it.
                MacroEvent::Delay { .. } => i += 1,
            }
        }
        if out.len() + 4 > MACRO_BYTES {
            return Err(ProtoError::MacroTooLong(out.len() + 4, MACRO_BYTES));
        }
        out.resize(MACRO_BYTES, 0);
        Ok(out)
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        let repeat = u16::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]);
        let mut events = Vec::new();
        let mut a = 2;
        while a + 1 < b.len() {
            let (code, flags) = (b[a], b[a + 1]);
            if code == 0 && flags == 0 && b.get(a + 2).copied().unwrap_or(0) == 0 && b.get(a + 3).copied().unwrap_or(0) == 0 {
                break;
            }
            events.push(MacroEvent::Key { code, down: flags & 0x80 != 0 });
            let delay = if flags & 0x7F != 0 {
                a += 2;
                (flags & 0x7F) as u16
            } else {
                let d = u16::from_le_bytes([b.get(a + 2).copied().unwrap_or(0), b.get(a + 3).copied().unwrap_or(0)]);
                a += 4;
                d
            };
            if delay > 0 {
                events.push(MacroEvent::Delay { ms: delay });
            }
        }
        Macro { repeat, events }
    }

    pub fn write_packets(&self, index: u8) -> Result<Vec<Packet>, ProtoError> {
        Ok(paged_write(cmd::MACRO, index, MACRO_BYTES as u16, WRITE_PAGES, &self.to_bytes()?))
    }

    pub fn read_requests(index: u8) -> Vec<Packet> {
        (0..READ_PAGES).map(|p| read_request(cmd::MACRO, index, p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MacroEvent::*;

    fn hi() -> Macro {
        Macro {
            repeat: 1,
            events: vec![
                Key { code: 11, down: true },
                Delay { ms: 30 },
                Key { code: 11, down: false },
                Delay { ms: 500 },
                Key { code: 12, down: true },
                Delay { ms: 20 },
                Key { code: 12, down: false },
            ],
        }
    }

    #[test]
    fn encodes_like_vendor_app() {
        // Captured: 01 00 | 0b 9e | 0b 00 f4 01 | 0c 94 | 0c 00 00 00
        let b = hi().to_bytes().unwrap();
        assert_eq!(&b[..16], &[0x01, 0x00, 0x0B, 0x9E, 0x0B, 0x00, 0xF4, 0x01, 0x0C, 0x94, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let p = hi().write_packets(0).unwrap();
        assert_eq!(&p[0][..5], &[0x0B, 0x00, 0x00, 0x01, 0x00]);
    }

    #[test]
    fn round_trips() {
        let m = hi();
        assert_eq!(Macro::from_bytes(&m.to_bytes().unwrap()), m);
    }

    #[test]
    fn rejects_overflow() {
        let events = (0..200).flat_map(|_| [Key { code: 4, down: true }, Delay { ms: 1000 }]).collect();
        assert!(Macro { repeat: 1, events }.to_bytes().is_err());
    }
}
