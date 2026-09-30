//! Physical keyboard layouts (key positions + matrix slots), loaded from JSON.

use std::collections::BTreeMap;

use crate::keymap::KeyAction;
use crate::{Frame, Rgb};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub id: String,
    pub label: String,
    /// Position and size in key units (1u = one standard key).
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Index into the 128-slot key/LED matrix.
    pub slot: usize,
    /// Default HID usage (0 for Fn).
    pub hid: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub id: String,
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub width: f32,
    pub height: f32,
    pub slots: usize,
    pub user_picture_layers: u8,
    pub onboard_profiles: u8,
    pub default_matrix: Vec<[u8; 4]>,
    pub keys: Vec<Key>,
    /// LEDs with no key of their own on this layout: a board shared with the ISO version
    /// keeps the ISO keys' LEDs, hidden under a wider ANSI cap or beside it.
    #[serde(default)]
    pub hidden_leds: Vec<HiddenLed>,
    /// The colour of the keyboard itself (case and keycaps), so the app can draw it as it
    /// looks on the desk.
    #[serde(default)]
    pub finish: Finish,
    /// What the Fn layer does by default, by key id (keys left out do nothing with Fn): what
    /// "Reset" puts back, and the standard Fn layer Settings offers.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fn_layer: BTreeMap<String, KeyAction>,
}

/// What a keyboard's case and keycaps look like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Finish {
    #[default]
    Dark,
    /// A white or clear case with white keycaps (the TK68's acrylic body).
    White,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenLed {
    pub slot: usize,
    /// The key whose cap covers it; the LED takes that key's colour.
    pub under: String,
}

pub const TK68_JSON: &str = include_str!("../../../layouts/epomaker-tk68.json");

impl Layout {
    pub fn tk68() -> Layout {
        serde_json::from_str(TK68_JSON).expect("bundled TK68 layout is valid")
    }

    /// A layout that ships with Keylume: a board's own (`layouts/`) or a standard one.
    pub fn bundled(id: &str) -> Option<Layout> {
        match id {
            "epomaker-tk68" => Some(Layout::tk68()),
            _ => crate::standard::standard(id),
        }
    }

    /// A picture from key colours; hidden LEDs follow the key above them, the rest stay off.
    pub fn frame(&self, colour: impl Fn(&Key) -> Option<Rgb>) -> Frame {
        let mut frame = Frame(vec![Rgb::BLACK; self.slots.max(1)]);
        for k in &self.keys {
            if let Some(c) = colour(k) {
                frame.set(k.slot, c);
            }
        }
        self.cover_hidden(&mut frame);
        frame
    }

    /// Give each hidden LED the colour of the key it sits under, so no stray light
    /// (off, or the factory picture) shows beside that key.
    pub fn cover_hidden(&self, frame: &mut Frame) {
        for h in &self.hidden_leds {
            if let Some(c) = self.key(&h.under).and_then(|k| frame.0.get(k.slot).copied()) {
                frame.set(h.slot, c);
            }
        }
    }

    pub fn key(&self, id: &str) -> Option<&Key> {
        self.keys.iter().find(|k| k.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fn_gives_f_keys_on_boards_without_an_f_row() {
        use crate::keymap::KeyAction;
        let tk68 = Layout::tk68();
        let f = |l: &Layout, id: &str| l.fn_layer.get(id).cloned();
        // the TK68: Fn + 1 … = are F1 … F12, media on free keys, its own functions kept
        assert_eq!(f(&tk68, "1"), Some(KeyAction::key(0x3A)));
        assert_eq!(f(&tk68, "equal"), Some(KeyAction::key(0x45)));
        assert_eq!(f(&tk68, "x"), Some(KeyAction::Consumer { usage: 0xCD }));
        assert_eq!(f(&tk68, "period"), Some(KeyAction::Consumer { usage: 0xEA }));
        assert!(matches!(f(&tk68, "up"), Some(KeyAction::Raw { .. })), "its lighting controls stay");
        assert_eq!(f(&tk68, "k"), Some(KeyAction::key(0x4A)), "Fn + K is still Home");
        // every key id in it is a key on the board
        assert!(tk68.fn_layer.keys().all(|id| tk68.key(id).is_some()));
        // standard layouts: the same rule
        for id in ["ansi-60", "ansi-65"] {
            let l = crate::standard::standard(id).unwrap();
            assert_eq!(f(&l, "1"), Some(KeyAction::key(0x3A)), "{id}");
            assert_eq!(f(&l, "z"), Some(KeyAction::Consumer { usage: 0xB6 }), "{id}");
        }
        let full = crate::standard::standard("ansi-full").unwrap();
        assert_eq!(f(&full, "1"), None, "a board with F-keys keeps its number row");
        assert_eq!(f(&full, "comma"), Some(KeyAction::Consumer { usage: 0xE2 }));
    }

    #[test]
    fn the_tk68_is_white_and_older_layouts_read_as_dark() {
        assert_eq!(Layout::tk68().finish, Finish::White);
        let mut v: serde_json::Value = serde_json::from_str(TK68_JSON).unwrap();
        v.as_object_mut().unwrap().remove("finish");
        assert_eq!(serde_json::from_value::<Layout>(v).unwrap().finish, Finish::Dark);
        assert_eq!(crate::standard::standard("ansi-full").unwrap().finish, Finish::Dark);
    }

    #[test]
    fn tk68_is_consistent() {
        let l = Layout::tk68();
        assert_eq!(l.keys.len(), 68);
        assert_eq!(l.default_matrix.len(), 128);
        let mut slots: Vec<_> = l.keys.iter().map(|k| k.slot).collect();
        slots.sort();
        slots.dedup();
        assert_eq!(slots.len(), 68, "every key has its own slot");
        for k in &l.keys {
            let m = l.default_matrix[k.slot];
            if k.id == "fn" {
                assert_eq!(m, [10, 1, 0, 0]);
            } else {
                assert_eq!(m, [0, 0, k.hid, 0], "{}", k.id);
            }
        }
        assert_eq!(l.key("esc").unwrap().slot, 1);
        // the hidden ISO LEDs sit on free matrix slots, under keys that exist
        for h in &l.hidden_leds {
            assert!(l.keys.iter().all(|k| k.slot != h.slot) && l.key(&h.under).is_some(), "{h:?}");
        }
        assert_eq!(l.hidden_leds.len(), 2);
    }

    #[test]
    fn hidden_leds_follow_their_key() {
        let l = Layout::tk68();
        let f = l.frame(|k| match k.id.as_str() {
            "enter" => Some(Rgb(255, 0, 0)),
            "lshift" => Some(Rgb(0, 255, 0)),
            _ => None,
        });
        assert_eq!(f.0[75], Rgb(255, 0, 0), "the ISO # LED under Enter");
        assert_eq!(f.0[10], Rgb(0, 255, 0), "the ISO \\ LED under left Shift, below A");
        assert_eq!(f.0[l.key("a").unwrap().slot], Rgb::BLACK);
    }
}
