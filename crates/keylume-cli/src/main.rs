//! `keylume-cli` — poke the keyboard from a terminal. Handy for protocol work.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use hidapi::HidApi;
use keylume_device::{hid, sim::SimKeyboard, Keyboard, Transport};
use keylume_proto::{picture::Frame, Effect, Layout, Mode, Rgb};

#[derive(Parser)]
#[command(name = "keylume-cli", version, about = "Control a keylume-supported keyboard")]
struct Cli {
    /// Talk to the built-in firmware simulator instead of real hardware
    #[arg(long, global = true)]
    sim: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Design packs: make a signing key, build and sign a pack, check one (no keyboard needed)
    Pack {
        #[command(subcommand)]
        cmd: PackCmd,
    },
    /// List devices with a supported keyboard's USB ids, and whether each passed the
    /// identity checks (strings and HID descriptor; read-only)
    List,
    /// Show firmware, effect and settings
    Info,
    /// Set a lighting effect, e.g. `effect breathing --color #0040ff --speed 1`
    Effect {
        mode: String,
        #[arg(long, default_value = "#0040ff")]
        color: String,
        #[arg(long, default_value_t = 2)]
        speed: u8,
        #[arg(long, default_value_t = 4)]
        brightness: u8,
        #[arg(long, default_value_t = 0)]
        direction: u8,
        #[arg(long)]
        rainbow: bool,
    },
    /// Write a vertical gradient into picture layer N (1-3) and verify it by readback
    Gradient {
        layer: u8,
        #[arg(default_value = "#00fff0")]
        top: String,
        #[arg(default_value = "#0012b0")]
        bottom: String,
    },
    /// Dump picture layer N (1-3) as JSON (key id -> colour)
    ReadPicture { layer: u8 },
    /// Write a JSON file (key id -> colour, as produced by read-picture) into layer N (1-3);
    /// `"slot:N": colour` sets one matrix slot directly, such as a hidden LED
    WritePicture { layer: u8, file: std::path::PathBuf },
    /// Show the hidden ISO LEDs (under Enter, below A) in each picture layer. `--off` turns
    /// them off in layers 1-2 (the factory pictures), saving each layer to
    /// `layerN-before-hidden-off.json` first (restore: `write-picture N <file>`)
    HiddenLeds {
        #[arg(long)]
        off: bool,
    },
    /// Diagnostics: switch to picture layer N and time how long until the keyboard answers again
    SwitchTest { layer: u8 },
    /// Diagnostics: write a solid-colour frame to the current layer and time recovery
    WriteTiming {
        #[arg(default_value = "#0040ff")]
        color: String,
        #[arg(long, default_value_t = 20)]
        gap_ms: u64,
        /// quiet time after the last page before polling
        #[arg(long, default_value_t = 1500)]
        wait_ms: u64,
    },
    /// Diagnostics: run a step sequence and log each step. Steps:
    /// `sleep:MS`, `led` (read 0x87), `pic:PAGE` (read 0x8C page), `write:#RRGGBB` (7 pages, no wait),
    /// `layer:N` (select picture layer 1-3, no wait), `wait-ready` (poll 0x87 every 2 s, max 60 s)
    Seq { steps: Vec<String> },
    /// Stream a synthetic animation through the music-bars mode for SECONDS
    Stream {
        #[arg(default_value_t = 5)]
        seconds: u64,
        #[arg(long, default_value_t = 33)]
        frame_ms: u64,
        #[arg(long, default_value = "#0060ff")]
        color: String,
        /// music mode variant: bars (0x14) or pulse (0x16)
        #[arg(long, default_value = "bars")]
        mode: String,
        #[arg(long, default_value_t = 0)]
        direction: u8,
    },
    /// Diagnostics: send COUNT static-colour effects GAP ms apart, then read back
    Burst {
        #[arg(default_value_t = 10)]
        count: usize,
        #[arg(default_value_t = 50)]
        gap_ms: u64,
    },
    /// Diagnostics: send one read request (hex bytes, e.g. `8c 00 00`) and show each attempt
    Raw { bytes: Vec<String> },
    /// Run a live (host-driven) preset for SECONDS, e.g. `live "Blue Heartbeat" 10`.
    /// `live list` shows the presets.
    Live {
        name: String,
        #[arg(default_value_t = 10)]
        seconds: u64,
    },
    /// Remap one key on onboard profile P: `remap 3 caps 0x29` (hex or decimal HID usage)
    Remap {
        profile: u8,
        key: String,
        code: String,
        /// diagnostics: send pages raw with this gap (ms) instead of the library's timing
        #[arg(long)]
        gap_ms: Option<u64>,
        /// diagnostics: silence after the raw pages before reading back
        #[arg(long, default_value_t = 1500)]
        settle_ms: u64,
    },
    /// Switch the active onboard profile (0-2) and read it back
    Profile { profile: u8 },
    /// Set keyboard options, e.g. `options --win-lock true`
    Options {
        #[arg(long)]
        win_lock: Option<bool>,
        #[arg(long)]
        wasd_swap: Option<bool>,
    },
    /// Dump the base key map of onboard profile P as JSON
    Keymap {
        #[arg(default_value_t = 0)]
        profile: u8,
    },
}

