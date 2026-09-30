//! End-to-end tests of `Keyboard` against the firmware simulator.
//! The simulator reproduces the TK68's busy windows; every test asserts that we never
//! hit one (`busy_violations == 0`) as well as checking the data round-trips.

use keylume_device::sim::SimKeyboard;
use keylume_device::Keyboard;
use keylume_proto::keymap::{consumer, KeyAction, Keymap};
use keylume_proto::macros::{Macro, MacroEvent};
use keylume_proto::picture::Frame;
use keylume_proto::settings::{KeyboardOptions, ReportRate, SleepTimers};
use keylume_proto::stream::BANDS;
use keylume_proto::{Effect, Layout, Mode, Rgb};

/// 50× faster than real time: a 1.1 s settle becomes 22 ms.
const SPEEDUP: u32 = 50;

fn kb() -> Keyboard<SimKeyboard> {
    SimKeyboard::keyboard(SPEEDUP)
}

fn assert_clean(kb: &Keyboard<SimKeyboard>) {
    let s = kb.transport().stats();
    assert_eq!(s.busy_violations, 0, "firmware busy window violated: {s:?}");
    assert_eq!(s.bad_checksums, 0, "checksum rejected: {s:?}");
}

#[test]
fn reads_device_info() {
    let kb = kb();
    assert_eq!(kb.firmware_version().unwrap(), 772);
    assert_eq!(kb.report_rate().unwrap(), ReportRate::Hz1000);
    assert_eq!(kb.debounce().unwrap(), 2);
    assert_eq!(kb.profile().unwrap(), 0);
    assert_eq!(kb.sleep_timers().unwrap().bluetooth, 3600);
    assert_eq!(kb.options().unwrap(), KeyboardOptions::default());
    assert_clean(&kb);
}

#[test]
fn every_effect_round_trips() {
    let kb = kb();
    for m in Mode::ALL {
        let dirs = m.info().directions.len().max(1) as u8;
        let e = Effect { mode: m, speed: 1, brightness: 3, direction: dirs - 1, rainbow: false, color: Rgb(0, 0x40, 0xFF) };
        kb.set_effect(&e).unwrap();
        let back = kb.effect().unwrap();
        assert_eq!(back.mode, m);
        assert_eq!(back.brightness, 3);
        assert_eq!(back.direction, dirs - 1, "{m:?}");
        if m != Mode::UserPicture {
            assert_eq!(back.speed, 1, "{m:?}");
            assert_eq!(back.color, e.color, "{m:?}");
        }
    }
    assert_clean(&kb);
}

#[test]
fn rapid_effect_changes_never_wedge() {
    // Like a user flicking through profiles: back-to-back writes must be paced.
    let kb = kb();
    let colors = ["#0040ff", "#00e5ff", "#1565ff", "#7df9ff", "#3d5afe", "#00a8ff"];
    for (i, c) in colors.iter().cycle().take(12).enumerate() {
        let mode = if i % 2 == 0 { Mode::Static } else { Mode::Breathing };
        kb.set_effect(&Effect { color: c.parse().unwrap(), ..Effect::new(mode) }).unwrap();
    }
    // 12 writes through 6 colours: the last one applied is colours[5].
    assert_eq!(kb.effect().unwrap().color, "#00a8ff".parse::<Rgb>().unwrap());
    assert_clean(&kb);
}

#[test]
fn picture_layers_round_trip_and_stay_separate() {
    let kb = kb();
    let layout = Layout::tk68();
    let frames: Vec<Frame> = (0..3u8)
        .map(|l| {
            let mut f = Frame::default();
            for k in &layout.keys {
                f.set(k.slot, Rgb(l * 80, (k.slot * 2) as u8, 255 - k.slot as u8));
            }
            f
        })
        .collect();
    for (l, f) in frames.iter().enumerate() {
        kb.write_picture(l as u8, f, 4).unwrap();
    }
    for (l, f) in frames.iter().enumerate() {
        assert_eq!(&kb.read_picture(l as u8).unwrap(), f, "layer {l}");
    }
    let e = kb.effect().unwrap();
    assert_eq!((e.mode, e.direction), (Mode::UserPicture, 2));
    assert_clean(&kb);
}

#[test]
fn keymaps_round_trip() {
    let kb = kb();
    let layout = Layout::tk68();
    let mut km = kb.keymap(0).unwrap();
    assert_eq!(km.0[layout.key("esc").unwrap().slot], KeyAction::key(0x29));
    // Caps -> Esc, Esc -> play/pause, F -> macro 2
    km.0[layout.key("caps").unwrap().slot] = KeyAction::key(0x29);
    km.0[layout.key("esc").unwrap().slot] = KeyAction::Consumer { usage: consumer::PLAY_PAUSE };
    km.0[layout.key("f").unwrap().slot] = KeyAction::Macro { index: 2, mode: 0 };
    kb.set_keymap(0, &km).unwrap();
    let back = kb.keymap(0).unwrap();
    // Only 504 of 512 bytes are written; compare the slots that exist.
    for k in &layout.keys {
        assert_eq!(back.0[k.slot], km.0[k.slot], "{}", k.id);
    }
    // Other onboard profiles are untouched.
    assert_eq!(kb.keymap(1).unwrap().0[layout.key("caps").unwrap().slot], KeyAction::key(0x39));

    let mut fnm = Keymap(vec![KeyAction::Disabled; 128]);
    fnm.0[layout.key("1").unwrap().slot] = KeyAction::Consumer { usage: consumer::VOL_DOWN };
    kb.set_fn_keymap(0, &fnm).unwrap();
    assert_eq!(kb.fn_keymap(0).unwrap().0[layout.key("1").unwrap().slot], fnm.0[layout.key("1").unwrap().slot]);
    assert_clean(&kb);
}

