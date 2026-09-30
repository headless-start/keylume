//! Standard ANSI layouts (60 %, 65 %, 75 %, TKL and full size), the HID key codes of every
//! key id Keylume knows, and layouts built from where a keyboard says its lights are.
//!
//! Key ids are Keylume's own (`esc`, `a`, `lshift`, `kp7`…), the same on every board, so
//! a design made for one keyboard lights the same keys on another. Positions are in key
//! units (1u = one standard key, 19.05 mm).

use std::collections::BTreeMap;

use crate::keymap::KeyAction;
use crate::layout::{HiddenLed, Key, Layout};

/// A standard key's pitch in micrometres (0.75 inch).
pub const UNIT_UM: f32 = 19_050.0;

/// Key id, label and HID keyboard usage (page 0x07) of every key Keylume knows.
pub const KEYS: &[(&str, &str, u8)] = &[
    ("esc", "Esc", 0x29),
    ("f1", "F1", 0x3A),
    ("f2", "F2", 0x3B),
    ("f3", "F3", 0x3C),
    ("f4", "F4", 0x3D),
    ("f5", "F5", 0x3E),
    ("f6", "F6", 0x3F),
    ("f7", "F7", 0x40),
    ("f8", "F8", 0x41),
    ("f9", "F9", 0x42),
    ("f10", "F10", 0x43),
    ("f11", "F11", 0x44),
    ("f12", "F12", 0x45),
    ("prtsc", "PrtSc", 0x46),
    ("scrlk", "ScrLk", 0x47),
    ("pause", "Pause", 0x48),
    ("grave", "`", 0x35),
    ("1", "1", 0x1E),
    ("2", "2", 0x1F),
    ("3", "3", 0x20),
    ("4", "4", 0x21),
    ("5", "5", 0x22),
    ("6", "6", 0x23),
    ("7", "7", 0x24),
    ("8", "8", 0x25),
    ("9", "9", 0x26),
    ("0", "0", 0x27),
    ("minus", "-", 0x2D),
    ("equal", "=", 0x2E),
    ("backspace", "Backspace", 0x2A),
    ("ins", "Ins", 0x49),
    ("home", "Home", 0x4A),
    ("pgup", "PgUp", 0x4B),
    ("numlock", "Num", 0x53),
    ("kpslash", "/", 0x54),
    ("kpstar", "*", 0x55),
    ("kpminus", "-", 0x56),
    ("tab", "Tab", 0x2B),
    ("q", "Q", 0x14),
    ("w", "W", 0x1A),
    ("e", "E", 0x08),
    ("r", "R", 0x15),
    ("t", "T", 0x17),
    ("y", "Y", 0x1C),
    ("u", "U", 0x18),
    ("i", "I", 0x0C),
    ("o", "O", 0x12),
    ("p", "P", 0x13),
    ("lbracket", "[", 0x2F),
    ("rbracket", "]", 0x30),
    ("backslash", "\\", 0x31),
    ("delete", "Del", 0x4C),
    ("end", "End", 0x4D),
    ("pgdn", "PgDn", 0x4E),
    ("kp7", "7", 0x5F),
    ("kp8", "8", 0x60),
    ("kp9", "9", 0x61),
    ("kpplus", "+", 0x57),
    ("caps", "Caps", 0x39),
    ("a", "A", 0x04),
    ("s", "S", 0x16),
    ("d", "D", 0x07),
    ("f", "F", 0x09),
    ("g", "G", 0x0A),
    ("h", "H", 0x0B),
    ("j", "J", 0x0D),
    ("k", "K", 0x0E),
    ("l", "L", 0x0F),
    ("semicolon", ";", 0x33),
    ("quote", "'", 0x34),
    ("iso_hash", "#", 0x32),
    ("enter", "Enter", 0x28),
    ("kp4", "4", 0x5C),
    ("kp5", "5", 0x5D),
    ("kp6", "6", 0x5E),
    ("lshift", "Shift", 0xE1),
    ("iso_backslash", "\\", 0x64),
    ("z", "Z", 0x1D),
    ("x", "X", 0x1B),
    ("c", "C", 0x06),
    ("v", "V", 0x19),
    ("b", "B", 0x05),
    ("n", "N", 0x11),
    ("m", "M", 0x10),
    ("comma", ",", 0x36),
    ("period", ".", 0x37),
    ("slash", "/", 0x38),
    ("rshift", "Shift", 0xE5),
    ("up", "↑", 0x52),
    ("kp1", "1", 0x59),
    ("kp2", "2", 0x5A),
    ("kp3", "3", 0x5B),
    ("kpenter", "Enter", 0x58),
    ("lctrl", "Ctrl", 0xE0),
    ("lwin", "Win", 0xE3),
    ("lalt", "Alt", 0xE2),
    ("space", "", 0x2C),
    ("ralt", "Alt", 0xE6),
    ("rwin", "Win", 0xE7),
    ("fn", "Fn", 0x00),
    ("menu", "Menu", 0x65),
    ("rctrl", "Ctrl", 0xE4),
    ("left", "←", 0x50),
    ("down", "↓", 0x51),
    ("right", "→", 0x4F),
    ("kp0", "0", 0x62),
    ("kpdot", ".", 0x63),
];