#[derive(Subcommand)]
enum PackCmd {
    /// Make a publisher key: the secret goes to FILE (keep it private and backed up), the
    /// public half is printed for the app's list of publishers
    Keygen { file: std::path::PathBuf },
    /// Build DIR (`pack.json` plus theme collections, e.g. `themes.json`, and designs made
    /// in Keylume in `designs.json`) into a `.keylumepack`, checked as the app will check
    /// it; signed when a key is given
    Build {
        dir: std::path::PathBuf,
        /// Where to write it (default: `<id>-<version>.keylumepack` here)
        #[arg(short, long)]
        out: Option<std::path::PathBuf>,
        /// Secret key file from `pack keygen`
        #[arg(long)]
        key: Option<std::path::PathBuf>,
        /// The key's id in the app's list of publishers
        #[arg(long, default_value = "keylume-2026")]
        key_id: String,
    },
    /// Check a `.keylumepack` as the app would when it's added
    Check { file: std::path::PathBuf },
}

/// Ids and names the built-in library already uses (a pack must not reuse them).
fn library_names() -> std::collections::HashSet<String> {
    keylume_core::packs::taken_by(&keylume_profiles::builtin(&Layout::tk68()))
}

/// Who a pack is from, as the Library would say it.
fn who(i: &keylume_core::packs::PackInfo) -> String {
    match (&i.maker_key, i.official) {
        (_, true) => format!("official, by {}", i.publisher),
        (Some(key), _) => format!("community, by {}, signed with maker key {key}", i.publisher),
        (None, _) => format!("community, unsigned, says it's by {}", i.publisher),
    }
}

/// What's in a pack, in a few words.
fn contents(i: &keylume_core::packs::PackInfo) -> String {
    let mut parts = Vec::new();
    if i.themes > 0 {
        parts.push(format!("{} themes", i.themes));
    }
    if i.designs > 0 {
        parts.push(format!("{} designs", i.designs));
    }
    format!("{} in {}", parts.join(" and "), i.collections.join(", "))
}

