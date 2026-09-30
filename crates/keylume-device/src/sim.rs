//! A behavioural model of the TK68 firmware.
//!
//! It stores lighting, picture layers, key maps, macros and settings, validates
//! checksums, and — most importantly — reproduces the timing traps measured on the
//! real board: a SET inside a busy window fails and *wedges* the channel until a bare
//! GET. Tests drive [`crate::Keyboard`] against it to prove we never trip them.
//! The app also uses it as a demo keyboard when no hardware is connected.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use keylume_proto::packet::{ck7_ok, ck8_ok, cmd, Packet, PAGE_DATA, REPORT_LEN};
use keylume_proto::{stream, Layout};

use crate::{DeviceInfo, Keyboard, Timing, Transport};

/// Busy windows of the real firmware (just inside our safety margins in `Timing::TK68`).
#[derive(Clone, Copy, Debug)]
pub struct FirmwareTiming {
    pub config_busy: Duration,
    pub flash_busy: Duration,
    pub keymap_busy: Duration,
    pub reset_busy: Duration,
}

impl FirmwareTiming {
    pub const TK68: FirmwareTiming = FirmwareTiming {
        config_busy: Duration::from_millis(950),
        flash_busy: Duration::from_millis(1400),
        keymap_busy: Duration::from_millis(2700),
        reset_busy: Duration::from_millis(2500),
    };
    pub fn scaled(f: u32) -> Self {
        let t = Self::TK68;
        FirmwareTiming { config_busy: t.config_busy / f, flash_busy: t.flash_busy / f, keymap_busy: t.keymap_busy / f, reset_busy: t.reset_busy / f }
    }
}

/// Counters exposed for tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimStats {
    pub sets: u32,
    /// SETs that weren't read requests: config and paged writes, and streamed frames.
    pub writes: u32,
    pub gets: u32,
    /// SETs that arrived while the firmware was busy (each one wedged the channel).
    pub busy_violations: u32,
    pub bad_checksums: u32,
    pub stream_frames: u32,
    /// The last whole-board colour streamed (command 0x0E), for tests.
    pub last_stream_color: [u8; 3],
}

#[derive(Clone)]
pub struct SimState {
    pub led: [u8; 8],
    pub side: [u8; 8],
    pub pictures: [[u8; 384]; 3],
    pub keymaps: [[u8; 512]; 3],
    pub fn_keymap: [u8; 512],
    pub macros: [[u8; 256]; 16],
    pub report_code: u8,
    pub debounce: u8,
    pub sleep: [u8; 8],
    pub profile: u8,
    pub options: [u8; 3],
    pub last_levels: [u8; stream::BANDS],
    pub last_color: [u8; 3],
}

struct Inner {
    state: SimState,
    defaults: SimState,
    timing: FirmwareTiming,
    busy_until: Instant,
    wedged: bool,
    reply: Packet,
    stats: SimStats,
}

/// Faults a test can switch on while a keyboard is in use. Shared, so it outlives a
/// simulated unplug and replug.
#[derive(Debug, Default)]
pub struct SimControl {
    /// Unplugged: every transfer fails, and the service can't open the keyboard.
    unplugged: AtomicBool,
    /// Writes still allowed before refusals start.
    ok_writes: AtomicU32,
    /// Writes (not reads) to refuse, as a wedged or vanished board would.
    failing: AtomicU32,
    /// The simulated LampArray keyboard, when that's the demo (for tests to look at).
    lamparray: std::sync::Mutex<Option<Arc<crate::lamparray_sim::SimLampArray>>>,
}

impl SimControl {
    /// The simulated LampArray keyboard, once the service has made one.
    pub fn lamparray(&self) -> Option<Arc<crate::lamparray_sim::SimLampArray>> {
        self.lamparray.lock().unwrap().clone()
    }

    pub fn set_lamparray(&self, sim: Arc<crate::lamparray_sim::SimLampArray>) {
        *self.lamparray.lock().unwrap() = Some(sim);
    }

    pub fn set_unplugged(&self, v: bool) {
        self.unplugged.store(v, Ordering::SeqCst);
    }

    pub fn is_unplugged(&self) -> bool {
        self.unplugged.load(Ordering::SeqCst)
    }

    /// Refuse the next `n` writes.
    pub fn fail_next_writes(&self, n: u32) {
        self.fail_writes_after(0, n);
    }

    /// Let `ok` more writes through, then refuse `n`.
    pub fn fail_writes_after(&self, ok: u32, n: u32) {
        self.ok_writes.store(ok, Ordering::SeqCst);
        self.failing.store(n, Ordering::SeqCst);
    }

    fn take_write_failure(&self) -> bool {
        let dec = |a: &AtomicU32| a.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
        self.failing.load(Ordering::SeqCst) > 0 && !dec(&self.ok_writes) && dec(&self.failing)
    }
}

