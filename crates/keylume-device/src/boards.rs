//! Keyboards described as data (`boards/*.json`): how to recognise one, which driver
//! talks to it, its layout, and what it can do. The app adapts to whatever board is
//! connected from this description; nothing else in Keylume names a model.
//!
//! Keyboards that implement the HID LampArray standard need no file: they describe their
//! own lights, and [`lamparray_features`] says what Keylume does with them.

use std::sync::OnceLock;

use keylume_proto::Mode;
use serde::{Deserialize, Deserializer, Serialize};

/// Which protocol talks to the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Driver {
    /// Rongyuan-based boards (the TK68's family): 64-byte feature reports, firmware
    /// effects, pictures stored in flash (`docs/PROTOCOL.md`).
    Rongyuan,
    /// The HID LampArray standard (Windows Dynamic Lighting): Keylume drives every light.
    Lamparray,
}

/// How far Keylume trusts its support for a board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    /// Checked on the real keyboard.
    Verified,
    /// Built to a public standard or a description nobody has checked on the board yet.
    Experimental,
}

/// What a board can do; the app offers only these.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    /// Every key can have its own colour.
    pub per_key: bool,
    /// Onboard picture layers per-key designs can be stored in (0: Keylume holds them).
    pub picture_layers: u8,
    /// Animations available: run by the firmware, or by Keylume when `host_driven`.
    pub effects: Vec<Mode>,
    /// Keylume drives the lights frame by frame (nothing is stored on the board; the
    /// board shows its own lighting again when Keylume lets go).
    pub host_driven: bool,
    pub side_light: bool,
    /// Live effects (one colour for the board, or music bars).
    pub live: bool,
    pub keymap: bool,
    pub macros: bool,
    pub onboard_profiles: u8,
    /// Report rate, debounce and sleep timers.
    pub settings: bool,
    pub backup: bool,
    /// Keylume can read back what the board shows.
    pub read_back: bool,
}

/// USB identity checks before anything is written (see [`crate::identity`]).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usb {
    #[serde(deserialize_with = "hex")]
    pub vid: u16,
    #[serde(deserialize_with = "hex")]
    pub pid: u16,
    /// USB interface and top-level collection that carry the configuration channel.
    pub interface: i32,
    #[serde(deserialize_with = "hex")]
    pub usage_page: u16,
    #[serde(deserialize_with = "hex")]
    pub usage: u16,
    /// USB strings as the verified board reports them (compared ignoring case).
    pub manufacturer: String,
    pub products: Vec<String>,
    /// Size of the feature report the protocol runs on (report ID 0).
    pub feature_report: usize,
}

/// One keyboard model.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardDef {
    pub id: String,
    pub name: String,
    pub maker: String,
    pub driver: Driver,
    pub support: Support,
    #[serde(default)]
    pub note: String,
    /// Layout id (`layouts/<id>.json`, or a standard layout).
    pub layout: String,
    pub usb: Usb,
    pub features: Features,
}

fn hex<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    let s = String::deserialize(d)?;
    u16::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
}

/// The board files bundled with the app.
const FILES: &[&str] = &[include_str!("../../../boards/epomaker-tk68.json")];

/// Every board Keylume knows, in file order.
pub fn all() -> &'static [BoardDef] {
    static ALL: OnceLock<Vec<BoardDef>> = OnceLock::new();
    ALL.get_or_init(|| FILES.iter().map(|f| serde_json::from_str(f).expect("bundled board files are valid")).collect())
}

pub fn by_id(id: &str) -> Option<&'static BoardDef> {
    all().iter().find(|b| b.id == id)
}

/// What Keylume offers on a LampArray keyboard: per-key colour and every animation it can
/// draw itself (not the ones that react to typing: Keylume doesn't watch the keys).
pub fn lamparray_features() -> Features {
    use Mode::*;
    Features {
        per_key: true,
        picture_layers: 0,
        effects: vec![
            Off,
            Static,
            Breathing,
            Spectrum,
            Wave,
            Raindrop,
            Snake,
            Converge,
            SineWave,
            Kaleidoscope,
            LineWave,
            CircleWave,
            Dazzle,
            RainDown,
            Meteor,
        ],
        host_driven: true,
        side_light: false,
        live: true,
        keymap: false,
        macros: false,
        onboard_profiles: 0,
        settings: false,
        backup: false,
        read_back: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_boards_load() {
        let tk68 = by_id("epomaker-tk68").unwrap();
        assert_eq!((tk68.usb.vid, tk68.usb.pid, tk68.usb.usage_page, tk68.usb.usage), (0x05AC, 0x024F, 0x01, 0x06));
        assert_eq!(tk68.support, Support::Verified);
        assert_eq!(tk68.features.effects.len(), Mode::ALL.len(), "the TK68 runs every firmware mode");
        assert!(keylume_proto::layout::Layout::bundled(&tk68.layout).is_some());
        // ids are unique and every layout exists
        for (i, b) in all().iter().enumerate() {
            assert!(all()[i + 1..].iter().all(|o| o.id != b.id), "{} twice", b.id);
            assert!(keylume_proto::layout::Layout::bundled(&b.layout).is_some(), "{}: layout {}", b.id, b.layout);
        }
    }

    #[test]
    fn lamparray_boards_never_offer_typing_effects() {
        let f = lamparray_features();
        for m in [Mode::Ripple, Mode::Reactive, Mode::ReactiveOff, Mode::Laser, Mode::UserPicture, Mode::MusicBars] {
            assert!(!f.effects.contains(&m), "{m:?}");
        }
    }
}