fn pack(cmd: PackCmd) -> Result<()> {
    use keylume_core::packs;
    match cmd {
        PackCmd::Keygen { file } => {
            if file.exists() {
                bail!("{} exists already: a new key would leave packs signed with the old one unverifiable", file.display());
            }
            let (secret, public) = packs::new_key().map_err(anyhow::Error::msg)?;
            std::fs::write(&file, format!("{secret}\n"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600))?;
            }
            println!("secret key written to {} (keep it private, and back it up)", file.display());
            println!("public key: {public}");
            println!("add it to PUBLISHERS in crates/keylume-core/src/packs.rs");
        }
        PackCmd::Build { dir, out, key, key_id } => {
            let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("pack.json")).context("pack.json")?)?;
            let mut pack = manifest.as_object().context("pack.json should be a JSON object")?.clone();
            let mut collections = Vec::new();
            let mut files: Vec<_> = std::fs::read_dir(&dir)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
            files.sort();
            for f in files.iter().filter(|f| f.extension().is_some_and(|x| x == "json")) {
                let json =
                    || -> Result<serde_json::Value> { serde_json::from_str(&std::fs::read_to_string(f)?).with_context(|| f.display().to_string()) };
                match f.file_name().and_then(|n| n.to_str()) {
                    Some("pack.json") => {}
                    Some("designs.json") => {
                        pack.insert("designs".into(), json()?);
                    }
                    _ => collections.push(json()?),
                }
            }
            pack.insert("keylumePack".into(), packs::FORMAT.into());
            if !collections.is_empty() {
                pack.insert("collections".into(), collections.into());
            }
            let mut json = serde_json::to_string_pretty(&pack)?;
            if let Some(key) = key {
                json = packs::sign(&json, &key_id, &std::fs::read_to_string(key)?).map_err(anyhow::Error::msg)?;
            }
            let checked = packs::parse(json.as_bytes(), &library_names()).map_err(|e| anyhow::anyhow!("the pack isn't ready: {e}"))?;
            let i = &checked.info;
            let out = out.unwrap_or_else(|| format!("{}-{}.{}", i.id, i.version, packs::EXTENSION).into());
            std::fs::write(&out, json)?;
            println!("{}: {} {} ({}), {}", out.display(), i.name, i.version, who(i), contents(i));
        }
        PackCmd::Check { file } => {
            let p = packs::parse(&std::fs::read(&file)?, &library_names()).map_err(|e| anyhow::anyhow!("{}: {e}", file.display()))?;
            let i = &p.info;
            println!("ok {}: {} {} ({}), {}", file.display(), i.name, i.version, who(i), contents(i));
        }
    }
    Ok(())
}

fn mode_by_name(name: &str) -> Result<Mode> {
    let want = name.to_lowercase().replace(['-', '_', ' '], "");
    Mode::ALL
        .into_iter()
        .find(|m| {
            let s = serde_json::to_string(m).unwrap().trim_matches('"').replace('-', "");
            s == want || m.info().name.to_lowercase().replace(' ', "") == want
        })
        .with_context(|| format!("unknown mode {name:?}"))
}

/// A picture as `read-picture` prints it: key id -> colour, plus `slot:N` for hidden LEDs.
fn picture_map(layout: &Layout, f: &Frame) -> serde_json::Map<String, serde_json::Value> {
    let mut map: serde_json::Map<_, _> = layout.keys.iter().map(|k| (k.id.clone(), f.0[k.slot].to_string().into())).collect();
    for h in &layout.hidden_leds {
        map.insert(format!("slot:{}", h.slot), f.0[h.slot].to_string().into());
    }
    map
}