pub struct SimKeyboard {
    inner: RefCell<Inner>,
    control: Arc<SimControl>,
}

impl SimKeyboard {
    pub fn new(timing: FirmwareTiming) -> Self {
        SimKeyboard::with_control(timing, Arc::default())
    }

    pub fn with_control(timing: FirmwareTiming, control: Arc<SimControl>) -> Self {
        let layout = Layout::tk68();
        let mut keymap = [0u8; 512];
        for (i, s) in layout.default_matrix.iter().enumerate() {
            keymap[i * 4..i * 4 + 4].copy_from_slice(s);
        }
        let mut sleep = [0u8; 8];
        for i in 0..4 {
            sleep[i * 2..i * 2 + 2].copy_from_slice(&3600u16.to_le_bytes());
        }
        let defaults = SimState {
            led: [cmd::LED, 0x04, 2, 4, 8, 0xFF, 0, 0],             // rainbow wave
            side: [cmd::SIDE_LED, 0x03, 0, 4, 1, 0xFF, 0xFF, 0xFF], // rainbow neon, as shipped
            pictures: [[0; 384]; 3],
            keymaps: [keymap; 3],
            fn_keymap: [0; 512],
            macros: [[0; 256]; 16],
            report_code: 1,
            debounce: 2,
            sleep,
            profile: 0,
            options: [0, 0x80, 0],
            last_levels: [0; stream::BANDS],
            last_color: [0; 3],
        };
        SimKeyboard {
            inner: RefCell::new(Inner {
                state: defaults.clone(),
                defaults,
                timing,
                busy_until: Instant::now(),
                wedged: false,
                reply: [0; REPORT_LEN],
                stats: SimStats::default(),
            }),
            control,
        }
    }

    pub fn stats(&self) -> SimStats {
        self.inner.borrow().stats.clone()
    }

    pub fn state(&self) -> SimState {
        self.inner.borrow().state.clone()
    }

    pub fn is_wedged(&self) -> bool {
        self.inner.borrow().wedged
    }

    /// A ready-to-use `Keyboard` over a simulator. `speedup` shrinks both the firmware's
    /// busy windows and our timing margins by the same factor (1 = real time).
    pub fn keyboard(speedup: u32) -> Keyboard<SimKeyboard> {
        SimKeyboard::keyboard_with(speedup, Arc::default())
    }

    /// [`SimKeyboard::keyboard`] whose faults `control` switches.
    pub fn keyboard_with(speedup: u32, control: Arc<SimControl>) -> Keyboard<SimKeyboard> {
        let board = crate::boards::by_id("epomaker-tk68").expect("bundled");
        let info = DeviceInfo::of(board, "TK68 (simulated)", "Keylume", "sim://tk68", true);
        let timing = if speedup <= 1 { Timing::TK68 } else { Timing::scaled(speedup) };
        Keyboard::new(SimKeyboard::with_control(FirmwareTiming::scaled(speedup.max(1)), control), info, timing)
    }
}

fn copy_page(dst: &mut [u8], page: u8, p: &Packet) {
    let off = page as usize * PAGE_DATA;
    if off < dst.len() {
        let n = PAGE_DATA.min(dst.len() - off);
        dst[off..off + n].copy_from_slice(&p[8..8 + n]);
    }
}

fn read_page(src: &[u8], page: u8) -> Packet {
    let mut out = [0u8; REPORT_LEN];
    let off = page as usize * REPORT_LEN;
    if off < src.len() {
        let n = REPORT_LEN.min(src.len() - off);
        out[..n].copy_from_slice(&src[off..off + n]);
    }
    out
}

impl Inner {
    fn busy(&mut self, d: Duration) {
        self.busy_until = Instant::now() + d;
    }