#[test]
fn keymap_writes_target_the_right_profile_even_though_firmware_writes_the_active_one() {
    // Regression (found on hardware): the firmware ignores the profile byte on writes.
    let kb = kb();
    let layout = Layout::tk68();
    let caps = layout.key("caps").unwrap().slot;
    assert_eq!(kb.profile().unwrap(), 0);
    let mut km = kb.keymap(2).unwrap();
    km.0[caps] = KeyAction::key(0x29);
    kb.set_keymap(2, &km).unwrap();
    assert_eq!(kb.keymap(2).unwrap().0[caps], KeyAction::key(0x29), "landed in profile 3");
    assert_eq!(kb.keymap(0).unwrap().0[caps], KeyAction::key(0x39), "profile 1 untouched");
    assert_eq!(kb.profile().unwrap(), 0, "active profile restored");
    assert!(kb.set_keymap(3, &km).is_err(), "TK68 has only 3 onboard profiles");
    assert!(kb.set_profile(3).is_err());
    assert_clean(&kb);
}

#[test]
fn macros_round_trip() {
    let kb = kb();
    let m = Macro {
        repeat: 3,
        events: vec![
            MacroEvent::Key { code: 0x0B, down: true },
            MacroEvent::Delay { ms: 30 },
            MacroEvent::Key { code: 0x0B, down: false },
            MacroEvent::Delay { ms: 500 },
            MacroEvent::Key { code: 0x0C, down: true },
            MacroEvent::Delay { ms: 20 },
            MacroEvent::Key { code: 0x0C, down: false },
        ],
    };
    kb.set_macro(5, &m).unwrap();
    assert_eq!(kb.macro_at(5).unwrap(), m);
    assert_eq!(kb.macro_at(4).unwrap(), Macro::default());
    assert_clean(&kb);
}

#[test]
fn settings_round_trip() {
    let kb = kb();
    kb.set_report_rate(0, ReportRate::Hz250).unwrap();
    assert_eq!(kb.report_rate().unwrap(), ReportRate::Hz250);
    kb.set_debounce(5).unwrap();
    assert_eq!(kb.debounce().unwrap(), 5);
    let t = SleepTimers { bluetooth: 600, wireless: 900, deep_bluetooth: 1800, deep_wireless: 3600 };
    kb.set_sleep_timers(t).unwrap();
    assert_eq!(kb.sleep_timers().unwrap(), t);
    let o = KeyboardOptions { win_key_lock: true, wasd_arrows_swap: true, ..Default::default() };
    kb.set_options(0, o).unwrap();
    assert_eq!(kb.options().unwrap(), o);
    kb.set_profile(2).unwrap();
    assert_eq!(kb.profile().unwrap(), 2);
    assert_clean(&kb);
}

#[test]
fn streaming_after_a_mode_change_is_paced() {
    let kb = kb();
    kb.set_effect(&Effect::new(Mode::MusicBars)).unwrap();
    for i in 0..60u8 {
        let mut lv = [0u8; BANDS];
        lv.iter_mut().enumerate().for_each(|(b, l)| *l = (b as u8 + i) % 7);
        kb.stream_levels(&lv).unwrap();
    }
    kb.set_effect(&Effect::new(Mode::ScreenSync)).unwrap();
    kb.stream_color(Rgb(0, 0x80, 0xFF)).unwrap();
    let st = kb.transport().state();
    assert_eq!(st.last_color, [0, 0x80, 0xFF]);
    assert_eq!(kb.transport().stats().stream_frames, 61);
    assert_clean(&kb);
}

#[test]
fn factory_reset_restores_defaults() {
    let kb = kb();
    kb.set_debounce(9).unwrap();
    kb.factory_reset().unwrap();
    assert_eq!(kb.debounce().unwrap(), 2);
    assert_clean(&kb);
}

#[test]
fn recovers_from_a_wedged_channel() {
    let kb = kb();
    kb.set_effect(&Effect::new(Mode::Static)).unwrap();
    // Simulate another program barging in during the busy window.
    let rogue = Effect::new(Mode::Wave).encode().unwrap();
    assert!(kb.raw_send(&rogue).is_err());
    assert!(kb.transport().is_wedged());
    // Our next request must clear the wedge (bare GET) and succeed.
    kb.settle();
    assert_eq!(kb.effect().unwrap().mode, Mode::Static);
    assert!(!kb.transport().is_wedged());
}

#[test]
fn simulator_really_models_the_trap() {
    // A naive client that ignores timing gets refused: proof the tests above mean something.
    let kb = kb();
    let a = Effect::new(Mode::Static).encode().unwrap();
    let b = Effect::new(Mode::Breathing).encode().unwrap();
    kb.raw_send(&a).unwrap();
    assert!(kb.raw_send(&b).is_err(), "back-to-back config writes must be refused");
    assert_eq!(kb.transport().stats().busy_violations, 1);
}

#[test]
fn rejects_bad_checksums() {
    let kb = kb();
    let mut p = Effect::new(Mode::Static).encode().unwrap();
    p[8] ^= 0xFF;
    assert!(kb.raw_send(&p).is_err());
    assert_eq!(kb.transport().stats().bad_checksums, 1);
}
