//! Snapshot and restore everything stored on the keyboard.
//!
//! A restore rewrites the keyboard's flash, so a backup is checked in full first: the
//! file on its own ([`Backup::validate`]), then against the keyboard it's going to
//! ([`check`]). Nothing is written unless all of it passes. If writing stops part-way
//! (the cable is pulled), [`RestoreError::Partial`] says what was already written and
//! how to recover.

use std::io::Read;
use std::path::Path;

use keylume_device::{DeviceInfo, ONBOARD_PROFILES};
use keylume_proto::keymap::{Keymap, SLOTS as KEY_SLOTS};
use keylume_proto::macros::{Macro, MACRO_SLOTS};
use keylume_proto::picture::{Frame, SLOTS as LED_SLOTS};
use keylume_proto::settings::{KeyboardOptions, ReportRate, SleepTimers, MAX_DEBOUNCE};
use keylume_proto::{Effect, Mode, SideLight};
use serde::{Deserialize, Serialize};

use crate::board::Board;

/// The backup format this version writes and reads.
pub const FORMAT: u32 = 1;
/// A backup is about 100 KB; anything much bigger isn't one.
pub const MAX_FILE_BYTES: u64 = 4 << 20;
/// Picture layers on the keyboard.
const PICTURE_LAYERS: usize = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub keylume_backup: u32,
    pub device: String,
    pub firmware: u16,
    pub effect: Effect,
    pub pictures: Vec<Frame>,
    pub keymaps: Vec<Keymap>,
    pub fn_keymap: Keymap,
    pub macros: Vec<Macro>,
    pub report_rate: ReportRate,
    pub debounce: u8,
    pub sleep: SleepTimers,
    pub options: KeyboardOptions,
    pub profile: u8,
    /// The side light strip (backups made before 0.3.10 don't have it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_light: Option<SideLight>,
}

/// Read everything. Takes a while (each picture layer switch needs a settle).
pub fn snapshot(b: &dyn Board) -> keylume_device::Result<Backup> {
    let effect = b.effect()?;
    let pictures = (0..PICTURE_LAYERS as u8).map(|l| b.read_picture(l)).collect::<Result<Vec<_>, _>>()?;
    let keymaps = (0..ONBOARD_PROFILES).map(|p| b.keymap(p)).collect::<Result<Vec<_>, _>>()?;
    let backup = Backup {
        keylume_backup: FORMAT,
        device: b.info().board.clone(),
        firmware: b.firmware_version()?,
        effect,
        pictures,
        keymaps,
        fn_keymap: b.fn_keymap(0)?,
        macros: (0..MACRO_SLOTS).map(|i| b.macro_at(i)).collect::<Result<Vec<_>, _>>()?,
        report_rate: b.report_rate()?,
        debounce: b.debounce()?,
        sleep: b.sleep_timers()?,
        options: b.options()?,
        profile: b.profile()?,
        side_light: b.side_light().ok(),
    };
    // reading pictures switched layers: put the original lighting back
    b.set_effect(&backup.effect)?;
    Ok(backup)
}

impl Backup {
    /// Is this, on its own, a backup Keylume can restore? Err says why, in words.
    pub fn validate(&self) -> Result<(), String> {
        if self.keylume_backup != FORMAT {
            return Err(format!("it's backup format {}, and this version of Keylume reads format {FORMAT}", self.keylume_backup));
        }
        let count = |what: &str, got: usize, want: usize| {
            if got == want {
                Ok(())
            } else {
                Err(format!("it holds {got} {what}, a keyboard has {want}"))
            }
        };
        count("picture layers", self.pictures.len(), PICTURE_LAYERS)?;
        count("key maps", self.keymaps.len(), ONBOARD_PROFILES as usize)?;
        count("macros", self.macros.len(), MACRO_SLOTS as usize)?;
        for (i, f) in self.pictures.iter().enumerate() {
            count(&format!("colours in layer {}", i + 1), f.0.len(), LED_SLOTS)?;
        }
        for (i, k) in self.keymaps.iter().chain([&self.fn_keymap]).enumerate() {
            let name = if i < self.keymaps.len() { format!("key map {}", i + 1) } else { "the Fn layer".into() };
            count(&format!("keys in {name}"), k.0.len(), KEY_SLOTS)?;
            if !k.0.iter().all(|a| a.is_valid(ONBOARD_PROFILES)) {
                return Err(format!("{name} refers to a macro or onboard profile the keyboard doesn't have"));
            }
        }
        for (i, m) in self.macros.iter().enumerate() {
            m.to_bytes().map_err(|e| format!("macro {}: {e}", i + 1))?;
        }
        self.effect.validate().map_err(|e| format!("its lighting effect: {e}"))?;
        if self.effect.mode == Mode::UserPicture && self.effect.direction as usize >= PICTURE_LAYERS {
            return Err("its lighting effect shows a picture layer the keyboard doesn't have".into());
        }
        if self.profile >= ONBOARD_PROFILES {
            return Err(format!("its active onboard profile is {}, a keyboard has {ONBOARD_PROFILES}", self.profile + 1));
        }
        if self.debounce > MAX_DEBOUNCE {
            return Err(format!("its debounce is {}, above {MAX_DEBOUNCE}", self.debounce));
        }
        if let Some(s) = &self.side_light {
            s.validate().map_err(|e| format!("its side light: {e}"))?;
        }
        Ok(())
    }