/// The key id for a HID keyboard usage (`fn` has none).
pub fn key_for_usage(usage: u16) -> Option<&'static str> {
    KEYS.iter().find(|(_, _, u)| *u != 0 && *u as u16 == usage).map(|(id, _, _)| *id)
}

fn label(id: &str) -> &'static str {
    KEYS.iter().find(|(k, _, _)| *k == id).map(|(_, l, _)| *l).unwrap_or("")
}

fn usage(id: &str) -> u8 {
    KEYS.iter().find(|(k, _, _)| *k == id).map(|(_, _, u)| *u).unwrap_or(0)
}

/// Width and height of a key on standard boards (1×1 unless listed).
pub fn key_size(id: &str) -> (f32, f32) {
    match id {
        "backspace" => (2.0, 1.0),
        "tab" | "backslash" => (1.5, 1.0),
        "caps" => (1.75, 1.0),
        "enter" | "lshift" => (2.25, 1.0),
        "rshift" => (2.75, 1.0),
        "space" => (6.25, 1.0),
        "lctrl" | "lwin" | "lalt" | "ralt" | "rwin" | "menu" | "rctrl" => (1.25, 1.0),
        "kpplus" | "kpenter" => (1.0, 2.0),
        "kp0" => (2.0, 1.0),
        _ => (1.0, 1.0),
    }
}

/// The standard layouts, smallest first.
pub const STANDARD: &[&str] = &["ansi-60", "ansi-65", "ansi-75", "ansi-tkl", "ansi-full"];

/// A key in a row: id, the gap before it (in units), its width (`None`: the standard one).
type Slot = (&'static str, f32, Option<f32>);
/// One row of keys and its y position.
type Row = (Vec<Slot>, f32);

/// The standard Fn layer Keylume offers any keyboard with keys: on a board without an F-row
/// (60 and 65 %), Fn + 1 … = are F1 … F12; everywhere, Fn + Z X C are previous / play-pause /
/// next, Fn + , . / mute / volume down / volume up, and Fn + [ ] screen brightness down / up.
/// Keys a board lacks are left out; a board file may add its own functions beside these.
pub fn standard_fn(keys: &[Key]) -> BTreeMap<String, KeyAction> {
    let has = |id: &str| keys.iter().any(|k| k.id == id);
    let mut out = BTreeMap::new();
    if !has("f1") {
        for (i, id) in ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "minus", "equal"].into_iter().enumerate() {
            if has(id) {
                out.insert(id.to_string(), KeyAction::key(0x3A + i as u8)); // F1 = 0x3A … F12 = 0x45
            }
        }
    }
    let media: [(&str, u16); 8] =
        [("z", 0xB6), ("x", 0xCD), ("c", 0xB5), ("comma", 0xE2), ("period", 0xEA), ("slash", 0xE9), ("lbracket", 0x70), ("rbracket", 0x6F)];
    for (id, usage) in media {
        if has(id) {
            out.insert(id.to_string(), KeyAction::Consumer { usage });
        }
    }
    out
}