/// `hidden-leds`: report each layer's hidden LEDs and, with `off`, turn them off in the
/// factory layers (1-2; layer 3 is Keylume's own), then put the effect back as it was.
fn hidden_leds<T: Transport>(kb: &Keyboard<T>, layout: &Layout, off: bool, save_dir: &std::path::Path) -> Result<Vec<String>> {
    let before = kb.effect()?;
    let mut out = vec![];
    for layer in 1..=3u8 {
        let f = kb.read_picture(layer - 1)?;
        let lit: Vec<_> = layout.hidden_leds.iter().map(|h| format!("slot {} {}", h.slot, f.0[h.slot])).collect();
        out.push(format!("layer {layer}: {}", lit.join(", ")));
        if !off || layer == 3 || layout.hidden_leds.iter().all(|h| f.0[h.slot] == Rgb::BLACK) {
            continue;
        }
        let file = save_dir.join(format!("layer{layer}-before-hidden-off.json"));
        std::fs::write(&file, serde_json::to_string_pretty(&picture_map(layout, &f))?)?;
        let mut g = f.clone();
        for h in &layout.hidden_leds {
            g.set(h.slot, Rgb::BLACK);
        }
        kb.write_picture(layer - 1, &g, before.brightness)?;
        std::thread::sleep(std::time::Duration::from_millis(200));
        let back = kb.read_picture(layer - 1)?;
        let slots: Vec<_> = layout.keys.iter().map(|k| k.slot).chain(layout.hidden_leds.iter().map(|h| h.slot)).collect();
        let ok = slots.iter().all(|&i| back.0[i] == g.0[i]);
        out.push(format!("  turned off (saved {}); readback {}", file.display(), if ok { "matches" } else { "DIFFERS" }));
    }
    kb.set_effect(&before)?;
    Ok(out)
}

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgb(f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Cmd::Pack { cmd } = cli.cmd {
        return pack(cmd);
    }
    if cli.sim {
        let kb = SimKeyboard::keyboard(1);
        return run(&kb, cli.cmd);
    }
    let mut api = HidApi::new()?;
    if let Cmd::List = cli.cmd {
        // read-only: strings and HID descriptors, nothing is sent to the devices
        let found = hid::identify_all(&api);
        if found.is_empty() {
            println!("no device with a supported keyboard's USB ids is connected");
        }
        for (c, verdict) in found {
            let id = format!("{:04x}:{:04x} {:?} by {:?} ({})", c.vid, c.pid, c.product, c.manufacturer, c.path);
            match verdict {
                Ok(m) => println!("{}: verified {id}", m.name),
                Err(why) => println!("not used: {id}: {why}"),
            }
        }
        return Ok(());
    }
    let kb = hid::open(&mut api)?;
    run(&kb, cli.cmd)
}