    /// Was this made on this kind of keyboard, with the firmware it runs now?
    pub fn check_device(&self, device: &DeviceInfo, firmware: u16) -> Result<(), String> {
        if self.device != device.board {
            return Err(format!("it was made on a {:?}, not a {:?}", self.device, device.board));
        }
        if self.firmware != firmware {
            return Err(format!(
                "it was made on firmware {}, and this keyboard runs {}; Keylume only restores a backup onto the firmware it came from",
                firmware_label(self.firmware),
                firmware_label(firmware)
            ));
        }
        Ok(())
    }
}

/// "3.04" from the version word the keyboard reports (0x0304).
pub fn firmware_label(fw: u16) -> String {
    format!("{}.{:02x}", fw >> 8, fw & 0xFF)
}

/// Read and validate a backup file, reading at most [`MAX_FILE_BYTES`].
pub fn read_file(path: &Path) -> Result<Backup, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("can't open it ({e})"))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("can't read it ({e})"))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("it's far too big to be a Keylume backup".into());
    }
    let b: Backup = serde_json::from_slice(&bytes).map_err(|e| format!("it isn't a Keylume backup ({e})"))?;
    b.validate()?;
    Ok(b)
}

/// Write a backup file (to a temporary file first, so a failure never leaves half a file).
pub fn write_file(path: &Path, b: &Backup) -> std::io::Result<()> {
    crate::store::write_atomic(path, &serde_json::to_vec_pretty(b).map_err(std::io::Error::other)?)
}

/// Everything a restore checks before its first write: the backup itself, and that it
/// belongs to this keyboard and firmware (one read: the firmware version).
pub fn check(b: &dyn Board, s: &Backup) -> Result<(), String> {
    s.validate()?;
    let firmware = b.firmware_version().map_err(|e| format!("the keyboard didn't say which firmware it runs ({e})"))?;
    s.check_device(b.info(), firmware)
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RestoreError {
    /// Checked before anything was written.
    #[error("Nothing was restored: {0}.")]
    Refused(String),
    /// Writing stopped part-way.
    #[error(
        "The restore stopped while writing {failed} ({error}). {} The keyboard may now mix the backup with what it had: reconnect it and restore the same backup again. If its settings seem stuck, do a factory reset first, then restore.",
        if done.is_empty() { "Nothing before it was written.".to_string() } else { format!("Already restored: {}.", done.join(", ")) }
    )]
    Partial { done: Vec<String>, failed: String, error: String },
}

