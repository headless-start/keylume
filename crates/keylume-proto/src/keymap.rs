//! Key remapping (base layer 0x09 / 0x89, Fn layer 0x10 / 0x90).
//!
//! 128 slots × 4 bytes. Write = 9 pages (504 bytes), read = 8 pages of 64.

use serde::{Deserialize, Serialize};

use crate::packet::{cmd, paged_write, read_request, Packet};

pub const SLOTS: usize = 128;
pub const MATRIX_BYTES: usize = SLOTS * 4;
const WRITE_LEN: u16 = 0x01F8;
const WRITE_PAGES: usize = 9;
const READ_PAGES: u8 = 8;

/// HID modifier usages as used in combo actions.
pub mod modifier {
    pub const CTRL: u8 = 0xE0;
    pub const SHIFT: u8 = 0xE1;
    pub const ALT: u8 = 0xE2;
    pub const WIN: u8 = 0xE3;
}

/// What a key does. Unknown vendor encodings are kept verbatim as `Raw`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum KeyAction {
    Disabled,
    /// A HID keyboard usage, optionally with one modifier and a second key.
    Key {
        code: u8,
        #[serde(default)]
        modifier: u8,
        #[serde(default)]
        code2: u8,
    },
    /// Mouse button / wheel: 0xF0 left, 0xF1 right, 0xF2 middle, 0xF3 back, 0xF4 forward, 0xF5 wheel.
    Mouse {
        code: u8,
        #[serde(default)]
        arg: u8,
    },
    /// System control: 0x81 power, 0x82 sleep, 0x83 wake.
    System {
        code: u8,
    },
    /// Consumer usage (media keys, launchers).
    Consumer {
        usage: u16,
    },
    /// Onboard profile control: op 0 = value, 1 = next, 2 = prev, 3 = loop, 4 = select `arg`.
    Profile {
        op: u8,
        arg: u8,
    },
    /// Play a stored macro. mode 0 = repeat N times, 1 = toggle, 2 = while held.
    Macro {
        index: u8,
        #[serde(default)]
        mode: u8,
    },
    Fn,
    Raw {
        bytes: [u8; 4],
    },
}

impl KeyAction {
    pub fn key(code: u8) -> Self {
        KeyAction::Key { code, modifier: 0, code2: 0 }
    }

    pub fn encode(self) -> [u8; 4] {
        match self {
            KeyAction::Disabled => [0, 0, 0, 0],
            KeyAction::Key { code, modifier, code2 } => [0, modifier, code, code2],
            KeyAction::Mouse { code, arg } => [1, 0, code, arg],
            KeyAction::System { code } => [2, code, 0, 0],
            KeyAction::Consumer { usage } => {
                let [lo, hi] = usage.to_le_bytes();
                [3, 0, lo, hi]
            }
            KeyAction::Profile { op, arg } => [8, 0, op, arg],
            KeyAction::Macro { index, mode } => [9, mode, index, 0],
            KeyAction::Fn => [10, 1, 0, 0],
            KeyAction::Raw { bytes } => bytes,
        }
    }

    /// Does every number in it point at something that exists: a macro slot below
    /// [`crate::macros::MACRO_SLOTS`], a playback mode, an onboard profile below
    /// `profiles`? (Other codes are data and pass as they are.)
    pub fn is_valid(&self, profiles: u8) -> bool {
        match *self {
            KeyAction::Macro { index, mode } => index < crate::macros::MACRO_SLOTS && mode <= 2,
            KeyAction::Profile { op, arg } => op <= 4 && (op != 4 || arg < profiles),
            _ => true,
        }
    }

    pub fn decode(b: [u8; 4]) -> Self {
        match b {
            [0, 0, 0, 0] => KeyAction::Disabled,
            [0, m, c, c2] => KeyAction::Key { code: c, modifier: m, code2: c2 },
            [1, 0, c, a] => KeyAction::Mouse { code: c, arg: a },
            [2, c, 0, 0] => KeyAction::System { code: c },
            [3, 0, lo, hi] => KeyAction::Consumer { usage: u16::from_le_bytes([lo, hi]) },
            [8, 0, op, a] => KeyAction::Profile { op, arg: a },
            [9, mode, i, 0] => KeyAction::Macro { index: i, mode },
            [10, 1, 0, 0] => KeyAction::Fn,
            _ => KeyAction::Raw { bytes: b },
        }
    }
}

/// A full layer: one action per matrix slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keymap(pub Vec<KeyAction>);

