//! The device service against the firmware simulator (50× speed).

use std::time::Duration;

use keylume_core::backup;
use keylume_core::service::{Connect, Service, ServiceError, Task};
use keylume_core::Store;
use keylume_profiles::{Lighting, SpellWord};
use keylume_proto::{Effect, Layout, Mode, Rgb};

fn service() -> Service {
    let s = Service::start(Connect::Simulator { speedup: 50 }, |_| {});
    for _ in 0..50 {
        if s.status().connected {
            return s;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("simulator never connected");
}

fn violations(s: &Service) -> u32 {
    s.call(|b| Ok(b.sim_stats().unwrap().busy_violations)).unwrap()
}

#[test]
fn connects_and_reports_status() {
    let s = service();
    let st = s.status();
    assert!(st.connected);
    assert_eq!(st.firmware, Some(772));
    assert!(st.device.unwrap().simulated);
}

#[test]
fn applies_an_effect() {
    let s = service();
    let e = Effect { color: Rgb(0, 0x40, 0xFF), ..Effect::new(Mode::Breathing) };
    s.apply_effect(e);
    s.flush().unwrap();
    assert_eq!(s.call(|b| b.effect()).unwrap(), Effect { speed: e.speed, ..e });
    assert_eq!(violations(&s), 0);
}

#[test]
fn rapid_requests_coalesce_to_the_latest() {
    let s = service();
    for i in 0..30u8 {
        s.apply_effect(Effect { color: Rgb(0, i, 255), ..Effect::new(Mode::Static) });
    }
    s.flush().unwrap();
    assert_eq!(s.call(|b| b.effect()).unwrap().color, Rgb(0, 29, 255), "latest wins");
    let applied = s.status().applied;
    assert!(applied < 30, "requests should coalesce, applied {applied}");
    assert_eq!(violations(&s), 0);
}

#[test]
fn per_key_profiles_use_the_layer_cache() {
    let s = service();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path(), &Layout::tk68()).unwrap();
    let p = store.get("deep-ocean-cascade").unwrap().clone();

    s.apply(&p.name, &p.lighting);
    s.flush().unwrap();
    // The picture landed in layer 3 (index 2) and is showing.
    let frame = s.call(|b| b.read_picture(2)).unwrap();
    let Lighting::PerKey { keys, .. } = &p.lighting else { panic!() };
    let layout = Layout::tk68();
    assert_eq!(frame.0[layout.key("esc").unwrap().slot], keys["esc"]);
    // the hidden ISO LEDs light with the key above them instead of staying dark
    assert_eq!(frame.0[75], keys["enter"]);
    assert_eq!(frame.0[10], keys["lshift"]);
    let sets_after_first = s.call(|b| Ok(b.sim_stats().unwrap().sets)).unwrap();

    // Same profile again: only a layer select (+ its read), no 7-page rewrite.
    s.apply(&p.name, &p.lighting);
    s.flush().unwrap();
    let sets_after_second = s.call(|b| Ok(b.sim_stats().unwrap().sets)).unwrap();
    assert!(sets_after_second - sets_after_first < 7, "cache should skip the rewrite");
    assert_eq!(violations(&s), 0);
}

#[test]
fn live_effects_stream_and_stop() {
    let s = service();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path(), &Layout::tk68()).unwrap();
    let p = store.get("live-blue-heartbeat").unwrap().clone();
    s.apply(&p.name, &p.lighting);
    s.flush().unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(s.status().live.as_deref(), Some("Blue Heartbeat"));
    let frames = s.call(|b| Ok(b.sim_stats().unwrap().stream_frames)).unwrap();
    assert!(frames >= 5, "expected a stream of frames, got {frames}");
    assert_eq!(s.call(|b| b.effect()).unwrap().mode, Mode::ScreenSync);

    // Applying a normal effect stops the animation.
    s.apply_effect(Effect::new(Mode::Wave));
    s.flush().unwrap();
    assert_eq!(s.status().live, None);
    let f1 = s.call(|b| Ok(b.sim_stats().unwrap().stream_frames)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let f2 = s.call(|b| Ok(b.sim_stats().unwrap().stream_frames)).unwrap();
    assert_eq!(f1, f2, "no frames after stopping");
    assert_eq!(violations(&s), 0);
}

/// Two words that share a letter (S), like the spells people make on the Create page.
fn spell(words: &[&str]) -> Lighting {
    let colors = [Rgb(0x00, 0xc8, 0xff), Rgb(0xff, 0xb0, 0x00)];
    Lighting::Spell {
        words: words.iter().zip(colors).map(|(t, color)| SpellWord { text: t.to_string(), color }).collect(),
        background: Rgb(0x02, 0x0a, 0x3a),
        brightness: 4,
    }
}

#[test]
fn a_spell_plays_every_frame_then_holds_the_last() {
    let s = service();
    let lighting = spell(&["Keeps", "Glows"]);
    let layout = Layout::tk68();
    let slot = |id: &str| layout.key(id).unwrap().slot;

    s.apply("Spell Keeps & Glows", &lighting);
    s.flush().unwrap();
    assert_eq!(s.status().live.as_deref(), Some("Spell Keeps & Glows"), "shows as running while spelling");
    // After the first frame only K is lit.
    let first = s.call(|b| b.read_picture(2)).unwrap();
    let Lighting::Spell { background, .. } = &lighting else { panic!() };
    assert_ne!(first.0[slot("k")], *background);
    assert_eq!(first.0[slot("g")], *background);

    // 11 frames × ~1.5 s / 50 ≈ 0.3 s at simulator speed
    for _ in 0..100 {
        if s.status().live.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(s.status().live, None, "the spell finishes by itself");
    s.flush().unwrap();
    let last = s.call(|b| b.read_picture(2)).unwrap();
    assert_eq!(last.0[slot("g")], Rgb(0xff, 0xb0, 0x00), "the second word is lit at the end");
    assert_eq!(last.0[slot("s")], Rgb(255, 255, 255), "the shared S glows white");
    assert_eq!(violations(&s), 0);
}

#[test]
fn a_new_request_interrupts_a_spell() {
    let s = service();
    s.apply("Spell Keylume", &spell(&["Keylume"]));
    s.flush().unwrap();
    s.apply_effect(Effect::new(Mode::Meteor));
    s.flush().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(s.call(|b| b.effect()).unwrap().mode, Mode::Meteor, "no more spell frames after the new effect");
    assert_eq!(s.status().live, None);
}

#[test]
fn side_light_is_set_and_repeats_are_skipped() {
    use keylume_proto::{SideLight, SideMode};
    let s = service();
    let side = SideLight::new(SideMode::Breathing, Rgb(0x1f, 0x45, 0xff));
    s.set_side(side);
    s.flush().unwrap();
    assert_eq!(s.call(|b| b.side_light()).unwrap(), side);
    let sets = s.call(|b| Ok(b.sim_stats().unwrap().sets)).unwrap();
    s.set_side(side);
    s.flush().unwrap();
    let after = s.call(|b| Ok(b.sim_stats().unwrap().sets)).unwrap();
    assert_eq!(after - sets, 0, "an identical side light isn't written again");
    // lighting + side in quick succession: both land, no timing violations
    s.apply_effect(Effect::new(Mode::Wave));
    s.set_side(SideLight::new(SideMode::Neon, Rgb(255, 255, 255)));
    s.flush().unwrap();
    assert_eq!(s.call(|b| b.side_light()).unwrap().mode, SideMode::Neon);
    assert_eq!(s.call(|b| b.effect()).unwrap().mode, Mode::Wave);
    assert_eq!(violations(&s), 0);
}

#[test]
fn backup_and_restore_round_trip() {
    let s = service();
    let snap = s.call(|b| backup::snapshot(b)).unwrap();
    assert_eq!(snap.pictures.len(), 3);
    assert_eq!(snap.macros.len(), 16);
    // change things, then restore
    s.call(|b| {
        b.set_debounce(7)?;
        b.set_effect(&Effect::new(Mode::Meteor))
    })
    .unwrap();
    let json = serde_json::to_string(&snap).unwrap();
    let back: backup::Backup = serde_json::from_str(&json).unwrap();
    s.maintain(Task::Restore, move |b| backup::restore(b, &back)).unwrap().unwrap();
    assert_eq!(s.call(|b| b.debounce()).unwrap(), snap.debounce);
    assert_eq!(s.call(|b| b.effect()).unwrap(), snap.effect);
    assert_eq!(violations(&s), 0);
}

#[test]
fn pausing_releases_the_keyboard_and_resuming_reconnects() {
    let s = service();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path(), &Layout::tk68()).unwrap();
    let p = store.get("live-ocean-flow").unwrap().clone();
    s.apply(&p.name, &p.lighting);
    s.flush().unwrap();

    s.pause(Some("EPOMAKER Driver is running".into()));
    s.flush().unwrap();
    let st = s.status();
    assert_eq!(st.paused.as_deref(), Some("EPOMAKER Driver is running"));
    assert!(!st.connected);
    assert_eq!(st.live, None, "live effects stop while paused");
    assert!(matches!(s.call(|b| b.effect()), Err(ServiceError::Paused(_))));
    // Requests made while paused are dropped, not queued up for later.
    s.apply_effect(Effect::new(Mode::Meteor));

    s.pause(None);
    for _ in 0..50 {
        if s.status().connected {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let st = s.status();
    assert!(st.connected && st.paused.is_none());
    assert_ne!(s.call(|b| b.effect()).unwrap().mode, Mode::Meteor);
}

#[test]
fn true_colors_correct_pictures_effects_and_streams_when_on() {
    let s = service();
    s.set_true_colors(true);
    let layout = Layout::tk68();
    let grey = Rgb(0x80, 0x80, 0x80);
    let linear_grey = Rgb(0x37, 0x37, 0x37);

    // a written picture holds linearised values
    let keys: std::collections::BTreeMap<String, Rgb> = layout.keys.iter().map(|k| (k.id.clone(), grey)).collect();
    s.apply("Grey", &Lighting::PerKey { keys, brightness: 4 });
    s.flush().unwrap();
    let frame = s.call(|b| b.read_picture(2)).unwrap();
    assert_eq!(frame.0[layout.key("esc").unwrap().slot], linear_grey, "#808080 should become #373737 on the way to the board");

    // reading the layer back through the service undoes the correction (within ±1)
    let back = s.read_layer(2).unwrap();
    let v = back.0[layout.key("esc").unwrap().slot];
    assert!(v.0.abs_diff(grey.0) <= 1 && v.1.abs_diff(grey.1) <= 1 && v.2.abs_diff(grey.2) <= 1, "{v} should read back close to {grey}");

    // an effect colour is corrected
    s.apply_effect(Effect { color: grey, ..Effect::new(Mode::Breathing) });
    s.flush().unwrap();
    assert_eq!(s.call(|b| b.effect()).unwrap().color, linear_grey);

    // a whole-board live stream colour is corrected
    let live = keylume_live::LiveEffect::PaletteFlow { colors: vec![grey], period: 10.0 };
    s.apply("Grey Flow", &Lighting::Live { live });
    s.flush().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let streamed = s.call(|b| Ok(b.sim_stats().unwrap().last_stream_color)).unwrap();
    assert_eq!(streamed, [linear_grey.0, linear_grey.1, linear_grey.2]);
}

#[test]
fn true_colors_off_leaves_colours_as_sent() {
    let s = service();
    // off is the default: nothing is corrected until the app turns it on
    let layout = Layout::tk68();
    let grey = Rgb(0x80, 0x80, 0x80);

    let keys: std::collections::BTreeMap<String, Rgb> = layout.keys.iter().map(|k| (k.id.clone(), grey)).collect();
    s.apply("Grey", &Lighting::PerKey { keys, brightness: 4 });
    s.flush().unwrap();
    let frame = s.call(|b| b.read_picture(2)).unwrap();
    assert_eq!(frame.0[layout.key("esc").unwrap().slot], grey);

    s.apply_effect(Effect { color: grey, ..Effect::new(Mode::Breathing) });
    s.flush().unwrap();
    assert_eq!(s.call(|b| b.effect()).unwrap().color, grey);

    let live = keylume_live::LiveEffect::PaletteFlow { colors: vec![grey], period: 10.0 };
    s.apply("Grey Flow", &Lighting::Live { live });
    s.flush().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let streamed = s.call(|b| Ok(b.sim_stats().unwrap().last_stream_color)).unwrap();
    assert_eq!(streamed, [grey.0, grey.1, grey.2]);
}

#[test]
fn hardware_mode_without_a_keyboard_reports_not_connected() {
    // CI machines and WSL have no TK68 attached.
    let s = Service::start(Connect::Hardware, |_| {});
    std::thread::sleep(Duration::from_millis(100));
    if !s.status().connected {
        assert!(matches!(s.call(|b| b.effect()), Err(ServiceError::NotConnected)));
    }
}

// ---- lighting state and maintenance ------------------------------------------------------

use keylume_core::lighting::{LightingState, Origin, Phase};
use keylume_core::service::{Listeners, Options};
use std::sync::{Arc, Mutex};

/// A simulator service that records every lighting state it publishes.
fn watched(options: Options) -> (Service, Arc<Mutex<Vec<LightingState>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let s = Service::start_with(
        Connect::Simulator { speedup: 50 },
        options,
        Listeners { lighting: Some(Box::new(move |st: &LightingState| log.lock().unwrap().push(st.clone()))), ..Default::default() },
    );
    wait_for(&s, |st| st.phase == Phase::Applied, "the simulator to connect and be read back");
    (s, seen)
}

/// What was published, in `seq` order. Listeners run on the thread that made the change (the
/// caller for a request, the worker for the rest), so two can arrive the other way round;
/// `seq` is the order, and it's what the app keeps (the newest wins).
fn recorded(seen: &Mutex<Vec<LightingState>>) -> Vec<LightingState> {
    let mut v = seen.lock().unwrap().clone();
    v.sort_by_key(|st| st.seq);
    v
}

fn wait_for(s: &Service, ok: impl Fn(&LightingState) -> bool, what: &str) -> LightingState {
    for _ in 0..200 {
        let st = s.lighting();
        if ok(&st) {
            return st;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("gave up waiting for {what}: {:?}", s.lighting());
}

fn profile(id: &str) -> keylume_profiles::Profile {
    let dir = tempfile::tempdir().unwrap();
    Store::open(dir.path(), &Layout::tk68()).unwrap().get(id).unwrap().clone()
}

#[test]
fn connecting_reads_back_what_the_keyboard_shows() {
    let (s, _) = watched(Options::default());
    let st = s.lighting();
    let shown = st.shown.unwrap();
    assert_eq!(shown.origin, Origin::Keyboard);
    // the simulator starts on a rainbow wave
    assert!(matches!(shown.lighting, Some(Lighting::Effect { effect }) if effect.mode == Mode::Wave && effect.rainbow));
}

#[test]
fn a_look_counts_as_shown_only_once_the_keyboard_took_it() {
    let (s, seen) = watched(Options::default());
    seen.lock().unwrap().clear();
    let p = profile("deep-ocean-flame");
    let id = s.apply_profile(&p);
    s.flush().unwrap();
    let states = recorded(&seen);
    let phases: Vec<_> = states.iter().map(|st| st.phase).collect();
    assert_eq!(phases.first(), Some(&Phase::Requested), "{phases:?}");
    assert!(phases.contains(&Phase::Pending), "{phases:?}");
    assert_eq!(phases.last(), Some(&Phase::Applied), "{phases:?}");
    for st in &states {
        let claims = st.shown.as_ref().is_some_and(|sh| sh.profile_id.as_deref() == Some(p.id.as_str()));
        assert_eq!(claims, st.phase == Phase::Applied, "shown before it was applied: {st:?}");
        assert_eq!(st.request.as_ref().unwrap().id, id);
    }
    assert!(states.windows(2).all(|w| w[0].seq < w[1].seq), "every state has its own seq");
    let shown = s.lighting().shown.unwrap();
    assert_eq!((shown.request, shown.origin), (id, Origin::Profile));
    assert_eq!(violations(&s), 0);
}

#[test]
fn an_older_request_never_overwrites_a_newer_choice() {
    let (s, seen) = watched(Options::default());
    seen.lock().unwrap().clear();
    let picks = ["deep-ocean-cascade", "deep-ocean-flame", "fx-ripple-cyan", "midnight-gamer"].map(profile);
    let ids: Vec<u64> = picks.iter().map(|p| s.apply_profile(p)).collect();
    s.flush().unwrap();
    let last = *ids.last().unwrap();
    let st = s.lighting();
    assert_eq!((st.phase, st.request.as_ref().unwrap().id, st.shown.as_ref().unwrap().request), (Phase::Applied, last, last));
    // once the newest request was made, no state names an older one as the request
    let states = recorded(&seen);
    let from = states.iter().position(|st| st.request.as_ref().is_some_and(|r| r.id == last)).unwrap();
    assert!(states[from..].iter().all(|st| st.request.as_ref().unwrap().id == last));
    // and what's shown never goes back to an older pick once the newest is up
    let up = states.iter().position(|st| st.shown.as_ref().is_some_and(|sh| sh.request == last)).unwrap();
    assert!(states[up..].iter().all(|st| st.shown.as_ref().is_some_and(|sh| sh.request == last)));
    assert_eq!(violations(&s), 0);
}

#[test]
fn a_failed_write_is_reported_and_the_keyboard_comes_back() {
    let (s, seen) = watched(Options::default());
    let sim = s.simulator().unwrap();
    seen.lock().unwrap().clear();
    sim.fail_next_writes(2); // the write and its retry
    let id = s.apply_effect(Effect::new(Mode::Meteor));
    s.flush().unwrap();
    let states = recorded(&seen);
    assert!(states.iter().any(|st| st.error.is_some() && st.request.as_ref().is_some_and(|r| r.id == id)), "the failure is reported: {states:?}");
    assert!(states.iter().all(|st| st.shown.as_ref().is_none_or(|sh| sh.request != id)), "never claimed as shown");
    // it reconnects by itself and reads back what the keyboard has
    let st = wait_for(&s, |st| st.phase == Phase::Applied, "the reconnect");
    assert_eq!(st.shown.unwrap().origin, Origin::Keyboard);
}

#[test]
fn unplugging_and_replugging_are_followed() {
    let (s, _) = watched(Options::default());
    let sim = s.simulator().unwrap();
    sim.set_unplugged(true);
    let st = wait_for(&s, |st| st.phase == Phase::Disconnected, "the unplug to be noticed");
    assert!(st.shown.is_none(), "nothing is claimed while it's gone");
    let id = s.apply_profile(&profile("deep-ocean-flame"));
    s.flush().unwrap();
    let st = s.lighting();
    assert_eq!((st.phase, st.request.unwrap().id), (Phase::Disconnected, id));
    assert!(st.error.is_some(), "the request made while unplugged failed");
    sim.set_unplugged(false);
    let st = wait_for(&s, |st| st.phase == Phase::Applied, "the replug");
    assert_eq!(st.shown.unwrap().origin, Origin::Keyboard);
}

#[test]
fn stopping_holds_the_last_frame_and_frames_match_what_was_sent() {
    let (s, _) = watched(Options { true_colors: true, ..Options::default() });
    let p = profile("live-ocean-flow");
    let id = s.apply_profile(&p);
    s.flush().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let st = s.lighting();
    assert!(st.shown.as_ref().unwrap().running);
    let frame = s.frames().latest().unwrap();
    assert_eq!(frame.request, id);
    assert!(frame.seq >= 3, "a stream of frames: {}", frame.seq);
    s.stop_live();
    s.flush().unwrap();
    let st = s.lighting();
    let shown = st.shown.unwrap();
    assert!(!shown.running && shown.profile_id.as_deref() == Some(p.id.as_str()), "still the live profile, now stopped");
    // the last frame published is the last one the keyboard got (in design colours)
    let last = s.frames().latest().unwrap();
    let keylume_live::LiveFrame::Color(c) = last.frame else { panic!("a whole-board colour") };
    let sent = s.call(|b| Ok(b.sim_stats().unwrap().last_stream_color)).unwrap();
    let led = keylume_core::color::to_led(c);
    assert_eq!(sent, [led.0, led.1, led.2], "the window gets the colour before True colours; the keys got it after");
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(s.frames().latest().unwrap().seq, last.seq, "no frames after stopping");
}

#[test]
fn lights_off_is_its_own_state() {
    let (s, _) = watched(Options::default());
    s.apply_profile(&profile("deep-ocean-flame"));
    s.lights_off();
    s.flush().unwrap();
    let st = s.lighting();
    assert_eq!((st.phase, st.shown.unwrap().origin), (Phase::Off, Origin::Off));
    assert_eq!(s.call(|b| b.effect()).unwrap().mode, Mode::Off);
}

#[test]
fn a_spell_reports_real_progress_and_holds_the_end() {
    let (s, seen) = watched(Options::default());
    seen.lock().unwrap().clear();
    let id = s.apply("Spell Keeps & Glows", &spell(&["Keeps", "Glows"]));
    s.flush().unwrap();
    let st = wait_for(&s, |st| st.shown.as_ref().is_some_and(|sh| sh.request == id && !sh.running), "the spell to finish");
    let p = st.shown.unwrap().progress.unwrap();
    assert_eq!((p.done, p.total), (11, 11));
    let done: Vec<u32> = recorded(&seen).iter().filter_map(|st| st.shown.as_ref()?.progress.map(|p| p.done)).collect();
    assert!(done.windows(2).all(|w| w[0] <= w[1]), "progress only moves forward: {done:?}");
    assert_eq!(done.first(), Some(&1));
}

#[test]
fn nothing_else_touches_the_keyboard_during_a_restore() {
    let (s, _) = watched(Options::default());
    let s = Arc::new(s);
    // backed up while the simulator shows its own wave; then a design goes on layer 3
    let snap = s.maintain(Task::Backup, |b| backup::snapshot(b)).unwrap().unwrap();
    let p = profile("deep-ocean-flame");
    s.apply_profile(&p);
    s.flush().unwrap();
    let runner = {
        let s = s.clone();
        let snap = snap.clone();
        std::thread::spawn(move || s.maintain(Task::Restore, move |b| backup::restore(b, &snap)).unwrap())
    };
    let busy = (0..200).any(|_| {
        std::thread::sleep(Duration::from_millis(2));
        s.status().busy.is_some()
    });
    assert!(busy, "the status says it's busy");
    assert!(matches!(s.call(|b| b.effect()), Err(ServiceError::Busy(_))), "other calls are refused");
    assert!(matches!(s.maintain(Task::Reset, |b| b.factory_reset()), Err(ServiceError::Busy(_))), "one maintenance job at a time");
    let refused = s.apply_effect(Effect::new(Mode::Meteor));
    let st = s.lighting();
    assert_eq!(st.request.as_ref().unwrap().id, refused);
    assert_eq!(st.phase, Phase::Failed, "a lighting request is refused, not queued behind the restore");
    runner.join().unwrap().unwrap();
    assert!(s.status().busy.is_none());
    let st = s.lighting();
    assert_eq!(st.shown.unwrap().origin, Origin::Restored);
    assert_ne!(s.call(|b| b.effect()).unwrap().mode, Mode::Meteor);
    // the restore blanked layer 3 again, and the cache knows it: the design is rewritten
    let writes = s.call(|b| Ok(b.sim_stats().unwrap().writes)).unwrap();
    s.apply_profile(&p);
    s.flush().unwrap();
    let after = s.call(|b| Ok(b.sim_stats().unwrap().writes)).unwrap();
    assert!(after - writes >= 7, "rewritten after the restore ({} writes)", after - writes);
    assert_eq!(violations(&s), 0);
}

#[test]
fn a_bad_backup_writes_nothing_through_the_service() {
    let (s, _) = watched(Options::default());
    let snap = s.maintain(Task::Backup, |b| backup::snapshot(b)).unwrap().unwrap();
    s.flush().unwrap();
    let writes = || s.call(|b| Ok(b.sim_stats().unwrap().writes)).unwrap();
    let before = writes();
    let bad = backup::Backup { firmware: 0x0999, ..snap };
    let r = s.maintain(Task::Restore, move |b| backup::restore(b, &bad)).unwrap();
    assert!(matches!(r, Err(backup::RestoreError::Refused(_))), "{r:?}");
    assert_eq!(writes(), before, "not a single write");
}

// ---- a keyboard that describes its own lights (HID LampArray) -------------------------

use keylume_core::service::{Demo, SPELL_STEP};

fn lamparray_service() -> Service {
    let options = Options { demo: Demo::LampArray, ..Options::default() };
    let s = Service::start_with(Connect::Simulator { speedup: 50 }, options, Listeners::default());
    for _ in 0..50 {
        if s.status().connected {
            return s;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("simulator never connected");
}

/// Every lamp's colour on the simulated LampArray keyboard.
fn lamps(s: &Service) -> Vec<Rgb> {
    s.simulator().unwrap().lamparray().unwrap().colors().iter().map(|c| Rgb(c.r, c.g, c.b)).collect()
}

#[test]
fn a_lamparray_keyboard_brings_its_own_layout_and_features() {
    let s = lamparray_service();
    let st = s.status();
    let device = st.device.unwrap();
    assert!(device.features.host_driven && device.features.per_key && !device.features.side_light && !device.features.keymap);
    assert_eq!(st.firmware, None, "the standard has no firmware version");
    let layout = s.layout();
    assert_eq!(layout.keys.len(), 104, "the full-size keyboard's own lamps, not the TK68");
    assert!(layout.key("kp5").is_some());
    assert!(s.lighting().shown.is_none(), "nothing is known about its lights until Keylume sets them");
}

#[test]
fn designs_show_at_once_on_a_lamparray_keyboard() {
    let s = lamparray_service();
    let layout = s.layout();
    let keys: std::collections::BTreeMap<String, Rgb> = layout.keys.iter().map(|k| (k.id.clone(), Rgb(0, 0x80, 0xFF))).collect();
    let id = s.apply("Blue", &Lighting::PerKey { keys, brightness: 4 });
    s.flush().unwrap();
    let st = s.lighting();
    assert_eq!(st.shown.as_ref().map(|x| x.request), Some(id));
    let kp5 = layout.key("kp5").unwrap().slot;
    assert_eq!(lamps(&s)[kp5], Rgb(0, 0x80, 0xFF), "the keypad a TK68 doesn't have is lit too");
    s.lights_off();
    s.flush().unwrap();
    assert!(lamps(&s).iter().all(|c| *c == Rgb::BLACK));
}

#[test]
fn keylume_draws_the_animations_a_lamparray_keyboard_lacks() {
    let s = lamparray_service();
    s.apply_effect(Effect { color: Rgb(0, 0xC0, 0xFF), speed: 4, ..Effect::new(Mode::Wave) });
    s.flush().unwrap();
    let first = lamps(&s);
    std::thread::sleep(Duration::from_millis(400));
    assert_ne!(lamps(&s), first, "the wave moves: Keylume draws it frame by frame");
    // typing animations need the keys, which Keylume doesn't watch: refused, nothing sent
    let id = s.apply_effect(Effect::new(Mode::Reactive));
    s.flush().unwrap();
    let st = s.lighting();
    assert_eq!(st.request.map(|r| r.id), Some(id));
    assert!(st.error.unwrap_or_default().contains("can't show the Reactive animation"));
    assert!(st.shown.is_some(), "what it showed before is still known");
}

#[test]
fn spells_step_at_a_readable_pace_on_a_lamparray_keyboard() {
    let s = lamparray_service();
    let words = vec![SpellWord { text: "abc".into(), color: Rgb(255, 0, 0) }];
    s.apply("abc", &Lighting::Spell { words, background: Rgb::BLACK, brightness: 4 });
    s.flush().unwrap();
    let t = std::time::Instant::now();
    while s.lighting().shown.is_some_and(|x| x.running) && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let took = t.elapsed();
    assert!(took >= SPELL_STEP * 2, "three pictures take two steps, not a flash: {took:?}");
    let a = s.layout().key("a").unwrap().slot;
    assert_eq!(lamps(&s)[a], Rgb(255, 0, 0), "the last picture stays up");
}