fn build(id: &str, name: &str, rows: &[Row]) -> Layout {
    let mut keys = Vec::new();
    for (row, y) in rows {
        let mut x = 0.0;
        for &(key, gap, w) in row {
            x += gap;
            let (dw, h) = key_size(key);
            let w = w.unwrap_or(dw);
            keys.push(Key { id: key.to_string(), label: label(key).into(), x, y: *y, w, h, slot: keys.len(), hid: usage(key) });
            x += w;
        }
    }
    let width = keys.iter().map(|k| k.x + k.w).fold(0.0, f32::max);
    let height = keys.iter().map(|k| k.y + k.h).fold(0.0, f32::max);
    let fn_layer = standard_fn(&keys);
    Layout {
        id: id.into(),
        name: name.into(),
        vid: 0,
        pid: 0,
        width,
        height,
        slots: keys.len(),
        user_picture_layers: 0,
        onboard_profiles: 0,
        default_matrix: Vec::new(),
        keys,
        hidden_leds: Vec::new(),
        finish: Default::default(),
        fn_layer,
    }
}

const NUMBER_ROW: &[Slot] = &[
    ("grave", 0.0, None),
    ("1", 0.0, None),
    ("2", 0.0, None),
    ("3", 0.0, None),
    ("4", 0.0, None),
    ("5", 0.0, None),
    ("6", 0.0, None),
    ("7", 0.0, None),
    ("8", 0.0, None),
    ("9", 0.0, None),
    ("0", 0.0, None),
    ("minus", 0.0, None),
    ("equal", 0.0, None),
    ("backspace", 0.0, None),
];
const TOP_ROW: &[Slot] = &[
    ("tab", 0.0, None),
    ("q", 0.0, None),
    ("w", 0.0, None),
    ("e", 0.0, None),
    ("r", 0.0, None),
    ("t", 0.0, None),
    ("y", 0.0, None),
    ("u", 0.0, None),
    ("i", 0.0, None),
    ("o", 0.0, None),
    ("p", 0.0, None),
    ("lbracket", 0.0, None),
    ("rbracket", 0.0, None),
    ("backslash", 0.0, None),
];
const HOME_ROW: &[Slot] = &[
    ("caps", 0.0, None),
    ("a", 0.0, None),
    ("s", 0.0, None),
    ("d", 0.0, None),
    ("f", 0.0, None),
    ("g", 0.0, None),
    ("h", 0.0, None),
    ("j", 0.0, None),
    ("k", 0.0, None),
    ("l", 0.0, None),
    ("semicolon", 0.0, None),
    ("quote", 0.0, None),
    ("enter", 0.0, None),
];
const LETTERS_ROW: &[Slot] = &[
    ("z", 0.0, None),
    ("x", 0.0, None),
    ("c", 0.0, None),
    ("v", 0.0, None),
    ("b", 0.0, None),
    ("n", 0.0, None),
    ("m", 0.0, None),
    ("comma", 0.0, None),
    ("period", 0.0, None),
    ("slash", 0.0, None),
];

fn with(parts: &[&[Slot]]) -> Vec<Slot> {
    parts.concat()
}