/// Write everything back, after [`check`]ing all of it. Stops at the first failure.
pub fn restore(b: &dyn Board, s: &Backup) -> Result<(), RestoreError> {
    check(b, s).map_err(RestoreError::Refused)?;
    let mut done: Vec<String> = Vec::new();
    let mut step = |what: String, r: keylume_device::Result<()>| match r {
        Ok(()) => {
            done.push(what);
            Ok(())
        }
        Err(e) => Err(RestoreError::Partial { done: done.clone(), failed: what, error: e.to_string() }),
    };
    for (l, f) in s.pictures.iter().enumerate() {
        step(format!("picture layer {}", l + 1), b.write_picture(l as u8, f, s.effect.brightness))?;
    }
    for (p, km) in s.keymaps.iter().enumerate() {
        step(format!("key map {}", p + 1), b.set_keymap(p as u8, km))?;
    }
    step("the Fn layer".into(), b.set_fn_keymap(0, &s.fn_keymap))?;
    for (i, m) in s.macros.iter().enumerate() {
        step(format!("macro {}", i + 1), b.set_macro(i as u8, m))?;
    }
    step("the polling rate".into(), b.set_report_rate(s.profile, s.report_rate))?;
    step("debounce".into(), b.set_debounce(s.debounce))?;
    step("sleep timers".into(), b.set_sleep_timers(s.sleep))?;
    step("keyboard options".into(), b.set_options(s.profile, s.options))?;
    step("the active onboard profile".into(), b.set_profile(s.profile))?;
    if let Some(side) = &s.side_light {
        step("the side light".into(), b.set_side_light(side))?;
    }
    step("the lighting effect".into(), b.set_effect(&s.effect))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use keylume_device::sim::SimKeyboard;
    use keylume_proto::keymap::KeyAction;

    fn board() -> keylume_device::Keyboard<SimKeyboard> {
        SimKeyboard::keyboard(50)
    }

    #[test]
    fn a_snapshot_validates_and_round_trips_through_a_file() {
        let b = board();
        let snap = snapshot(&b).unwrap();
        assert_eq!(snap.validate(), Ok(()));
        assert!(snap.side_light.is_some(), "the side light is backed up too");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tk68.json");
        write_file(&path, &snap).unwrap();
        assert_eq!(read_file(&path).unwrap(), snap);
        // backups from before the side light still read
        let mut old: serde_json::Value = serde_json::to_value(&snap).unwrap();
        old.as_object_mut().unwrap().remove("sideLight");
        std::fs::write(&path, old.to_string()).unwrap();
        assert_eq!(read_file(&path).unwrap().side_light, None);
    }

    #[test]
    fn files_that_are_not_backups_are_refused_before_parsing_much() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.json");
        std::fs::write(&big, vec![b' '; MAX_FILE_BYTES as usize + 10]).unwrap();
        assert!(read_file(&big).unwrap_err().contains("too big"));
        let junk = dir.path().join("junk.json");
        std::fs::write(&junk, "{\"keylume\": 1}").unwrap();
        assert!(read_file(&junk).unwrap_err().contains("isn't a Keylume backup"));
        assert!(read_file(&dir.path().join("missing.json")).is_err());
    }

    /// Every way a backup can be wrong, each on its own.
    pub(crate) fn broken(snap: &Backup) -> Vec<(&'static str, Backup)> {
        let mut out = Vec::new();
        let mut with = |name: &'static str, f: &dyn Fn(&mut Backup)| {
            let mut b = snap.clone();
            f(&mut b);
            out.push((name, b));
        };
        with("newer format", &|b| b.keylume_backup = 2);
        with("other keyboard", &|b| b.device = "some-other-board".into());
        with("other firmware", &|b| b.firmware = 0x0305);
        with("two layers", &|b| b.pictures.truncate(2));
        with("short layer", &|b| b.pictures[2].0.truncate(127));
        with("four key maps", &|b| b.keymaps.push(b.keymaps[0].clone()));
        with("short Fn layer", &|b| b.fn_keymap.0.truncate(127));
        with("seventeen macros", &|b| b.macros.push(Macro::default()));
        with("macro slot 40", &|b| b.keymaps[1].0[3] = KeyAction::Macro { index: 40, mode: 0 });
        with("profile 7", &|b| b.keymaps[0].0[3] = KeyAction::Profile { op: 4, arg: 7 });
        with("macro too long", &|b| {
            b.macros[5].events = (0..200)
                .flat_map(|_| [keylume_proto::MacroEvent::Key { code: 4, down: true }, keylume_proto::MacroEvent::Delay { ms: 900 }])
                .collect()
        });
        with("effect speed 9", &|b| b.effect.speed = 9);
        with("picture layer 4", &|b| b.effect = Effect { direction: 3, ..Effect::user_picture(0) });
        with("active profile 3", &|b| b.profile = 3);
        with("debounce 200", &|b| b.debounce = 200);
        with("side light speed 7", &|b| b.side_light = Some(SideLight { speed: 7, ..SideLight::off() }));
        out
    }

    #[test]
    fn a_bad_backup_writes_nothing_at_all() {
        let b = board();
        let snap = snapshot(&b).unwrap();
        for (name, bad) in broken(&snap) {
            b.settle();
            let writes = b.transport().stats().writes;
            let err = restore(&b, &bad).unwrap_err();
            assert!(matches!(err, RestoreError::Refused(_)), "{name}: {err}");
            assert_eq!(b.transport().stats().writes, writes, "{name}: something was written");
        }
        restore(&b, &snap).unwrap();
        assert_eq!(b.transport().stats().busy_violations, 0);
    }

    #[test]
    fn a_restore_that_stops_says_what_was_written_and_how_to_recover() {
        let control = std::sync::Arc::new(keylume_device::sim::SimControl::default());
        let b = SimKeyboard::keyboard_with(50, control.clone());
        let snap = snapshot(&b).unwrap();
        b.settle();
        // the first two layers go through (a layer switch and 7 pages each), then the cable goes
        control.fail_writes_after(16, u32::MAX);
        let err = restore(&b, &snap).unwrap_err();
        let RestoreError::Partial { done, failed, .. } = &err else { panic!("{err}") };
        assert_eq!(done, &["picture layer 1", "picture layer 2"]);
        assert_eq!(failed, "picture layer 3");
        let text = err.to_string();
        assert!(text.contains("Already restored: picture layer 1, picture layer 2") && text.contains("restore the same backup again"), "{text}");
    }
}