impl Keymap {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Keymap(
            (0..SLOTS)
                .map(|i| match bytes.get(i * 4..i * 4 + 4) {
                    Some(b) => KeyAction::decode([b[0], b[1], b[2], b[3]]),
                    None => KeyAction::Disabled,
                })
                .collect(),
        )
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(MATRIX_BYTES);
        for i in 0..SLOTS {
            out.extend_from_slice(&self.0.get(i).copied().unwrap_or(KeyAction::Disabled).encode());
        }
        out
    }

    /// Base layer of onboard `profile`.
    pub fn write_packets(&self, profile: u8) -> Vec<Packet> {
        paged_write(cmd::KEYMAP, profile, WRITE_LEN, WRITE_PAGES, &self.to_bytes())
    }

    pub fn read_requests(profile: u8) -> Vec<Packet> {
        (0..READ_PAGES).map(|p| read_request(cmd::KEYMAP, profile, p)).collect()
    }

    /// Fn layer `fn_index` (0 on the TK68).
    pub fn write_fn_packets(&self, fn_index: u8) -> Vec<Packet> {
        paged_write(cmd::FN_KEYMAP, fn_index, WRITE_LEN, WRITE_PAGES, &self.to_bytes())
    }

    pub fn read_fn_requests(fn_index: u8) -> Vec<Packet> {
        (0..READ_PAGES).map(|p| read_request(cmd::FN_KEYMAP, fn_index, p)).collect()
    }
}

/// Common consumer usages for the UI.
pub mod consumer {
    pub const PLAY_PAUSE: u16 = 0x00CD;
    pub const STOP: u16 = 0x00B7;
    pub const NEXT: u16 = 0x00B5;
    pub const PREV: u16 = 0x00B6;
    pub const MUTE: u16 = 0x00E2;
    pub const VOL_UP: u16 = 0x00E9;
    pub const VOL_DOWN: u16 = 0x00EA;
    pub const MEDIA_PLAYER: u16 = 0x0183;
    pub const MAIL: u16 = 0x018A;
    pub const CALCULATOR: u16 = 0x0192;
    pub const MY_COMPUTER: u16 = 0x0194;
    pub const SEARCH: u16 = 0x0221;
    pub const HOME: u16 = 0x0223;
    pub const BACK: u16 = 0x0224;
    pub const REFRESH: u16 = 0x0227;
    pub const BRIGHTNESS_UP: u16 = 0x006F;
    pub const BRIGHTNESS_DOWN: u16 = 0x0070;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_match_vendor_table() {
        assert_eq!(KeyAction::key(0x29).encode(), [0, 0, 0x29, 0]);
        assert_eq!(KeyAction::Key { code: 47, modifier: modifier::SHIFT, code2: 0 }.encode(), [0, 225, 47, 0]); // "{"
        assert_eq!(KeyAction::Consumer { usage: consumer::PLAY_PAUSE }.encode(), [3, 0, 205, 0]);
        assert_eq!(KeyAction::Consumer { usage: consumer::CALCULATOR }.encode(), [3, 0, 146, 1]);
        assert_eq!(KeyAction::Mouse { code: 0xF0, arg: 0 }.encode(), [1, 0, 240, 0]);
        assert_eq!(KeyAction::Macro { index: 3, mode: 1 }.encode(), [9, 1, 3, 0]);
        assert_eq!(KeyAction::Fn.encode(), [10, 1, 0, 0]);
        assert_eq!(KeyAction::System { code: 0x82 }.encode(), [2, 130, 0, 0]);
    }

    #[test]
    fn decode_is_inverse() {
        for b in [[0, 0, 0, 0], [0, 0, 0x29, 0], [3, 0, 0xCD, 0], [9, 2, 5, 0], [10, 1, 0, 0], [0x0D, 8, 1, 0], [1, 0, 0xF5, 0xFF]] {
            assert_eq!(KeyAction::decode(b).encode(), b);
        }
        assert!(matches!(KeyAction::decode([0x0D, 8, 1, 0]), KeyAction::Raw { .. }));
    }

    #[test]
    fn write_header_matches_capture() {
        let km = Keymap(vec![KeyAction::Disabled; SLOTS]);
        let p = km.write_packets(0);
        assert_eq!(p.len(), 9);
        assert_eq!(&p[1][..8], &[0x09, 0x00, 0xF8, 0x01, 0x01, 0x00, 0x00, 0xFC]);
        let f = km.write_fn_packets(0);
        assert_eq!(&f[0][..8], &[0x10, 0x00, 0xF8, 0x01, 0x00, 0x00, 0x00, 0xF6]);
    }
}