    fn handle(&mut self, p: &Packet) -> Result<(), String> {
        let c = p[0];
        // Streaming frames: RAM only, no checksum on screen frames, never busy.
        if c == stream::AUDIO {
            self.state.last_levels.copy_from_slice(&p[8..8 + stream::BANDS]);
            self.stats.stream_frames += 1;
            return Ok(());
        }
        if c == stream::SCREEN {
            self.state.last_color.copy_from_slice(&p[1..4]);
            self.stats.last_stream_color = self.state.last_color;
            self.stats.stream_frames += 1;
            return Ok(());
        }
        let valid = if c == cmd::LED || c == cmd::SIDE_LED { ck8_ok(p) } else { ck7_ok(p) };
        if !valid {
            self.stats.bad_checksums += 1;
            self.wedged = true;
            return Err("checksum rejected (device not functioning)".into());
        }
        let t = self.timing;
        let s = &mut self.state;
        let mut reply = *p;
        match c {
            // ---- reads ----
            0x80 => {
                reply[1..3].copy_from_slice(&0x0304u16.to_le_bytes());
            }
            0x84 => {
                reply[2] = s.report_code;
            }
            0x85 => {
                reply[1] = 0;
                reply[2] = s.profile;
            }
            0x86 => {
                reply[2..5].copy_from_slice(&s.options);
            }
            0x87 => {
                reply[..8].copy_from_slice(&s.led);
                reply[0] = 0x87;
            }
            0x88 => {
                reply[..8].copy_from_slice(&s.side);
                reply[0] = 0x88;
            }
            0x89 => {
                reply = read_page(&s.keymaps[(p[1] as usize).min(2)], p[2]);
            }
            0x90 => {
                reply = read_page(&s.fn_keymap, p[2]);
            }
            0x8B => {
                reply = read_page(&s.macros[(p[1] as usize).min(15)], p[2]);
            }
            0x8C => {
                let layer = if s.led[1] == 0x0D { (s.led[4] >> 4).min(2) } else { 0 };
                reply = read_page(&s.pictures[layer as usize], p[2]);
            }
            0x91 => {
                reply[2] = s.debounce;
            }
            0x92 => {
                reply[1..9].copy_from_slice(&s.sleep);
            }
            // ---- config writes ----
            cmd::LED => {
                s.led.copy_from_slice(&p[..8]);
                self.busy(t.config_busy);
            }
            cmd::SIDE_LED => {
                s.side.copy_from_slice(&p[..8]);
                self.busy(t.config_busy);
            }
            cmd::REPORT_RATE => {
                s.report_code = p[2];
                self.busy(t.config_busy);
            }
            cmd::PROFILE => {
                // Only 3 onboard profiles; anything else is ignored.
                if p[1] < 3 {
                    s.profile = p[1];
                }
                self.busy(t.config_busy);
            }
            cmd::KB_OPTION => {
                s.options.copy_from_slice(&p[2..5]);
                self.busy(t.config_busy);
            }
            cmd::DEBOUNCE => {
                s.debounce = p[2];
                self.busy(t.config_busy);
            }
            cmd::SLEEP => {
                s.sleep.copy_from_slice(&p[8..16]);
                self.busy(t.config_busy);
            }
            cmd::RESET => {
                *s = self.defaults.clone();
                self.busy(t.reset_busy);
            }
            // ---- paged writes: commit (busy) after the last page ----
            cmd::USER_PICTURE => {
                let layer = if s.led[1] == 0x0D { (s.led[4] >> 4).min(2) } else { 0 };
                copy_page(&mut s.pictures[layer as usize], p[4], p);
                if p[4] == 6 {
                    self.busy(t.flash_busy);
                }
            }
            cmd::KEYMAP => {
                // Measured on hardware: the profile byte is ignored; writes always
                // land in the *active* profile, and the commit takes ~3 s.
                let active = s.profile as usize;
                copy_page(&mut s.keymaps[active], p[4], p);
                if p[4] == 8 {
                    self.busy(t.keymap_busy);
                }
            }
            cmd::FN_KEYMAP => {
                copy_page(&mut s.fn_keymap, p[4], p);
                if p[4] == 8 {
                    self.busy(t.keymap_busy);
                }
            }
            cmd::MACRO => {
                copy_page(&mut s.macros[(p[1] as usize).min(15)], p[4], p);
                if p[4] == 4 {
                    self.busy(t.flash_busy);
                }
            }
            _ => {}
        }
        self.reply = reply;
        Ok(())
    }
}

impl Transport for SimKeyboard {
    fn sim_stats(&self) -> Option<SimStats> {
        Some(self.stats())
    }

    fn set_feature(&self, buf: &[u8; REPORT_LEN + 1]) -> Result<(), String> {
        if self.control.is_unplugged() {
            return Err("device disconnected".into());
        }
        let mut g = self.inner.borrow_mut();
        g.stats.sets += 1;
        let write = buf[1] & cmd::READ == 0;
        if write {
            g.stats.writes += 1;
            if self.control.take_write_failure() {
                return Err("device not functioning (simulated fault)".into());
            }
        }
        if g.wedged {
            return Err("semaphore timeout (channel wedged)".into());
        }
        if Instant::now() < g.busy_until {
            g.stats.busy_violations += 1;
            g.wedged = true;
            return Err("device not functioning (firmware busy)".into());
        }
        let mut p = [0u8; REPORT_LEN];
        p.copy_from_slice(&buf[1..]);
        g.handle(&p)
    }

    fn get_feature(&self, buf: &mut [u8; REPORT_LEN + 1]) -> Result<usize, String> {
        if self.control.is_unplugged() {
            return Err("device disconnected".into());
        }
        let mut g = self.inner.borrow_mut();
        g.stats.gets += 1;
        g.wedged = false; // a bare GET clears a wedged channel
        buf[0] = 0;
        buf[1..].copy_from_slice(&g.reply);
        Ok(REPORT_LEN + 1)
    }
}
