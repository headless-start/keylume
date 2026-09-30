//! "Spell" lighting: words typed out on their own keys, one letter at a time.
//!
//! The keyboard can't animate individual keys in real time, so a spell is a short
//! sequence of per-key pictures (about 1.7 s each, the firmware's flash commit). It
//! plays once and then holds the final picture with every word lit.

use std::collections::{BTreeMap, BTreeSet};

use keylume_proto::{Layout, Rgb};
use serde::{Deserialize, Serialize};

use crate::color::{dim, mix};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellWord {
    pub text: String,
    pub color: Rgb,
}

const WHITE: Rgb = Rgb(255, 255, 255);

/// Key ids for the letters and digits of `text` that exist on `layout`, in order.
pub fn keys_of(layout: &Layout, text: &str) -> Vec<String> {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase().to_string())
        .filter(|id| layout.key(id).is_some())
        .collect()
}

fn blank(layout: &Layout, background: Rgb) -> BTreeMap<String, Rgb> {
    layout.keys.iter().map(|k| (k.id.clone(), background)).collect()
}

/// The picture the spell ends on: every word in its colour; keys shared by several
/// words (the S in "Keeps" and "Glows") glow white.
pub fn final_frame(layout: &Layout, words: &[SpellWord], background: Rgb) -> BTreeMap<String, Rgb> {
    let mut f = blank(layout, background);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for w in words {
        for id in keys_of(layout, &w.text).into_iter().collect::<BTreeSet<_>>() {
            *seen.entry(id.clone()).or_default() += 1;
            f.insert(id, w.color);
        }
    }
    for (id, n) in seen {
        if n > 1 {
            f.insert(id, WHITE);
        }
    }
    f
}

/// Every picture of the spell, in order. For each word, one frame per letter: the
/// letters so far in the word's colour, the newest one white-hot; a repeated letter
/// (the second E of "Keeps") flashes white on its key. Earlier words stay dimly lit.
/// Ends with [`final_frame`].
pub fn frames(layout: &Layout, words: &[SpellWord], background: Rgb) -> Vec<BTreeMap<String, Rgb>> {
    let mut out = Vec::new();
    let mut done: Vec<(Vec<String>, Rgb)> = Vec::new();
    for w in words {
        let mut lit: Vec<String> = Vec::new();
        for id in keys_of(layout, &w.text) {
            let mut f = blank(layout, background);
            for (ids, c) in &done {
                for k in ids {
                    f.insert(k.clone(), dim(*c, 0.12)); // ~35% in sRGB
                }
            }
            for k in &lit {
                f.insert(k.clone(), w.color);
            }
            if lit.contains(&id) {
                f.insert(id.clone(), WHITE);
            } else {
                f.insert(id.clone(), mix(w.color, WHITE, 0.55));
                lit.push(id);
            }
            out.push(f);
        }
        if !lit.is_empty() {
            done.push((lit, w.color));
        }
    }
    out.push(final_frame(layout, words, background));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_words() -> Vec<SpellWord> {
        vec![SpellWord { text: "Keeps".into(), color: Rgb(0, 200, 255) }, SpellWord { text: "Glows".into(), color: Rgb(255, 176, 0) }]
    }

    #[test]
    fn spells_letter_by_letter_then_holds() {
        let l = Layout::tk68();
        let bg = Rgb(2, 10, 58);
        let f = frames(&l, &two_words(), bg);
        // K E E P S + G L O W S + final
        assert_eq!(f.len(), 11);
        // frame 1: only K lit (white-hot), everything else background
        assert_ne!(f[0]["k"], bg);
        assert_eq!(f[0].values().filter(|c| **c != bg).count(), 1);
        // frame 3: the repeated E flashes white
        assert_eq!(f[2]["e"], WHITE);
        // while spelling the second word, the first stays dimly lit
        assert!(f[6]["k"] != bg && f[6]["k"].2 < 200);
        // final: both words, the shared S is white
        let last = f.last().unwrap();
        assert_eq!(last["k"], Rgb(0, 200, 255));
        assert_eq!(last["g"], Rgb(255, 176, 0));
        assert_eq!(last["s"], WHITE);
        assert_eq!(last["q"], bg);
    }

    #[test]
    fn ignores_characters_without_keys() {
        let l = Layout::tk68();
        assert_eq!(keys_of(&l, "Go-Lu!"), ["g", "o", "l", "u"]);
        let f = frames(&l, &[SpellWord { text: "?!".into(), color: WHITE }], Rgb::BLACK);
        assert_eq!(f.len(), 1, "nothing to spell: just the final picture");
    }
}
