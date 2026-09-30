//! Talks to the keyboard over HID feature reports.
//!
//! [`Keyboard`] is generic over a [`Transport`]: the real USB device ([`hid::HidTransport`])
//! or the firmware simulator ([`sim::SimKeyboard`]) used by tests and the app's demo mode.
//! It owns the firmware's timing rules, so callers can't wedge the config channel.
//! One `Keyboard` per device; callers serialise access (the app runs a single worker).

use std::cell::Cell;
use std::thread::sleep;
use std::time::{Duration, Instant};

use keylume_proto::{
    keymap::Keymap,
    macros::Macro,
    packet::{cmd, read_request, Packet, REPORT_LEN},
    picture::Frame,
    settings::{self, KeyboardOptions, ReportRate, SleepTimers},
    stream, Effect, Mode, ProtoError, Rgb, SideLight,
};

pub mod boards;
pub mod hid;
pub mod identity;
pub mod lamparray;
pub mod lamparray_sim;
pub mod sim;

/// Raw feature-report I/O. Buffers include the report ID (0) at index 0.
pub trait Transport {
    fn set_feature(&self, buf: &[u8; REPORT_LEN + 1]) -> std::result::Result<(), String>;
    fn get_feature(&self, buf: &mut [u8; REPORT_LEN + 1]) -> std::result::Result<usize, String>;
    /// Simulator counters (None on real hardware).
    fn sim_stats(&self) -> Option<sim::SimStats> {
        None
    }
}

/// Firmware timing, measured on a TK68 (docs/PROTOCOL.md, "Timing").
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// Gap between pages of a paged write.
    pub write_gap: Duration,
    /// Gap between a read request and fetching its reply.
    pub read_gap: Duration,
    /// Pause after clearing a refused request.
    pub retry_gap: Duration,
    /// Quiet time after a config write (LED, report rate, …). 0.7 s fails, 1.0 s passes.
    pub config_settle: Duration,
    /// Quiet time after a paged (flash) write of a picture or macro.
    pub flash_commit: Duration,
    /// Key-map pages take longer to commit (1.5 s wedges the channel; 3 s is clean).
    pub keymap_commit: Duration,
    /// Quiet time after a factory reset.
    pub reset_settle: Duration,
}

impl Timing {
    pub const TK68: Timing = Timing {
        write_gap: Duration::from_millis(20),
        read_gap: Duration::from_millis(30),
        retry_gap: Duration::from_millis(300),
        config_settle: Duration::from_millis(1100),
        flash_commit: Duration::from_millis(1500),
        keymap_commit: Duration::from_millis(3000),
        reset_settle: Duration::from_millis(3000),
    };

    /// Same proportions, `factor` times faster (for simulator tests).
    pub fn scaled(factor: u32) -> Timing {
        let s = |d: Duration| d / factor;
        let t = Timing::TK68;
        Timing {
            write_gap: s(t.write_gap),
            read_gap: s(t.read_gap),
            retry_gap: s(t.retry_gap),
            config_settle: s(t.config_settle),
            flash_commit: s(t.flash_commit),
            keymap_commit: s(t.keymap_commit),
            reset_settle: s(t.reset_settle),
        }
    }
}

const RETRIES: usize = 3;

/// Onboard key-map profiles on the TK68 (`layer: 3` in the vendor's device table;
/// writing a 4th stalls the firmware for ~20 s and is discarded).
pub const ONBOARD_PROFILES: u8 = 3;

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("no supported keyboard is connected")]
    NotFound,
    /// Something with a supported keyboard's USB ids that isn't one (see [`identity`]).
    #[error("a device with the keyboard's USB ids is connected, but Keylume won't write to it: {0}")]
    Unrecognised(String),
    #[error("{0} supported keyboards are connected; Keylume drives one at a time, so unplug the others")]
    Ambiguous(usize),
    #[error("USB error: {0}")]
    Transport(String),
    #[error(transparent)]
    Proto(#[from] ProtoError),
    #[error("keyboard did not answer command {0:#04x} (is another keyboard app running?)")]
    NoReply(u8),
    /// The board can't do this (see its [`boards::Features`]).
    #[error("this keyboard can't {0}")]
    Unsupported(&'static str),
    /// Found, but the system won't let Keylume open it (Linux: a udev rule is missing).
    #[error("{0}")]
    Permission(String),
}

pub type Result<T> = std::result::Result<T, DeviceError>;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// The board: a file in `boards/`, or `lamparray-<vid>-<pid>` for a LampArray keyboard.
    pub board: String,
    /// What to call it ("Epomaker TK68").
    pub name: String,
    pub maker: String,
    pub support: boards::Support,
    pub features: boards::Features,
    pub layout_id: String,
    pub vid: u16,
    pub pid: u16,
    pub product: String,
    pub manufacturer: String,
    pub path: String,
    pub simulated: bool,
}

