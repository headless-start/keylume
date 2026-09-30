//! Morse code timing for the `Morse` live effect.
//!
//! A message becomes a list of on/off *units* (dot = 1 on, dash = 3 on, 1 off between
//! symbols, 3 between letters, 7 between words and before the message repeats).

fn code(c: char) -> Option<&'static str> {
    Some(match c.to_ascii_uppercase() {
        'A' => ".-",
        'B' => "-...",
        'C' => "-.-.",
        'D' => "-..",
        'E' => ".",
        'F' => "..-.",
        'G' => "--.",
        'H' => "....",
        'I' => "..",
        'J' => ".---",
        'K' => "-.-",
        'L' => ".-..",
        'M' => "--",
        'N' => "-.",
        'O' => "---",
        'P' => ".--.",
        'Q' => "--.-",
        'R' => ".-.",
        'S' => "...",
        'T' => "-",
        'U' => "..-",
        'V' => "...-",
        'W' => ".--",
        'X' => "-..-",
        'Y' => "-.--",
        'Z' => "--..",
        '0' => "-----",
        '1' => ".----",
        '2' => "..---",
        '3' => "...--",
        '4' => "....-",
        '5' => ".....",
        '6' => "-....",
        '7' => "--...",
        '8' => "---..",
        '9' => "----.",
        _ => return None,
    })
}

/// On/off state of every unit of `message`, ending with the gap before it repeats.
/// Unknown characters are skipped; an empty result means nothing to blink.
pub fn units(message: &str) -> Vec<bool> {
    let mut out = Vec::new();
    for word in message.split_whitespace() {
        let letters: Vec<&str> = word.chars().filter_map(code).collect();
        if letters.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.extend([false; 7]);
        }
        for (li, l) in letters.iter().enumerate() {
            if li > 0 {
                out.extend([false; 3]);
            }
            for (si, s) in l.chars().enumerate() {
                if si > 0 {
                    out.push(false);
                }
                out.extend(std::iter::repeat_n(true, if s == '-' { 3 } else { 1 }));
            }
        }
    }
    if !out.is_empty() {
        out.extend([false; 7]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(u: &[bool]) -> String {
        u.iter().map(|&b| if b { '#' } else { '_' }).collect()
    }

    #[test]
    fn mixed_case_word_spells_correctly() {
        // L .-..  E .  E .  T -
        assert_eq!(show(&units("LeeT")), "#_###_#_#___#___#___###_______");
    }

    #[test]
    fn words_and_junk() {
        assert_eq!(show(&units("e e")), "#_______#_______");
        assert!(units("!?").is_empty());
        assert_eq!(show(&units("sos")), "#_#_#___###_###_###___#_#_#_______");
    }
}