/// A standard layout by id (see [`STANDARD`]).
pub fn standard(id: &str) -> Option<Layout> {
    let k = |id: &'static str| (id, 0.0, None);
    let f_row = |gap_esc: f32, gap: f32| -> Vec<Slot> {
        let mut r = vec![k("esc")];
        for (i, f) in ["f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"].into_iter().enumerate() {
            r.push((
                f,
                if i == 0 {
                    gap_esc
                } else if i % 4 == 0 {
                    gap
                } else {
                    0.0
                },
                None,
            ));
        }
        r
    };
    let shift_row = |right: f32| -> Vec<Slot> {
        let mut r = vec![k("lshift")];
        r.extend_from_slice(LETTERS_ROW);
        r.push(("rshift", 0.0, Some(right)));
        r
    };
    Some(match id {
        "ansi-60" => build(
            id,
            "60 % (ANSI)",
            &[
                (with(&[&[k("esc")], &NUMBER_ROW[1..]]), 0.0),
                (TOP_ROW.to_vec(), 1.0),
                (HOME_ROW.to_vec(), 2.0),
                (shift_row(2.75), 3.0),
                (with(&[&[k("lctrl"), k("lwin"), k("lalt"), k("space"), k("ralt"), ("fn", 0.0, Some(1.25)), k("menu"), k("rctrl")]]), 4.0),
            ],
        ),
        "ansi-65" => build(
            id,
            "65 % (ANSI)",
            &[
                (with(&[&[k("esc")], &NUMBER_ROW[1..], &[k("grave")]]), 0.0),
                (with(&[TOP_ROW, &[k("delete")]]), 1.0),
                (with(&[HOME_ROW, &[k("pgup")]]), 2.0),
                (with(&[&shift_row(1.75), &[k("up"), k("pgdn")]]), 3.0),
                (
                    with(&[
                        &[k("lctrl"), k("lwin"), k("lalt"), k("space")],
                        &[("ralt", 0.0, Some(1.0)), ("fn", 0.0, None), ("rctrl", 0.0, Some(1.0))],
                        &[k("left"), k("down"), k("right")],
                    ]),
                    4.0,
                ),
            ],
        ),
        "ansi-75" => build(
            id,
            "75 % (ANSI)",
            &[
                (with(&[&f_row(0.0, 0.0), &[k("prtsc"), k("ins"), k("delete")]]), 0.0),
                (with(&[NUMBER_ROW, &[k("home")]]), 1.25),
                (with(&[TOP_ROW, &[k("pgup")]]), 2.25),
                (with(&[HOME_ROW, &[k("pgdn")]]), 3.25),
                (with(&[&shift_row(1.75), &[k("up"), k("end")]]), 4.25),
                (
                    with(&[
                        &[k("lctrl"), k("lwin"), k("lalt"), k("space")],
                        &[("ralt", 0.0, Some(1.0)), ("fn", 0.0, None), ("rctrl", 0.0, Some(1.0))],
                        &[k("left"), k("down"), k("right")],
                    ]),
                    5.25,
                ),
            ],
        ),
        "ansi-tkl" | "ansi-full" => {
            let full = id == "ansi-full";
            let nav = |a: &'static str, b: &'static str, c: &'static str| [(a, 0.25, None), k(b), k(c)];
            let mut rows: Vec<Row> = vec![
                (with(&[&f_row(1.0, 0.5), &nav("prtsc", "scrlk", "pause")]), 0.0),
                (with(&[NUMBER_ROW, &nav("ins", "home", "pgup")]), 1.25),
                (with(&[TOP_ROW, &nav("delete", "end", "pgdn")]), 2.25),
                (HOME_ROW.to_vec(), 3.25),
                (with(&[&shift_row(2.75), &[("up", 1.25, None)]]), 4.25),
                (
                    with(&[
                        &[k("lctrl"), k("lwin"), k("lalt"), k("space"), k("ralt"), k("rwin"), k("menu"), k("rctrl")],
                        &[("left", 0.25, None), k("down"), k("right")],
                    ]),
                    5.25,
                ),
            ];
            if full {
                // the keypad starts 0.25u right of the navigation block (after x = 18.25)
                rows[1].0.extend([("numlock", 0.25, None), k("kpslash"), k("kpstar"), k("kpminus")]);
                rows[2].0.extend([("kp7", 0.25, None), k("kp8"), k("kp9"), k("kpplus")]);
                rows[3].0.extend([("kp4", 3.5, None), k("kp5"), k("kp6")]);
                rows[4].0.extend([("kp1", 1.25, None), k("kp2"), k("kp3"), k("kpenter")]);
                rows[5].0.extend([("kp0", 0.25, None), k("kpdot")]);
            }
            build(id, if full { "Full size (ANSI)" } else { "Tenkeyless (ANSI)" }, &rows)
        }
        _ => return None,
    })
}

/// Every key Keylume knows, side by side: for checking that a design names real keys
/// without caring which board it was made on.
pub fn every_key() -> Layout {
    let keys: Vec<Key> = KEYS
        .iter()
        .enumerate()
        .map(|(i, (id, label, hid))| Key { id: id.to_string(), label: label.to_string(), x: i as f32, y: 0.0, w: 1.0, h: 1.0, slot: i, hid: *hid })
        .collect();
    Layout {
        id: "every-key".into(),
        name: "any keyboard".into(),
        vid: 0,
        pid: 0,
        width: keys.len() as f32,
        height: 1.0,
        slots: keys.len(),
        user_picture_layers: 0,
        onboard_profiles: 0,
        default_matrix: Vec::new(),
        keys,
        hidden_leds: Vec::new(),
        finish: Default::default(),
        fn_layer: BTreeMap::new(),
    }
}