impl DeviceInfo {
    /// A board from [`boards`], as found at `path`.
    pub fn of(board: &boards::BoardDef, product: &str, manufacturer: &str, path: &str, simulated: bool) -> DeviceInfo {
        DeviceInfo {
            board: board.id.clone(),
            name: board.name.clone(),
            maker: board.maker.clone(),
            support: board.support,
            features: board.features.clone(),
            layout_id: board.layout.clone(),
            vid: board.usb.vid,
            pid: board.usb.pid,
            product: product.into(),
            manufacturer: manufacturer.into(),
            path: path.into(),
            simulated,
        }
    }
}

pub struct Keyboard<T: Transport> {
    t: T,
    pub info: DeviceInfo,
    /// The board's layout (every Rongyuan board's layout ships with Keylume).
    pub layout: keylume_proto::Layout,
    timing: Timing,
    /// The firmware is busy (reloading lighting / committing flash) until this instant.
    quiet_until: Cell<Instant>,
}

impl<T: Transport> Keyboard<T> {
    pub fn new(t: T, info: DeviceInfo, timing: Timing) -> Self {
        let layout = keylume_proto::Layout::bundled(&info.layout_id).unwrap_or_else(keylume_proto::Layout::tk68);
        Keyboard { t, info, layout, timing, quiet_until: Cell::new(Instant::now()) }
    }

    pub fn transport(&self) -> &T {
        &self.t
    }

    // ---- transport rules -------------------------------------------------

    fn wait_quiet(&self) {
        let now = Instant::now();
        let until = self.quiet_until.get();
        if until > now {
            sleep(until - now);
        }
    }

    fn busy_for(&self, d: Duration) {
        self.quiet_until.set(Instant::now() + d);
    }

    /// True while the firmware still needs quiet time.
    pub fn is_busy(&self) -> bool {
        Instant::now() < self.quiet_until.get()
    }

    /// Block until the keyboard can take requests again.
    pub fn settle(&self) {
        self.wait_quiet();
    }

    fn send_once(&self, p: &Packet) -> Result<()> {
        let mut buf = [0u8; REPORT_LEN + 1];
        buf[1..].copy_from_slice(p);
        self.t.set_feature(&buf).map_err(DeviceError::Transport)
    }

    fn fetch(&self) -> Result<Packet> {
        let mut buf = [0u8; REPORT_LEN + 1];
        let n = self.t.get_feature(&mut buf).map_err(DeviceError::Transport)?;
        let mut out = [0u8; REPORT_LEN];
        // Some backends return the report ID first, some don't.
        if n == REPORT_LEN + 1 {
            out.copy_from_slice(&buf[1..]);
        } else {
            let n = n.min(REPORT_LEN);
            out[..n].copy_from_slice(&buf[..n]);
        }
        Ok(out)
    }

    /// A bare GET clears a wedged channel.
    fn recover(&self) {
        let _ = self.fetch();
        sleep(self.timing.retry_gap);
    }

    /// SET_FEATURE after the quiet window; on refusal, recover and retry once.
    fn send(&self, p: &Packet) -> Result<()> {
        self.wait_quiet();
        match self.send_once(p) {
            Ok(()) => Ok(()),
            Err(_) => {
                self.recover();
                self.send_once(p)
            }
        }
    }

    /// A single configuration write (LED, report rate, debounce, …).
    fn write(&self, p: &Packet) -> Result<()> {
        self.send(p)?;
        self.busy_for(self.timing.config_settle);
        Ok(())
    }