fn run<T: Transport>(kb: &Keyboard<T>, cmd: Cmd) -> Result<()> {
    let layout = kb.layout.clone();
    match cmd {
        Cmd::Pack { cmd } => pack(cmd)?,
        Cmd::List => println!("{} ({})", kb.info.product, kb.info.path),
        Cmd::Info => {
            println!("device      {} ({}, {})", kb.info.product, kb.info.name, kb.info.board);
            println!("firmware    {}", kb.firmware_version()?);
            println!("effect      {}", serde_json::to_string(&kb.effect()?)?);
            println!("report rate {} Hz", kb.report_rate()?.hz());
            println!("debounce    {}", kb.debounce()?);
            println!("profile     {}", kb.profile()?);
            println!("sleep       {}", serde_json::to_string(&kb.sleep_timers()?)?);
            println!("options     {}", serde_json::to_string(&kb.options()?)?);
        }
        Cmd::Live { name, seconds } => {
            use keylume_live::{presets, Animator, Inputs, LiveFrame};
            use std::time::{Duration, Instant};
            if name == "list" {
                for (n, d, _) in presets() {
                    println!("{n:18} {d}");
                }
                return Ok(());
            }
            let (_, _, fx) = presets()
                .into_iter()
                .find(|(n, _, _)| n.eq_ignore_ascii_case(&name))
                .with_context(|| format!("no preset {name:?}; try `live list`"))?;
            kb.set_effect(&fx.base_effect())?;
            let mut anim = Animator::new(fx);
            let t0 = Instant::now();
            let (mut ok, mut err) = (0u32, 0u32);
            // The CLI has no audio/screen capture: feed a synthetic beat and a CPU ramp.
            while t0.elapsed() < Duration::from_secs(seconds) {
                let t = t0.elapsed().as_secs_f32();
                let beat = (t * 2.0).fract();
                let mut bands = [0f32; keylume_proto::stream::BANDS];
                for (i, b) in bands.iter_mut().enumerate() {
                    *b = ((1.0 - beat) * (1.0 - i as f32 / 40.0)).max(0.0);
                }
                let inputs =
                    Inputs { bands: Some(bands), loudness: Some(1.0 - beat), cpu: Some((t / seconds as f32).min(1.0)), ..Default::default() };
                let r = match anim.frame(t, &inputs) {
                    LiveFrame::Color(c) => kb.stream_color(c),
                    LiveFrame::Levels(l) => kb.stream_levels(&l),
                };
                if r.is_ok() {
                    ok += 1
                } else {
                    err += 1
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            println!("{name}: {ok} frames, {err} errors, {:.1} fps", ok as f32 / t0.elapsed().as_secs_f32());
        }
        Cmd::Remap { profile, key, code, gap_ms, settle_ms } => {
            let code = if let Some(h) = code.strip_prefix("0x") { u8::from_str_radix(h, 16)? } else { code.parse()? };
            let slot = layout.key(&key).with_context(|| format!("no key {key:?}"))?.slot;
            let t = std::time::Instant::now();
            let mut km = kb.keymap(profile)?;
            println!("read  {:?} ({:?})", km.0[slot], t.elapsed());
            km.0[slot] = keylume_proto::KeyAction::key(code);
            let t = std::time::Instant::now();
            if let Some(gap) = gap_ms {
                for (i, p) in km.write_packets(profile).iter().enumerate() {
                    let a = std::time::Instant::now();
                    let r = kb.raw_send(p);
                    println!("  page {i}: {r:?} in {:?}", a.elapsed());
                    std::thread::sleep(std::time::Duration::from_millis(gap));
                }
                std::thread::sleep(std::time::Duration::from_millis(settle_ms));
            } else {
                kb.set_keymap(profile, &km)?;
            }
            println!("wrote ({:?})", t.elapsed());
            let t = std::time::Instant::now();
            println!("back  {:?} ({:?})", kb.keymap(profile)?.0[slot], t.elapsed());
        }
        Cmd::Profile { profile } => {
            kb.set_profile(profile)?;
            println!("profile now {}", kb.profile()?);
        }
        Cmd::Options { win_lock, wasd_swap } => {
            let mut o = kb.options()?;
            if let Some(v) = win_lock {
                o.win_key_lock = v;
            }
            if let Some(v) = wasd_swap {
                o.wasd_arrows_swap = v;
            }
            let profile = kb.profile()?;
            kb.set_options(profile, o)?;
            println!("options now {}", serde_json::to_string(&kb.options()?)?);
        }
        Cmd::Effect { mode, color, speed, brightness, direction, rainbow } => {
            let e = Effect { mode: mode_by_name(&mode)?, speed, brightness, direction, rainbow, color: color.parse()? };
            kb.set_effect(&e)?;
            let back = kb.effect()?;
            println!("set  {}\nread {}", serde_json::to_string(&e)?, serde_json::to_string(&back)?);
        }
        Cmd::Gradient { layer, top, bottom } => {
            if !(1..=3).contains(&layer) {
                bail!("layer must be 1-3");
            }
            let (top, bottom): (Rgb, Rgb) = (top.parse()?, bottom.parse()?);
            let frame = layout.frame(|k| Some(lerp(top, bottom, k.y / (layout.height - 1.0))));
            let t = std::time::Instant::now();
            kb.write_picture(layer - 1, &frame, 4)?;
            let wrote = t.elapsed();
            std::thread::sleep(std::time::Duration::from_millis(200));
            let back = kb.read_picture(layer - 1)?;
            let ok = layout.keys.iter().filter(|k| back.0[k.slot] == frame.0[k.slot]).count();
            println!("wrote layer {layer} in {wrote:?}; readback {ok}/{} keys match", layout.keys.len());
        }
        Cmd::HiddenLeds { off } => {
            for line in hidden_leds(kb, &layout, off, std::path::Path::new("."))? {
                println!("{line}");
            }
        }
        Cmd::ReadPicture { layer } => {
            let f = kb.read_picture(layer.saturating_sub(1))?;
            println!("{}", serde_json::to_string_pretty(&picture_map(&layout, &f))?);
        }
        Cmd::WritePicture { layer, file } => {
            if !(1..=3).contains(&layer) {
                bail!("layer must be 1-3");
            }
            let map: std::collections::HashMap<String, Rgb> = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
            let mut frame = layout.frame(|k| map.get(&k.id).copied());
            // `slot:N` entries set one matrix slot directly (e.g. a hidden LED)
            for (id, c) in &map {
                if let Some(n) = id.strip_prefix("slot:") {
                    frame.set(n.parse().with_context(|| format!("bad slot {id:?}"))?, *c);
                }
            }
            kb.write_picture(layer - 1, &frame, 4)?;
            std::thread::sleep(std::time::Duration::from_millis(200));
            let back = kb.read_picture(layer - 1)?;
            let ok = layout.keys.iter().filter(|k| back.0[k.slot] == frame.0[k.slot]).count();
            println!("wrote {} into layer {layer}; readback {ok}/{} keys match", file.display(), layout.keys.len());
        }
        Cmd::WriteTiming { color, gap_ms, wait_ms } => {
            let frame = Frame::filled(color.parse()?);
            let t = std::time::Instant::now();
            for (i, p) in frame.write_packets().iter().enumerate() {
                match kb.raw_send(p) {
                    Ok(()) => println!("{:>7.1?} page {i} sent", t.elapsed()),
                    Err(e) => {
                        println!("{:>7.1?} page {i} FAILED: {e}", t.elapsed());
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(gap_ms));
            }
            std::thread::sleep(std::time::Duration::from_millis(wait_ms));
            for i in 0..6 {
                let r = kb.try_effect_once();
                println!("{:>7.1?} poll {i}: {:?}", t.elapsed(), r.as_ref().map(|e| e.mode));
                if r.is_ok() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2000));
            }
        }
        Cmd::Seq { steps } => {
            use std::time::{Duration, Instant};
            let t = Instant::now();
            for s in steps {
                let (op, arg) = s.split_once(':').unwrap_or((s.as_str(), ""));
                let res: String = match op {
                    "sleep" => {
                        std::thread::sleep(Duration::from_millis(arg.parse()?));
                        "ok".into()
                    }
                    "led" => format!("{:?}", kb.try_effect_once().map(|e| (e.mode, e.direction))),
                    "pic" => {
                        let req = keylume_proto::packet::read_request(0x0C, 0, arg.parse()?);
                        match kb.raw_exchange(&req) {
                            Ok(r) => format!("{:02x?}", &r[..12]),
                            Err(e) => e,
                        }
                    }
                    "write" => {
                        let f = Frame::filled(arg.parse()?);
                        let mut out = "ok".to_string();
                        for p in f.write_packets() {
                            if let Err(e) = kb.raw_send(&p) {
                                out = e;
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        out
                    }
                    "send8" => {
                        // send8:08,01,00,04,07,ff,00,00 -> 64-byte SET with a CK8 checksum (LED-style)
                        let mut b = [0u8; 64];
                        for (i, x) in arg.split(',').enumerate() {
                            b[i] = u8::from_str_radix(x, 16)?;
                        }
                        keylume_proto::packet::ck8(&mut b);
                        format!("{:?}", kb.raw_send(&b))
                    }
                    "read" => {
                        // read:88 -> SET [cmd] with CK7, then GET
                        let req = keylume_proto::packet::with_ck7(&[u8::from_str_radix(arg, 16)?]);
                        match kb.raw_exchange(&req) {
                            Ok(r) => format!("{:02x?}", &r[..12]),
                            Err(e) => e,
                        }
                    }
                    "get" => match kb.raw_fetch() {
                        Ok(r) => format!("{:02x?}", &r[..12]),
                        Err(e) => e,
                    },
                    "fx" => {
                        // fx:MODE:#rrggbb
                        let (m, c) = arg.split_once(':').unwrap_or((arg, "#0040ff"));
                        let e = Effect { color: c.parse()?, ..Effect::new(mode_by_name(m)?) };
                        let t0 = Instant::now();
                        let r = kb.raw_send(&e.encode()?);
                        format!("{r:?} (send took {:?})", t0.elapsed())
                    }
                    "poll" => {
                        // poll:MS  -> read 0x87 every 100 ms for up to MS, report first success
                        let limit: u64 = arg.parse()?;
                        let t0 = Instant::now();
                        let mut out = String::from("no answer");
                        while t0.elapsed() < Duration::from_millis(limit) {
                            let a = Instant::now();
                            let r = kb.try_effect_once();
                            if let Ok(e) = r {
                                out = format!("answered at {:?} ({:?})", t0.elapsed(), e.mode);
                                break;
                            }
                            println!("            poll attempt took {:?}: {:?}", a.elapsed(), r.err());
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        out
                    }
                    "layer" => {
                        let l: u8 = arg.parse()?;
                        match kb.raw_send(&Effect::user_picture(l - 1).encode()?) {
                            Ok(()) => "ok".into(),
                            Err(e) => e,
                        }
                    }
                    "wait-ready" => {
                        let mut out = "gave up".to_string();
                        for i in 0..30 {
                            if kb.try_effect_once().is_ok() {
                                out = format!("ready after {i} polls");
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(2000));
                        }
                        out
                    }
                    other => bail!("unknown step {other}"),
                };
                println!("{:>8.2?}  {s:<16} {res}", t.elapsed());
            }
        }
        Cmd::Stream { seconds, frame_ms, mode, .. } if mode == "color" => {
            use std::time::{Duration, Instant};
            kb.set_effect(&Effect::new(Mode::ScreenSync))?;
            let stops: Vec<Rgb> = ["#00fff0", "#0080ff", "#0020ff", "#6a2cff", "#00c8ff"].iter().map(|s| s.parse().unwrap()).collect();
            let t = Instant::now();
            let (mut ok, mut err) = (0u32, 0u32);
            while t.elapsed() < Duration::from_secs(seconds) {
                let x = (t.elapsed().as_secs_f32() / 3.0).fract() * (stops.len() - 1) as f32;
                let i = x.floor() as usize;
                let c = lerp(stops[i], stops[i + 1], x - i as f32);
                match kb.stream_color(c) {
                    Ok(()) => ok += 1,
                    Err(e) => {
                        err += 1;
                        if err < 4 {
                            println!("{e}");
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(frame_ms));
            }
            println!("streamed {ok} colour frames ok, {err} errors, {:.1} fps", ok as f32 / t.elapsed().as_secs_f32());
        }
        Cmd::Stream { seconds, frame_ms, color, mode, direction } => {
            use std::time::{Duration, Instant};
            let m = if mode == "pulse" { Mode::MusicPulse } else { Mode::MusicBars };
            kb.set_effect(&Effect { mode: m, direction, color: color.parse()?, ..Effect::new(m) })?;
            let t = Instant::now();
            let (mut ok, mut err, mut worst) = (0u32, 0u32, Duration::ZERO);
            let mut frame = 0u32;
            while t.elapsed() < Duration::from_secs(seconds) {
                let ph = frame as f32 * 0.25;
                let mut lv = [0u8; keylume_proto::stream::BANDS];
                for (i, l) in lv.iter_mut().enumerate() {
                    let v = 3.0 + 3.0 * ((i as f32 * 0.35) - ph).sin();
                    *l = v.round() as u8;
                }
                let a = Instant::now();
                match kb.stream_levels(&lv) {
                    Ok(()) => ok += 1,
                    Err(e) => {
                        err += 1;
                        if err < 4 {
                            println!("frame {frame}: {e}");
                        }
                    }
                }
                worst = worst.max(a.elapsed());
                frame += 1;
                std::thread::sleep(Duration::from_millis(frame_ms));
            }
            println!("streamed {ok} frames ok, {err} errors, worst send {worst:?}, {:.1} fps", ok as f32 / t.elapsed().as_secs_f32());
            println!("after: {:?}", kb.try_effect_once().map(|e| e.mode));
        }
        Cmd::Burst { count, gap_ms } => {
            // Rapid effect changes, like a user flicking through profiles.
            let colors = ["#0040ff", "#00e5ff", "#1565ff", "#7df9ff", "#3d5afe"];
            let t = std::time::Instant::now();
            for i in 0..count {
                let e = Effect { color: colors[i % colors.len()].parse()?, ..Effect::new(Mode::Static) };
                let p = e.encode()?;
                let r = kb.raw_send(&p);
                // poll GET until the firmware echoes our command
                let a = std::time::Instant::now();
                let mut polls = 0;
                let mut last = String::new();
                while a.elapsed() < std::time::Duration::from_secs(3) {
                    polls += 1;
                    match kb.raw_fetch() {
                        Ok(g) if g[0] == p[0] && g[1] == p[1] => {
                            last = format!("echo {:02x?}", &g[..8]);
                            break;
                        }
                        Ok(g) => last = format!("{:02x?}", &g[..4]),
                        Err(e) => last = e,
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                println!("{:>8.2?} set {i}: {r:?}; ready after {:?} / {polls} polls: {last}", t.elapsed(), a.elapsed());
                std::thread::sleep(std::time::Duration::from_millis(gap_ms));
            }
            std::thread::sleep(std::time::Duration::from_millis(1200));
            println!("{:>8.2?} read: {:?}", t.elapsed(), kb.try_effect_once().map(|e| e.color.to_string()));
        }
        Cmd::Raw { bytes } => {
            let b: Vec<u8> = bytes.iter().map(|s| u8::from_str_radix(s, 16)).collect::<Result<_, _>>()?;
            let req = keylume_proto::packet::with_ck7(&b);
            for i in 0..3 {
                match kb.raw_exchange(&req) {
                    Ok(r) => println!("try {i}: {:02x?}", &r[..24]),
                    Err(e) => println!("try {i}: {e}"),
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        Cmd::SwitchTest { layer } => {
            let t = std::time::Instant::now();
            println!("before: {:?} ({:?})", kb.effect().map(|e| e.direction), t.elapsed());
            let t = std::time::Instant::now();
            kb.set_effect(&Effect::user_picture(layer.saturating_sub(1)))?;
            println!("sent switch ({:?})", t.elapsed());
            for i in 0..40 {
                let r = kb.try_effect_once();
                println!("{:>6.0?} try {i}: {:?}", t.elapsed(), r.as_ref().map(|e| e.direction));
                if r.is_ok() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        Cmd::Keymap { profile } => {
            let km = kb.keymap(profile)?;
            for k in &layout.keys {
                println!("{:10} {}", k.id, serde_json::to_string(&km.0[k.slot])?);
            }
        }
    }
    // Don't leave the firmware mid-commit for whoever talks to it next.
    kb.settle();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_leds_off_clears_only_the_factory_layers() {
        let kb = SimKeyboard::keyboard(50); // firmware timing at 50× speed
        let layout = Layout::tk68();
        let blue = Rgb(0, 0xa1, 0xff);
        let mut f = layout.frame(|_| Some(Rgb(255, 0, 0)));
        for h in &layout.hidden_leds {
            f.set(h.slot, blue);
        }
        for layer in 0..3 {
            kb.write_picture(layer, &f, 4).unwrap();
        }
        kb.set_effect(&Effect::new(Mode::Static)).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let out = hidden_leds(&kb, &layout, true, dir.path()).unwrap();
        assert_eq!(kb.effect().unwrap().mode, Mode::Static, "the effect is put back");
        assert_eq!(out.iter().filter(|l| l.contains("readback matches")).count(), 2, "{out:?}");
        for layer in 0..3 {
            let back = kb.read_picture(layer).unwrap();
            let want = if layer == 2 { blue } else { Rgb::BLACK };
            assert!(layout.hidden_leds.iter().all(|h| back.0[h.slot] == want), "layer {layer}");
            assert_eq!(back.0[layout.key("enter").unwrap().slot], Rgb(255, 0, 0), "the keys are kept");
        }
        assert!(dir.path().join("layer1-before-hidden-off.json").exists());
        assert_eq!(kb.transport().sim_stats().unwrap().busy_violations, 0, "firmware timing respected");
    }
}