/// One light as a keyboard reports it (HID LampArray): where it is, and the key it sits
/// under (`None` for lights that aren't under a key: edges, logos, underglow).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    pub slot: usize,
    pub x_um: f32,
    pub y_um: f32,
    /// HID keyboard usage of the key above it (0: none).
    pub usage: u16,
    /// Under a key, even without a usage (e.g. Fn).
    pub key: bool,
}

/// A layout drawn from a keyboard's own description of its lights. Key lights become
/// keys at the positions reported (standard key sizes when the key is known); a key with
/// several lights keeps the first as its own and the others follow it; lights that
/// aren't under a key follow the nearest key, the way hidden LEDs do on other boards.
pub fn from_lamps(id: &str, name: &str, lamps: &[Lamp]) -> Layout {
    let mut keys: Vec<Key> = Vec::new();
    let mut followers: Vec<(usize, f32, f32)> = Vec::new();
    let degenerate = lamps.windows(2).all(|w| w[0].x_um == w[1].x_um && w[0].y_um == w[1].y_um);
    let full = standard("ansi-full").expect("built in");
    let mut spare = 0.0f32;
    for l in lamps {
        let known = key_for_usage(l.usage);
        if l.usage == 0 && !l.key {
            followers.push((l.slot, l.x_um, l.y_um));
            continue;
        }
        let id = known
            .map(str::to_string)
            .unwrap_or_else(|| if l.usage != 0 { format!("usage-{:02x}", l.usage) } else { format!("key-{}", l.slot) });
        if keys.iter().any(|k| k.id == id) {
            followers.push((l.slot, l.x_um, l.y_um));
            continue;
        }
        let (w, h) = key_size(&id);
        let (x, y) = if degenerate {
            // no positions reported: the key's place on a full-size board, extras in a row below
            match full.key(&id) {
                Some(k) => (k.x, k.y),
                None => {
                    spare += 1.0;
                    (spare - 1.0, full.height + 0.25)
                }
            }
        } else {
            (l.x_um / UNIT_UM - w / 2.0, l.y_um / UNIT_UM - h / 2.0)
        };
        keys.push(Key { id, label: known.map(label).unwrap_or("").into(), x, y, w, h, slot: l.slot, hid: l.usage.min(255) as u8 });
    }
    // origin at the top-left key, positions snapped to eighths of a key
    let (min_x, min_y) = keys.iter().fold((f32::MAX, f32::MAX), |(a, b), k| (a.min(k.x), b.min(k.y)));
    let snap = |v: f32| (v * 8.0).round() / 8.0;
    for k in &mut keys {
        k.x = snap(k.x - min_x);
        k.y = snap(k.y - min_y);
    }
    let centre = |k: &Key| ((k.x + k.w / 2.0 + min_x) * UNIT_UM, (k.y + k.h / 2.0 + min_y) * UNIT_UM);
    let hidden_leds = followers
        .into_iter()
        .filter_map(|(slot, x, y)| {
            let near = keys.iter().min_by(|a, b| {
                let d = |k: &Key| {
                    let (cx, cy) = centre(k);
                    (cx - x).powi(2) + (cy - y).powi(2)
                };
                d(a).total_cmp(&d(b))
            })?;
            Some(HiddenLed { slot, under: near.id.clone() })
        })
        .collect();
    Layout {
        id: id.into(),
        name: name.into(),
        vid: 0,
        pid: 0,
        width: keys.iter().map(|k| k.x + k.w).fold(0.0, f32::max),
        height: keys.iter().map(|k| k.y + k.h).fold(0.0, f32::max),
        slots: lamps.iter().map(|l| l.slot + 1).max().unwrap_or(0),
        user_picture_layers: 0,
        onboard_profiles: 0,
        default_matrix: Vec::new(),
        keys,
        hidden_leds,
        finish: Default::default(),
        fn_layer: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn standard_layouts_are_consistent() {
        let expect = [("ansi-60", 61), ("ansi-65", 68), ("ansi-75", 84), ("ansi-tkl", 87), ("ansi-full", 104)];
        for (id, n) in expect {
            let l = standard(id).unwrap();
            assert_eq!(l.keys.len(), n, "{id}");
            let ids: HashSet<_> = l.keys.iter().map(|k| k.id.as_str()).collect();
            assert_eq!(ids.len(), n, "{id}: every key once");
            for k in &l.keys {
                assert!(KEYS.iter().any(|(i, _, _)| *i == k.id), "{id}: {} is a known key", k.id);
            }
            // no two keys overlap
            for (i, a) in l.keys.iter().enumerate() {
                for b in &l.keys[i + 1..] {
                    let overlap = a.x < b.x + b.w - 0.01 && b.x < a.x + a.w - 0.01 && a.y < b.y + b.h - 0.01 && b.y < a.y + a.h - 0.01;
                    assert!(!overlap, "{id}: {} overlaps {}", a.id, b.id);
                }
            }
        }
        let full = standard("ansi-full").unwrap();
        assert_eq!((full.width, full.height), (22.5, 6.25));
        assert_eq!(standard("ansi-tkl").unwrap().width, 18.25);
        assert_eq!(standard("ansi-60").unwrap().width, 15.0);
        assert_eq!(standard("ansi-75").unwrap().width, 16.0);
        // the 65 % board has the TK68's keys
        let tk68: HashSet<_> = Layout::tk68().keys.into_iter().map(|k| k.id).collect();
        let l65: HashSet<_> = standard("ansi-65").unwrap().keys.into_iter().map(|k| k.id).collect();
        assert_eq!(tk68, l65);
    }

    #[test]
    fn usages_match_the_tk68() {
        for k in Layout::tk68().keys {
            assert_eq!(usage(&k.id), k.hid, "{}", k.id);
            if k.hid != 0 {
                assert_eq!(key_for_usage(k.hid as u16), Some(k.id.as_str()));
            }
        }
    }

    #[test]
    fn lamps_become_a_layout() {
        let full = standard("ansi-full").unwrap();
        // a keyboard reporting its key lights at the full-size positions, plus two edge lights
        let mut lamps: Vec<Lamp> = full
            .keys
            .iter()
            .map(|k| Lamp {
                slot: k.slot,
                x_um: (k.x + k.w / 2.0 + 1.0) * UNIT_UM,
                y_um: (k.y + k.h / 2.0 + 0.5) * UNIT_UM,
                usage: k.hid as u16,
                key: true,
            })
            .collect();
        lamps.push(Lamp { slot: 104, x_um: 0.0, y_um: 0.0, usage: 0, key: false });
        // a second light under the space bar
        let space = full.key("space").unwrap();
        lamps.push(Lamp { slot: 105, x_um: (space.x + 5.0) * UNIT_UM, y_um: (space.y + 1.0) * UNIT_UM, usage: 0x2C, key: true });
        let l = from_lamps("lamparray-test", "Test", &lamps);
        assert_eq!(l.keys.len(), 104);
        assert_eq!(l.slots, 106);
        assert_eq!((l.width, l.height), (full.width, full.height));
        for k in &full.keys {
            let got = l.key(&k.id).unwrap();
            assert_eq!((got.x, got.y, got.w, got.h, got.slot), (k.x, k.y, k.w, k.h, k.slot), "{}", k.id);
        }
        assert_eq!(l.hidden_leds.len(), 2);
        assert_eq!(l.hidden_leds.iter().find(|h| h.slot == 104).unwrap().under, "esc", "the corner light follows the nearest key");
        assert_eq!(l.hidden_leds.iter().find(|h| h.slot == 105).unwrap().under, "space");
    }

    #[test]
    fn lamps_without_positions_take_standard_places() {
        let lamps: Vec<Lamp> = [0x29u16, 0x04, 0x2C, 0xF0]
            .iter()
            .enumerate()
            .map(|(slot, &usage)| Lamp { slot, x_um: 0.0, y_um: 0.0, usage, key: true })
            .collect();
        let l = from_lamps("x", "X", &lamps);
        assert_eq!(l.keys.len(), 4);
        assert!(l.key("usage-f0").is_some(), "an unknown key keeps its usage");
        let a = l.key("a").unwrap();
        assert!(a.y > l.key("esc").unwrap().y);
    }
}