    /// A paged (flash-backed) write, followed by `commit` of quiet time.
    fn write_pages(&self, packets: &[Packet], commit: Duration) -> Result<()> {
        for p in packets {
            self.send(p)?;
            sleep(self.timing.write_gap);
        }
        self.busy_for(commit);
        Ok(())
    }

    /// Read request + reply. `echo`: single-value replies start with the command byte;
    /// paged replies are raw data.
    fn query(&self, req: &Packet, echo: bool) -> Result<Packet> {
        for _ in 0..RETRIES {
            if self.send(req).is_ok() {
                sleep(self.timing.read_gap);
                if let Ok(r) = self.fetch() {
                    if !echo || r[0] == req[0] {
                        return Ok(r);
                    }
                }
            }
            self.recover();
        }
        Err(DeviceError::NoReply(req[0]))
    }

    fn query_pages(&self, reqs: &[Packet]) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(reqs.len() * REPORT_LEN);
        for r in reqs {
            data.extend_from_slice(&self.query(r, false)?);
        }
        Ok(data)
    }

    // ---- lighting ----------------------------------------------------------

    pub fn firmware_version(&self) -> Result<u16> {
        Ok(settings::parse_version(&self.query(&settings::get_version(), true)?)?)
    }

    pub fn effect(&self) -> Result<Effect> {
        Ok(Effect::decode(&self.query(&read_request(cmd::LED, 0, 0), true)?)?)
    }

    /// Apply an effect. Returns once sent; later requests wait for the firmware.
    pub fn set_effect(&self, e: &Effect) -> Result<()> {
        self.write(&e.encode()?)
    }

    pub fn side_light(&self) -> Result<SideLight> {
        Ok(SideLight::decode(&self.query(&read_request(cmd::SIDE_LED, 0, 0), true)?)?)
    }

    /// Set the side light strip (a config write, like the LED command).
    pub fn set_side_light(&self, s: &SideLight) -> Result<()> {
        self.write(&s.encode()?)
    }

    /// Show picture layer `layer` (0-based) unless it's already showing.
    pub fn select_layer(&self, layer: u8, brightness: u8) -> Result<()> {
        let cur = self.effect()?;
        if cur.mode == Mode::UserPicture && cur.direction == layer && cur.brightness == brightness {
            return Ok(());
        }
        self.set_effect(&Effect { brightness, ..Effect::user_picture(layer) })
    }

    /// Store `frame` in picture layer `layer` (0-based) and show it.
    pub fn write_picture(&self, layer: u8, frame: &Frame, brightness: u8) -> Result<()> {
        self.select_layer(layer, brightness)?;
        self.write_pages(&frame.write_packets(), self.timing.flash_commit)
    }

    /// Read picture layer `layer` (switches the keyboard to it).
    pub fn read_picture(&self, layer: u8) -> Result<Frame> {
        let brightness = self.effect()?.brightness;
        self.select_layer(layer, brightness)?;
        Ok(Frame::from_bytes(&self.query_pages(&Frame::read_requests())?))
    }

    // ---- real-time streaming ----------------------------------------------

    /// One frame of band levels (music bars / pulse modes). RAM-only: no settle.
    pub fn stream_levels(&self, levels: &[u8; stream::BANDS]) -> Result<()> {
        self.wait_quiet();
        self.send_once(&stream::audio_frame(levels))
    }

    /// One whole-board colour (screen-sync mode). RAM-only: no settle.
    pub fn stream_color(&self, c: Rgb) -> Result<()> {
        self.wait_quiet();
        self.send_once(&stream::color_frame(c))
    }

    // ---- keys & macros -------------------------------------------------------

    pub fn keymap(&self, profile: u8) -> Result<Keymap> {
        Ok(Keymap::from_bytes(&self.query_pages(&Keymap::read_requests(profile))?))
    }

    /// Write the base key map of onboard `profile`.
    ///
    /// The firmware reads any profile but only *writes* the active one (the profile
    /// byte in the packet is ignored), so we switch to `profile`, write, and switch
    /// back — exactly what the vendor app does implicitly.
    pub fn set_keymap(&self, profile: u8, km: &Keymap) -> Result<()> {
        if profile >= ONBOARD_PROFILES {
            return Err(ProtoError::OutOfRange("onboard profile").into());
        }
        let active = self.profile()?;
        if active != profile {
            self.set_profile(profile)?;
        }
        let r = self.write_pages(&km.write_packets(profile), self.timing.keymap_commit);
        if active != profile {
            self.set_profile(active)?;
        }
        r
    }

    pub fn fn_keymap(&self, fn_index: u8) -> Result<Keymap> {
        Ok(Keymap::from_bytes(&self.query_pages(&Keymap::read_fn_requests(fn_index))?))
    }

    pub fn set_fn_keymap(&self, fn_index: u8, km: &Keymap) -> Result<()> {
        self.write_pages(&km.write_fn_packets(fn_index), self.timing.keymap_commit)
    }

    pub fn macro_at(&self, index: u8) -> Result<Macro> {
        Ok(Macro::from_bytes(&self.query_pages(&Macro::read_requests(index))?))
    }

    pub fn set_macro(&self, index: u8, m: &Macro) -> Result<()> {
        self.write_pages(&m.write_packets(index)?, self.timing.flash_commit)
    }

    // ---- settings -------------------------------------------------------------

    pub fn report_rate(&self) -> Result<ReportRate> {
        Ok(settings::parse_report_rate(&self.query(&settings::get_report_rate(), true)?)?)
    }

    pub fn set_report_rate(&self, profile: u8, r: ReportRate) -> Result<()> {
        self.write(&settings::set_report_rate(profile, r))
    }

    pub fn debounce(&self) -> Result<u8> {
        Ok(settings::parse_debounce(&self.query(&settings::get_debounce(), true)?)?)
    }

    pub fn set_debounce(&self, v: u8) -> Result<()> {
        self.write(&settings::set_debounce(v))
    }

    pub fn sleep_timers(&self) -> Result<SleepTimers> {
        Ok(settings::parse_sleep(&self.query(&settings::get_sleep(), true)?)?)
    }

    pub fn set_sleep_timers(&self, t: SleepTimers) -> Result<()> {
        self.write(&settings::set_sleep(t))
    }

    pub fn profile(&self) -> Result<u8> {
        Ok(settings::parse_profile(&self.query(&settings::get_profile(), true)?)?)
    }

    pub fn set_profile(&self, p: u8) -> Result<()> {
        if p >= ONBOARD_PROFILES {
            return Err(ProtoError::OutOfRange("onboard profile").into());
        }
        self.write(&settings::set_profile(p))
    }

    pub fn options(&self) -> Result<KeyboardOptions> {
        Ok(settings::parse_options(&self.query(&settings::get_options(), true)?)?)
    }

    pub fn set_options(&self, profile: u8, o: KeyboardOptions) -> Result<()> {
        self.write(&settings::set_options(profile, o))
    }

    pub fn factory_reset(&self) -> Result<()> {
        self.send(&settings::factory_reset())?;
        self.busy_for(self.timing.reset_settle);
        Ok(())
    }

    // ---- diagnostics (CLI) -------------------------------------------------

    /// Is the keyboard still there? A bare GET: it reads the firmware's last reply buffer,
    /// changes nothing, and fails once the device is gone. Waits out a busy window first.
    pub fn probe(&self) -> Result<()> {
        self.wait_quiet();
        self.fetch().map(|_| ())
    }

    pub fn raw_send(&self, p: &Packet) -> std::result::Result<(), String> {
        self.send_once(p).map_err(|e| e.to_string())
    }

    pub fn raw_fetch(&self) -> std::result::Result<Packet, String> {
        self.fetch().map_err(|e| e.to_string())
    }

    pub fn raw_exchange(&self, req: &Packet) -> std::result::Result<Packet, String> {
        self.send_once(req).map_err(|e| format!("send: {e}"))?;
        sleep(self.timing.read_gap);
        self.fetch().map_err(|e| format!("fetch: {e}"))
    }

    pub fn try_effect_once(&self) -> std::result::Result<Effect, String> {
        let r = self.raw_exchange(&read_request(cmd::LED, 0, 0))?;
        Effect::decode(&r).map_err(|_| format!("bad reply {:02x?}", &r[..9]))
    }
}
