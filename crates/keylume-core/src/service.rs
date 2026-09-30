//! The device service: one worker thread owns the keyboard.
//!
//! Why a dedicated thread: the firmware handles one conversation at a time and needs
//! quiet periods, the HID handle isn't `Sync`, and live animations need a steady tick.
//! Everything else (UI commands, tray, hotkeys) sends messages here.
//!
//! Behaviour:
//! - **Coalescing**: lighting changes are "latest wins". While the firmware is busy
//!   with one change, newer requests replace older queued ones, so flicking through
//!   20 profiles applies the one you stopped on, not all 20.
//! - **What's shown** ([`LightingState`]): every request gets an id; a look counts as
//!   shown only once the keyboard accepted it, and an older request never overwrites a
//!   newer one's state.
//! - **Live**: while a live effect runs, frames are pushed whenever the board is idle,
//!   and each frame sent is offered to the window through a [`FrameTap`] (never waited
//!   on). Applying any non-live lighting stops it.
//! - **Picture cache**: per-key profiles are skipped (layer re-selected only) when the
//!   target layer already holds the same frame — fast, and spares the flash.
//! - **Maintenance** ([`Task`]): backups, restores and resets run one at a time, and
//!   nothing else may use the keyboard meanwhile.
//! - **Hot-plug**: the worker reconnects on its own and reports status changes. It only
//!   keeps a keyboard that answered the firmware-version query, then reads back what it
//!   shows.
//! - **Any board**: pictures and spells are built on the worker for the board that's
//!   connected (its own layout), and only what the board's features allow is tried. On a
//!   host-driven board (HID LampArray) Keylume draws the animations itself, frame by
//!   frame, and spells step at a readable pace instead of the flash's.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use keylume_device::sim::{SimControl, SimKeyboard};
use keylume_device::{DeviceError, DeviceInfo};
use keylume_live::effects::EffectPlayer;
use keylume_live::{Animator, Inputs, LiveEffect, LiveFrame};
use keylume_profiles::{Lighting, Profile, SpellWord};
use keylume_proto::picture::Frame;
use keylume_proto::{Effect, Layout, Mode, Rgb, SideLight};
use serde::Serialize;

use crate::board::Board;
use crate::color::{from_led, to_led};
use crate::lamparray::LampArrayBoard;
use crate::lighting::{FrameTap, LightingState, LiveFrameEvent, Origin, Progress, Request, RequestStatus, Shown, Tracker};

/// How the service finds a keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connect {
    /// Real USB keyboards only.
    Hardware,
    /// The firmware simulator (`speedup` > 1 shrinks its timing, for tests).
    Simulator { speedup: u32 },
    /// Real hardware when present, otherwise the simulator (demo mode).
    HardwareOrSimulator,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub connected: bool,
    pub device: Option<DeviceInfo>,
    pub firmware: Option<u16>,
    /// Name of the running live effect, if any.
    pub live: Option<String>,
    /// Last error from a background operation.
    pub error: Option<String>,
    /// Lighting requests applied since start (for the UI / tests).
    pub applied: u64,
    /// Why the service is standing down (e.g. another keyboard app is running).
    pub paused: Option<String>,
    /// A backup, restore or factory reset in progress: nothing else may use the keyboard.
    pub busy: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("no keyboard connected")]
    NotConnected,
    #[error("paused: {0}")]
    Paused(String),
    #[error("the keyboard is busy ({0}); try again when it's done")]
    Busy(String),
    #[error(transparent)]
    Device(#[from] DeviceError),
    #[error("device service stopped")]
    Stopped,
    /// The connected board can't show this (nothing was sent).
    #[error("{0}")]
    Unsupported(String),
}

/// Long jobs nothing else may interleave with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Backup,
    Restore,
    Reset,
}

impl Task {
    pub fn label(self) -> &'static str {
        match self {
            Task::Backup => "backing up",
            Task::Restore => "restoring a backup",
            Task::Reset => "factory reset",
        }
    }

    /// Does it change what the keyboard stores (so caches and live effects go)?
    fn rewrites(self) -> bool {
        self != Task::Backup
    }
}

/// Which keyboard the simulator plays (demo mode, tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Demo {
    /// The TK68's firmware (verified protocol, stored pictures, firmware effects).
    #[default]
    Tk68,
    /// A full-size HID LampArray keyboard Keylume drives light by light.
    LampArray,
}

/// Settings the worker starts with, so the first connection already uses them.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub live_layer: u8,
    pub true_colors: bool,
    pub demo: Demo,
}

impl Default for Options {
    fn default() -> Self {
        Options { live_layer: 2, true_colors: false, demo: Demo::Tk68 }
    }
}

pub type StatusListener = Box<dyn Fn(&Status) + Send + Sync>;
pub type LightingListener = Box<dyn Fn(&LightingState) + Send + Sync>;

/// Who hears about changes (called from the thread that made them; keep them quick).
#[derive(Default)]
pub struct Listeners {
    pub status: Option<StatusListener>,
    pub lighting: Option<LightingListener>,
}

pub type InputsProvider = Arc<dyn Fn() -> Inputs + Send + Sync>;
type Job = Box<dyn FnOnce(&mut Worker) + Send>;

/// A lighting change to apply (coalesced). Key colours are turned into the board's
/// pictures on the worker, for whichever board is connected by then.
#[derive(Clone, Debug)]
pub enum LightingRequest {
    Effect(Effect),
    Picture {
        keys: BTreeMap<String, Rgb>,
        brightness: u8,
    },
    Live {
        name: String,
        effect: LiveEffect,
    },
    /// Pictures shown one after another (a spell): as fast as the firmware allows, or at
    /// [`SPELL_STEP`] on a board that shows pictures at once; the last one stays up.
    Spell {
        name: String,
        words: Vec<SpellWord>,
        background: Rgb,
        brightness: u8,
    },
    Off,
}

/// A request on its way to the worker, with what to show once it's applied.
struct Pending {
    id: u64,
    req: LightingRequest,
    shown: Shown,
}

enum Msg {
    Job(Job),
    Lighting(Box<Pending>),
    Side(SideLight),
    StopLive,
    Inputs(InputsProvider),
    LiveLayer(u8),
    TrueColors(bool),
    Pause(Option<String>),
    Shutdown,
}

/// State the service handle and its worker share.
struct Shared {
    status: Mutex<Status>,
    /// The connected board's layout (the last one's while none is connected).
    layout: Mutex<Arc<Layout>>,
    lighting: Mutex<Tracker>,
    next_request: AtomicU64,
    busy: Mutex<Option<&'static str>>,
    frames: Arc<FrameTap>,
    listeners: Listeners,
}

impl Shared {
    fn publish_status(&self, f: impl FnOnce(&mut Status)) {
        let snapshot = {
            let mut s = self.status.lock().unwrap();
            let before = s.clone();
            f(&mut s);
            if *s == before {
                return;
            }
            s.clone()
        };
        if let Some(l) = &self.listeners.status {
            l(&snapshot);
        }
    }

    fn publish_lighting(&self, f: impl FnOnce(&mut Tracker)) {
        let snapshot = {
            let mut t = self.lighting.lock().unwrap();
            let before = t.clone();
            f(&mut t);
            if *t == before {
                return;
            }
            t.seq += 1;
            t.state()
        };
        if let Some(l) = &self.listeners.lighting {
            l(&snapshot);
        }
    }

    fn busy(&self) -> Option<&'static str> {
        *self.busy.lock().unwrap()
    }
}

pub struct Service {
    tx: Sender<Msg>,
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
    sim: Option<Arc<SimControl>>,
}

impl Service {
    pub fn start(connect: Connect, on_status: impl Fn(&Status) + Send + Sync + 'static) -> Service {
        Service::start_with(connect, Options::default(), Listeners { status: Some(Box::new(on_status)), lighting: None })
    }

    pub fn start_with(connect: Connect, options: Options, listeners: Listeners) -> Service {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            status: Mutex::new(Status::default()),
            layout: Mutex::new(Arc::new(Layout::tk68())),
            lighting: Mutex::new(Tracker::default()),
            next_request: AtomicU64::new(0),
            busy: Mutex::new(None),
            frames: Arc::default(),
            listeners,
        });
        let sim = (connect != Connect::Hardware).then(Arc::<SimControl>::default);
        let worker = Worker::new(connect, options, shared.clone(), sim.clone());
        let handle = thread::Builder::new().name("keylume-device".into()).spawn(move || worker.run(rx)).expect("spawn device worker");
        Service { tx, shared, handle: Some(handle), sim }
    }

    /// The connected board's layout (the last connected one's while none is).
    pub fn layout(&self) -> Arc<Layout> {
        self.shared.layout.lock().unwrap().clone()
    }

    pub fn status(&self) -> Status {
        self.shared.status.lock().unwrap().clone()
    }

    /// What the keyboard shows, and the newest request.
    pub fn lighting(&self) -> LightingState {
        self.shared.lighting.lock().unwrap().state()
    }

    /// Frames of the running live effect, as sent (for the window's preview).
    pub fn frames(&self) -> Arc<FrameTap> {
        self.shared.frames.clone()
    }

    /// The simulator's fault switches (None when the service only uses hardware).
    pub fn simulator(&self) -> Option<Arc<SimControl>> {
        self.sim.clone()
    }

    /// Run `f` on the worker thread and wait for it (no maintenance check).
    fn job<R: Send + 'static>(&self, f: impl FnOnce(&mut Worker) -> R + Send + 'static) -> Result<R, ServiceError> {
        let (rtx, rrx) = mpsc::channel();
        let job: Job = Box::new(move |w: &mut Worker| {
            let _ = rtx.send(f(w));
        });
        self.tx.send(Msg::Job(job)).map_err(|_| ServiceError::Stopped)?;
        rrx.recv().map_err(|_| ServiceError::Stopped)
    }

    fn not_busy(&self) -> Result<(), ServiceError> {
        match self.shared.busy() {
            Some(what) => Err(ServiceError::Busy(what.into())),
            None => Ok(()),
        }
    }

    /// Run `f` against the keyboard on the worker thread and wait for its result.
    pub fn call<R: Send + 'static>(&self, f: impl FnOnce(&dyn Board) -> keylume_device::Result<R> + Send + 'static) -> Result<R, ServiceError> {
        self.not_busy()?;
        self.job(move |w| {
            let r = match (w.board.as_deref(), &w.paused) {
                (_, Some(why)) => Err(ServiceError::Paused(why.clone())),
                (Some(b), None) => f(b).map_err(ServiceError::from),
                (None, None) => Err(ServiceError::NotConnected),
            };
            if let Err(ServiceError::Device(DeviceError::Transport(_))) = &r {
                w.drop_board("USB error");
            }
            r
        })?
    }

    /// Backups, restores and factory resets: one at a time, with nothing else touching
    /// the keyboard meanwhile (other calls get [`ServiceError::Busy`], lighting requests
    /// fail). A restore or reset stops live effects first, and afterwards forgets what the
    /// caches knew and reads back what the keyboard shows.
    pub fn maintain<R: Send + 'static>(&self, task: Task, f: impl FnOnce(&dyn Board) -> R + Send + 'static) -> Result<R, ServiceError> {
        {
            let mut busy = self.shared.busy.lock().unwrap();
            if let Some(what) = *busy {
                return Err(ServiceError::Busy(what.into()));
            }
            *busy = Some(task.label());
        }
        self.shared.publish_status(|s| s.busy = Some(task.label().into()));
        let r = self.job(move |w| w.maintain(task, f));
        *self.shared.busy.lock().unwrap() = None;
        self.shared.publish_status(|s| s.busy = None);
        r?
    }

    /// Queue a lighting change (non-blocking, coalesced). Returns its request id.
    fn request(&self, origin: Origin, profile_id: Option<String>, name: String, lighting: Option<Lighting>) -> u64 {
        let req = match &lighting {
            None => LightingRequest::Off,
            Some(Lighting::Effect { effect }) => LightingRequest::Effect(*effect),
            Some(Lighting::PerKey { keys, brightness }) => LightingRequest::Picture { keys: keys.clone(), brightness: *brightness },
            Some(Lighting::Live { live }) => LightingRequest::Live { name: name.clone(), effect: live.clone() },
            Some(Lighting::Spell { words, background, brightness }) => {
                LightingRequest::Spell { name: name.clone(), words: words.clone(), background: *background, brightness: *brightness }
            }
        };
        let brightness = match &lighting {
            None => 0,
            Some(Lighting::Effect { effect }) => effect.brightness,
            Some(Lighting::PerKey { brightness, .. } | Lighting::Spell { brightness, .. }) => *brightness,
            Some(Lighting::Live { live }) => live.base_effect().brightness,
        };
        let busy = self.shared.busy();
        let snapshot = {
            // Registered and sent under one lock, so the worker sees requests in id order.
            let mut t = self.shared.lighting.lock().unwrap();
            let id = self.shared.next_request.fetch_add(1, Ordering::SeqCst) + 1;
            t.request = Some(Request { id, profile_id: profile_id.clone(), name: name.clone() });
            t.status = match busy {
                Some(what) => RequestStatus::Failed(format!("the keyboard is busy ({what})")),
                None => RequestStatus::Requested,
            };
            if busy.is_none() {
                let shown = Shown { request: id, origin, profile_id, name, lighting, brightness, running: false, progress: None };
                let _ = self.tx.send(Msg::Lighting(Box::new(Pending { id, req, shown })));
            }
            t.seq += 1;
            (id, t.state())
        };
        if let Some(l) = &self.shared.listeners.lighting {
            l(&snapshot.1);
        }
        snapshot.0
    }

    /// Show a profile from the library. Returns the request id.
    pub fn apply_profile(&self, p: &Profile) -> u64 {
        self.request(Origin::Profile, Some(p.id.clone()), p.name.clone(), Some(p.lighting.clone()))
    }

    /// Show lighting that isn't a saved profile (an editor's preview). Returns the request id.
    pub fn apply(&self, name: &str, lighting: &Lighting) -> u64 {
        self.request(Origin::Preview, None, name.to_string(), Some(lighting.clone()))
    }

    /// Apply a raw effect (e.g. from the effect editor's sliders). Coalesced.
    pub fn apply_effect(&self, e: Effect) -> u64 {
        self.apply("Unsaved animation", &Lighting::Effect { effect: e })
    }

    pub fn lights_off(&self) -> u64 {
        self.request(Origin::Off, None, "Lights off".into(), None)
    }

    /// Set the side light strip (non-blocking, coalesced; skipped when unchanged).
    pub fn set_side(&self, s: SideLight) {
        if self.shared.busy().is_none() {
            let _ = self.tx.send(Msg::Side(s));
        }
    }

    /// Stop a live effect or spell; the keyboard keeps its last frame.
    pub fn stop_live(&self) {
        let _ = self.tx.send(Msg::StopLive);
    }

    pub fn set_inputs_provider(&self, p: InputsProvider) {
        let _ = self.tx.send(Msg::Inputs(p));
    }

    /// Stand down (release the keyboard) while another program drives it; None resumes.
    pub fn pause(&self, reason: Option<String>) {
        let _ = self.tx.send(Msg::Pause(reason));
    }

    /// Which picture layer (0-based) per-key profiles are written to.
    pub fn set_live_layer(&self, layer: u8) {
        let _ = self.tx.send(Msg::LiveLayer(layer.min(2)));
    }

    /// Correct every colour sent to the board for the LEDs' linear response (screen
    /// colours are gamma-encoded, the LEDs aren't). Off by default.
    pub fn set_true_colors(&self, on: bool) {
        let _ = self.tx.send(Msg::TrueColors(on));
    }

    /// Write a frame to a specific layer (blocking) and remember it in the cache. The
    /// keyboard then shows that layer.
    pub fn write_layer(&self, layer: u8, frame: Frame, brightness: u8) -> Result<(), ServiceError> {
        self.not_busy()?;
        self.job(move |w| {
            if w.stop_running() {
                w.publish(|s| s.live = None);
            }
            let r = w.write_frame(layer, &frame, brightness, true);
            let shown = match &r {
                Ok(()) => Some(w.picture_shown(layer, &w.cache[layer.min(2) as usize].clone().unwrap_or(frame), brightness, Origin::Preview)),
                Err(_) => None,
            };
            w.lit(|t| t.shown = shown);
            r
        })?
    }

    /// Read a picture layer back (blocking), decoded to the colours it was designed
    /// with (the inverse of the correction `write_layer` and friends apply on the way out).
    /// Reading switches the keyboard to that layer, which it then shows.
    pub fn read_layer(&self, layer: u8) -> Result<Frame, ServiceError> {
        self.not_busy()?;
        self.job(move |w| {
            let layer = layer.min(2);
            let r = match (w.board.as_deref(), &w.paused) {
                (_, Some(why)) => Err(ServiceError::Paused(why.clone())),
                (Some(b), None) => b.effect().and_then(|e| Ok((b.read_picture(layer)?, e.brightness))).map_err(ServiceError::from),
                (None, None) => Err(ServiceError::NotConnected),
            };
            if let Err(ServiceError::Device(DeviceError::Transport(_))) = &r {
                w.drop_board("USB error");
            }
            let (frame, brightness) = r?;
            if w.stop_running() {
                w.publish(|s| s.live = None);
            }
            let shown = w.picture_shown(layer, &frame, brightness, Origin::Keyboard);
            w.cache[layer as usize] = Some(frame.clone());
            w.lit(|t| t.shown = Some(shown));
            Ok(if w.true_colors { map_frame(&frame, from_led) } else { frame })
        })?
    }

    /// Wait until every queued request has been processed (tests, shutdown).
    pub fn flush(&self) -> Result<(), ServiceError> {
        self.job(|w| {
            w.apply_pending();
            if let Some(b) = w.board.as_deref() {
                b.settle();
            }
        })
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

// ---------------------------------------------------------------------------------------

struct LiveRun {
    request: u64,
    anim: Animator,
    t0: Instant,
    frames: u64,
}

struct SequenceRun {
    request: u64,
    frames: VecDeque<Frame>,
    brightness: u8,
    /// When the last picture went up (host-driven boards step at [`SPELL_STEP`]).
    shown_at: Instant,
}

/// An animation Keylume draws itself on a host-driven board.
struct HostRun {
    player: EffectPlayer,
    brightness: u8,
    t0: Instant,
}

pub(crate) struct Worker {
    connect: Connect,
    shared: Arc<Shared>,
    pub(crate) board: Option<Box<dyn Board>>,
    hid: Option<hidapi::HidApi>,
    sim: Option<Arc<SimControl>>,
    last_try: Option<Instant>,
    retry: Duration,
    /// When the keyboard last answered (an unplugged keyboard is otherwise only noticed at
    /// the next write).
    last_seen: Instant,
    pending: Option<Box<Pending>>,
    pending_side: Option<SideLight>,
    /// Last side light written (skip identical writes).
    side_cache: Option<SideLight>,
    live: Option<LiveRun>,
    /// Frames of a running sequence (spell) still to show.
    sequence: Option<SequenceRun>,
    /// An animation drawn by Keylume (host-driven boards).
    host: Option<HostRun>,
    inputs: InputsProvider,
    live_layer: u8,
    /// Correct colours for the LEDs' linear response before they go to the board.
    true_colors: bool,
    cache: [Option<Frame>; 3],
    paused: Option<String>,
    layout: Layout,
    demo: Demo,
}

/// Map every colour of a frame through `f` (used for the sRGB <-> LED correction).
fn map_frame(frame: &Frame, f: impl Fn(Rgb) -> Rgb) -> Frame {
    Frame(frame.0.iter().map(|&c| f(c)).collect())
}

const RETRY_CONNECT: Duration = Duration::from_millis(1500);
/// How often an idle keyboard is checked for (one bare GET).
const PRESENCE_CHECK: Duration = Duration::from_secs(2);
const LIVE_TICK: Duration = Duration::from_millis(40); // ~25 fps
/// How long each picture of a spell stays up on a board that shows pictures at once.
pub const SPELL_STEP: Duration = Duration::from_millis(700);
const IDLE_TICK: Duration = Duration::from_millis(250);

impl Worker {
    fn new(connect: Connect, options: Options, shared: Arc<Shared>, sim: Option<Arc<SimControl>>) -> Self {
        let retry = match connect {
            Connect::Simulator { speedup } => RETRY_CONNECT / speedup.max(1),
            _ => RETRY_CONNECT,
        };
        Worker {
            connect,
            shared,
            board: None,
            hid: None,
            sim,
            last_try: None,
            retry,
            last_seen: Instant::now(),
            pending: None,
            pending_side: None,
            side_cache: None,
            live: None,
            sequence: None,
            host: None,
            inputs: Arc::new(Inputs::default),
            live_layer: options.live_layer.min(2),
            true_colors: options.true_colors,
            cache: [None, None, None],
            paused: None,
            layout: Layout::tk68(),
            demo: options.demo,
        }
    }

    fn publish(&self, f: impl FnOnce(&mut Status)) {
        self.shared.publish_status(f);
    }

    fn lit(&self, f: impl FnOnce(&mut Tracker)) {
        self.shared.publish_lighting(f);
    }

    /// A verified board from `boards/` first; else a keyboard that describes its own
    /// lights (HID LampArray).
    fn open_hardware(&mut self) -> keylume_device::Result<Box<dyn Board>> {
        if self.hid.is_none() {
            self.hid = Some(hidapi::HidApi::new().map_err(|e| DeviceError::Transport(e.to_string()))?);
        }
        let api = self.hid.as_mut().expect("just set");
        match keylume_device::hid::open(api) {
            Ok(k) => Ok(Box::new(k)),
            Err(first @ (DeviceError::NotFound | DeviceError::Unrecognised(_))) => match keylume_device::hid::open_lamparray(api) {
                Ok((lamps, info)) => Ok(Box::new(LampArrayBoard::new(lamps, info)?)),
                // nothing there either: say why the first one was refused, if one was
                Err(DeviceError::NotFound) => Err(first),
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        }
    }

    fn open_simulator(&self, speedup: u32) -> keylume_device::Result<Box<dyn Board>> {
        let control = self.sim.clone().unwrap_or_default();
        if control.is_unplugged() {
            return Err(DeviceError::NotFound);
        }
        Ok(match self.demo {
            Demo::Tk68 => Box::new(SimKeyboard::keyboard_with(speedup, control)),
            Demo::LampArray => {
                let (board, sim) = crate::lamparray::demo();
                control.set_lamparray(sim);
                Box::new(board)
            }
        })
    }

    fn ensure_connected(&mut self) {
        if self.board.is_some() || self.paused.is_some() {
            return;
        }
        if self.last_try.is_some_and(|t| t.elapsed() < self.retry) {
            return;
        }
        self.last_try = Some(Instant::now());
        let opened = match self.connect {
            Connect::Hardware => self.open_hardware(),
            Connect::Simulator { speedup } => self.open_simulator(speedup),
            // a device that failed the identity checks is reported, not swapped for the demo
            Connect::HardwareOrSimulator => match self.open_hardware() {
                Err(DeviceError::NotFound | DeviceError::Transport(_)) => self.open_simulator(1),
                other => other,
            },
        };
        let b = match opened {
            Ok(b) => b,
            Err(DeviceError::NotFound) => return,
            Err(e) => {
                let msg = e.to_string();
                self.publish(|s| s.error = Some(msg));
                return;
            }
        };
        // The last check before keeping it: the firmware answers its version query (a
        // board without one answers a bare read).
        let fw = match b.firmware_version() {
            Ok(fw) => Some(fw),
            Err(DeviceError::Unsupported(_)) => match b.probe() {
                Ok(()) => None,
                Err(e) => {
                    let msg = format!("the keyboard didn't answer ({e})");
                    self.publish(|s| s.error = Some(msg));
                    return;
                }
            },
            Err(e) => {
                let msg = format!("the keyboard didn't answer ({e})");
                self.publish(|s| s.error = Some(msg));
                return;
            }
        };
        let info = b.info().clone();
        self.layout = b.layout().clone();
        *self.shared.layout.lock().unwrap() = Arc::new(self.layout.clone());
        self.board = Some(b);
        self.cache = [None, None, None];
        self.side_cache = None;
        self.publish(|s| {
            s.connected = true;
            s.device = Some(info);
            s.firmware = fw;
            s.error = None;
        });
        self.lit(|t| t.connected = true);
        self.read_back(Origin::Keyboard);
    }

    /// Learn what the keyboard shows by asking it (after connecting, a restore or a
    /// reset). Reads only: the picture of a user-picture layer is read without switching.
    fn read_back(&mut self, origin: Origin) {
        let Some(b) = self.board.as_deref() else { return };
        if !b.features().read_back {
            // what a host-driven board shows now is its own business until Keylume sets it
            self.lit(|t| {
                t.shown = None;
                if !matches!(t.status, RequestStatus::Requested | RequestStatus::Pending) {
                    t.status = RequestStatus::None;
                }
            });
            return;
        }
        let effect = match b.effect() {
            Ok(e) => e,
            Err(e) => {
                let transport = matches!(e, DeviceError::Transport(_));
                self.lit(|t| t.shown = None);
                if transport {
                    self.drop_board("USB error");
                }
                return;
            }
        };
        // the layer that's showing is read without switching (a layer the keyboard reports
        // beyond the three isn't read at all: reading it would mean switching, a write)
        let shown = if effect.mode == Mode::UserPicture && effect.direction <= 2 {
            let layer = effect.direction;
            match b.read_picture(layer) {
                Ok(frame) => {
                    let shown = self.picture_shown(layer, &frame, effect.brightness, origin);
                    self.cache[layer as usize] = Some(frame);
                    shown
                }
                Err(_) => self.shown(origin, format!("Picture layer {}", layer + 1), Lighting::Effect { effect }),
            }
        } else if effect.mode == Mode::UserPicture {
            self.shown(origin, "A picture layer".into(), Lighting::Effect { effect })
        } else {
            let color = if self.true_colors { from_led(effect.color) } else { effect.color };
            self.shown(origin, effect.mode.info().name.to_string(), Lighting::Effect { effect: Effect { color, ..effect } })
        };
        self.lit(|t| {
            t.shown = Some(shown);
            // a failure from before (say, the disconnect) is old news; a queued request isn't
            if !matches!(t.status, RequestStatus::Requested | RequestStatus::Pending) {
                t.status = RequestStatus::None;
            }
        });
    }

    /// What's shown when it didn't come from a request.
    fn shown(&self, origin: Origin, name: String, lighting: Lighting) -> Shown {
        let brightness = match &lighting {
            Lighting::Effect { effect } => effect.brightness,
            Lighting::PerKey { brightness, .. } | Lighting::Spell { brightness, .. } => *brightness,
            Lighting::Live { live } => live.base_effect().brightness,
        };
        Shown { request: 0, origin, profile_id: None, name, lighting: Some(lighting), brightness, running: false, progress: None }
    }

    /// A picture layer as the keyboard holds it (`frame` in LED values), in design colours.
    fn picture_shown(&self, layer: u8, frame: &Frame, brightness: u8, origin: Origin) -> Shown {
        let decode = |c: Rgb| if self.true_colors { from_led(c) } else { c };
        let keys = self.layout.keys.iter().map(|k| (k.id.clone(), decode(frame.0.get(k.slot).copied().unwrap_or_default()))).collect();
        self.shown(origin, format!("Picture layer {}", layer + 1), Lighting::PerKey { keys, brightness })
    }

    /// Is the keyboard still plugged in? A bare GET to it every couple of seconds while
    /// it's idle: it changes nothing, and fails once the device is gone. (Listing the
    /// system's devices instead would ask every HID device for its strings on Windows.)
    fn check_presence(&mut self) {
        let every = match self.connect {
            Connect::Simulator { speedup } => PRESENCE_CHECK / speedup.max(1),
            _ => PRESENCE_CHECK,
        };
        let Some(b) = self.board.as_deref() else { return };
        // a live effect or spell writes all the time, and a write notices by itself
        if self.last_seen.elapsed() < every || b.is_busy() || self.live.is_some() || self.sequence.is_some() || self.host.is_some() {
            return;
        }
        self.last_seen = Instant::now();
        if let Err(DeviceError::Transport(_)) = b.probe() {
            self.drop_board("the keyboard was unplugged");
        }
    }

    pub(crate) fn drop_board(&mut self, why: &str) {
        if self.board.take().is_some() {
            self.live = None;
            self.sequence = None;
            self.host = None;
            let why = why.to_string();
            self.publish(|s| {
                s.connected = false;
                s.device = None;
                s.firmware = None;
                s.live = None;
                s.error = Some(why);
            });
            self.lit(|t| {
                t.connected = false;
                t.shown = None;
                if matches!(t.status, RequestStatus::Requested | RequestStatus::Pending) {
                    t.status = RequestStatus::Failed("the keyboard was disconnected".into());
                }
            });
        }
    }

    fn set_paused(&mut self, reason: Option<String>) {
        if reason == self.paused {
            return;
        }
        match reason {
            Some(why) => {
                // Finish what the firmware is doing, then let go of the keyboard entirely.
                if let Some(b) = self.board.as_deref() {
                    b.settle();
                    b.release();
                }
                self.board = None;
                self.pending = None;
                self.pending_side = None;
                self.live = None;
                self.sequence = None;
                self.host = None;
                self.paused = Some(why.clone());
                self.publish(|s| {
                    s.paused = Some(why.clone());
                    s.connected = false;
                    s.device = None;
                    s.firmware = None;
                    s.live = None;
                });
                self.lit(|t| {
                    t.paused = Some(why);
                    t.connected = false;
                    t.shown = None;
                    if matches!(t.status, RequestStatus::Requested | RequestStatus::Pending) {
                        t.status = RequestStatus::Failed("Keylume is paused while another app uses the keyboard".into());
                    }
                });
            }
            None => {
                self.paused = None;
                self.last_try = None; // reconnect right away
                self.publish(|s| s.paused = None);
                self.lit(|t| t.paused = None);
            }
        }
    }

    fn fail(&mut self, e: DeviceError) {
        match e {
            DeviceError::Transport(_) | DeviceError::NotFound => self.drop_board(&e.to_string()),
            other => {
                let msg = other.to_string();
                self.publish(|s| s.error = Some(msg));
            }
        }
    }

    pub(crate) fn write_frame(&mut self, layer: u8, frame: &Frame, brightness: u8, force: bool) -> Result<(), ServiceError> {
        let board = self.board.as_deref().ok_or(ServiceError::NotConnected)?;
        let host = board.features().host_driven;
        let l = if host { 0 } else { layer.min(2) as usize };
        // Correct first, then cache and compare against what actually goes to the board.
        let frame = if self.true_colors { map_frame(frame, to_led) } else { frame.clone() };
        if !force && self.cache[l].as_ref() == Some(&frame) {
            if !host {
                board.select_layer(layer, brightness)?;
            }
        } else {
            // until the write completes, the layer's contents are unknown
            self.cache[l] = None;
            board.write_picture(layer, &frame, brightness)?;
            self.cache[l] = Some(frame);
        }
        Ok(())
    }

    /// Stop a running live effect or spell; the keyboard keeps its last frame. (An
    /// animation Keylume draws stops too, but that's no "running" effect to the user.)
    fn stop_running(&mut self) -> bool {
        self.host = None;
        let stopped = self.live.take().is_some() | self.sequence.take().is_some();
        if stopped {
            self.lit(|t| {
                if let Some(s) = t.shown.as_mut() {
                    s.running = false;
                }
            });
        }
        stopped
    }

    fn apply(&mut self, p: Pending) {
        let Pending { id, req, mut shown } = p;
        if self.board.is_none() {
            let why = match &self.paused {
                Some(_) => "Keylume is paused while another app uses the keyboard",
                None => "no keyboard connected",
            };
            self.lit(|t| {
                if t.is_newest(id) {
                    t.status = RequestStatus::Failed(why.into());
                }
            });
            return;
        }
        self.lit(|t| {
            if t.is_newest(id) {
                t.status = RequestStatus::Pending;
            }
        });
        // Anything new replaces a running live effect or spell.
        if self.stop_running() && !matches!(req, LightingRequest::Live { .. } | LightingRequest::Spell { .. }) {
            self.publish(|s| s.live = None);
        }
        let features = self.board.as_deref().unwrap().features().clone();
        let result: Result<(), ServiceError> = match req {
            LightingRequest::Spell { name, words, background, brightness } => {
                let frames = keylume_profiles::spell::frames(&self.layout, &words, background);
                let total = frames.len() as u32;
                let mut frames: VecDeque<Frame> = frames.iter().map(|k| self.frame_of(k)).collect();
                match frames.pop_front() {
                    Some(first) => {
                        let layer = self.live_layer;
                        let r = self.write_frame(layer, &first, brightness, false);
                        shown.progress = Some(Progress { done: 1, total });
                        if r.is_ok() && !frames.is_empty() {
                            shown.running = true;
                            self.sequence = Some(SequenceRun { request: id, frames, brightness, shown_at: Instant::now() });
                            self.publish(|s| s.live = Some(name));
                        } else {
                            self.publish(|s| s.live = None);
                        }
                        r
                    }
                    None => Ok(()),
                }
            }
            LightingRequest::Effect(e) if !features.effects.contains(&e.mode) => {
                Err(ServiceError::Unsupported(format!("This keyboard can't show the {} animation.", e.mode.info().name)))
            }
            LightingRequest::Effect(e) if features.host_driven => {
                // Keylume draws it (in design colours; `write_frame` corrects them)
                let player = EffectPlayer::new(e, &self.layout);
                let frame = self.frame_of(&player.frame(0.0));
                let r = self.write_frame(0, &frame, e.brightness, false);
                if r.is_ok() && !matches!(e.mode, Mode::Off | Mode::Static) {
                    self.host = Some(HostRun { player, brightness: e.brightness, t0: Instant::now() });
                }
                r
            }
            LightingRequest::Effect(e) => {
                let e = if self.true_colors { Effect { color: to_led(e.color), ..e } } else { e };
                self.board.as_deref().unwrap().set_effect(&e).map_err(Into::into)
            }
            LightingRequest::Off if features.host_driven => self.write_frame(0, &self.layout.frame(|_| Some(Rgb::BLACK)), 0, false),
            LightingRequest::Off => {
                self.board.as_deref().unwrap().set_effect(&Effect { brightness: 0, ..Effect::new(Mode::Off) }).map_err(Into::into)
            }
            LightingRequest::Picture { .. } if !features.per_key => Err(ServiceError::Unsupported("This keyboard has no per-key lighting.".into())),
            LightingRequest::Picture { keys, brightness } => {
                let layer = self.live_layer;
                let frame = self.frame_of(&keys);
                self.write_frame(layer, &frame, brightness, false)
            }
            LightingRequest::Live { .. } if !features.live => Err(ServiceError::Unsupported("This keyboard can't show live effects.".into())),
            LightingRequest::Live { name, effect } => {
                // a host-driven board needs no base effect: Keylume colours the bars itself
                let r = if features.host_driven {
                    Ok(())
                } else {
                    let base = effect.base_effect();
                    let base = if self.true_colors { Effect { color: to_led(base.color), ..base } } else { base };
                    self.board.as_deref().unwrap().set_effect(&base).map_err(Into::into)
                };
                if r.is_ok() {
                    shown.running = true;
                    self.live = Some(LiveRun { request: id, anim: Animator::new(effect), t0: Instant::now(), frames: 0 });
                    self.publish(|s| s.live = Some(name));
                }
                r
            }
        };
        match result {
            Ok(()) => {
                self.publish(|s| {
                    s.applied += 1;
                    s.error = None;
                });
                self.lit(|t| {
                    t.shown = Some(shown);
                    if t.is_newest(id) {
                        t.status = RequestStatus::Done;
                    }
                });
            }
            Err(e) => {
                // a refused value never reached the keyboard; anything else may have
                // left it half-changed, so what it shows is no longer known
                let untouched = matches!(e, ServiceError::Device(DeviceError::Proto(_) | DeviceError::Unsupported(_)) | ServiceError::Unsupported(_));
                let msg = e.to_string();
                self.lit(|t| {
                    if !untouched {
                        t.shown = None;
                    }
                    if t.is_newest(id) {
                        t.status = RequestStatus::Failed(msg.clone());
                    }
                });
                match e {
                    ServiceError::Device(e) => self.fail(e),
                    _ => self.publish(|s| s.error = Some(msg)),
                }
            }
        }
    }

    /// Apply the newest pending lighting request, waiting for the firmware first so
    /// that anything arriving meanwhile can still replace it.
    pub(crate) fn apply_pending(&mut self) {
        if self.pending.is_none() && self.pending_side.is_none() {
            return;
        }
        if let Some(b) = self.board.as_deref() {
            b.settle();
        }
        if let Some(p) = self.pending.take() {
            self.apply(*p);
        }
        if let Some(side) = self.pending_side.take() {
            self.apply_side(side);
        }
    }

    fn apply_side(&mut self, side: SideLight) {
        let Some(board) = self.board.as_deref() else { return };
        if !board.features().side_light {
            return;
        }
        // Correct first, then cache and compare against what actually goes to the board.
        let side = if self.true_colors { SideLight { color: to_led(side.color), ..side } } else { side };
        if self.side_cache == Some(side) {
            return;
        }
        board.settle();
        match board.set_side_light(&side) {
            Ok(()) => self.side_cache = Some(side),
            Err(e) => self.fail(e),
        }
    }

    /// Next picture of a running sequence, once the firmware has committed the last (or,
    /// on a board that shows pictures at once, once the last has been up for a while).
    fn tick_sequence(&mut self) {
        let Some(board) = self.board.as_deref() else { return };
        if board.is_busy() || self.pending.is_some() {
            return;
        }
        let host = board.features().host_driven;
        let Some(run) = self.sequence.as_mut() else { return };
        if host && run.shown_at.elapsed() < SPELL_STEP {
            return;
        }
        run.shown_at = Instant::now();
        let (request, brightness) = (run.request, run.brightness);
        let Some(frame) = run.frames.pop_front() else { return };
        let last = run.frames.is_empty();
        if last {
            self.sequence = None;
            self.publish(|s| s.live = None);
        }
        let layer = self.live_layer;
        match self.write_frame(layer, &frame, brightness, false) {
            Ok(()) => self.lit(|t| {
                if let Some(s) = t.shown.as_mut().filter(|s| s.request == request) {
                    if let Some(p) = s.progress.as_mut() {
                        p.done = (p.done + 1).min(p.total);
                    }
                    s.running = !last;
                }
            }),
            Err(e) => {
                self.sequence = None;
                self.lit(|t| t.shown = None); // the picture may be half-written
                if let ServiceError::Device(e) = e {
                    self.fail(e);
                }
            }
        }
    }

    fn tick_live(&mut self) {
        let Some(board) = self.board.as_deref() else { return };
        if board.is_busy() {
            return;
        }
        let inputs = (self.inputs)();
        let true_colors = self.true_colors;
        let Some(run) = self.live.as_mut() else { return };
        let t = run.t0.elapsed().as_secs_f32();
        let frame = run.anim.frame(t, &inputs);
        let base = run.anim.effect().base_effect();
        let r = match frame {
            LiveFrame::Color(c) => board.stream_color(if true_colors { to_led(c) } else { c }),
            LiveFrame::Levels(l) => board.stream_bars(&l, if true_colors { to_led(base.color) } else { base.color }, base.rainbow),
        };
        match r {
            Ok(()) => {
                // what the keyboard got, in design colours, for the window's preview
                self.shared.frames.offer(LiveFrameEvent { request: run.request, seq: run.frames, t, frame });
                run.frames += 1;
            }
            Err(e) => self.fail(e),
        }
    }

    /// Next frame of an animation Keylume draws itself.
    fn tick_host(&mut self) {
        let Some(run) = self.host.as_ref() else { return };
        let keys = run.player.frame(run.t0.elapsed().as_secs_f32());
        let brightness = run.brightness;
        let frame = self.frame_of(&keys);
        if let Err(e) = self.write_frame(0, &frame, brightness, false) {
            self.host = None;
            if let ServiceError::Device(e) = e {
                self.fail(e);
            }
        }
    }

    /// A picture of key colours on the connected board.
    fn frame_of(&self, keys: &BTreeMap<String, Rgb>) -> Frame {
        self.layout.frame(|k| keys.get(&k.id).copied())
    }

    fn maintain<R>(&mut self, task: Task, f: impl FnOnce(&dyn Board) -> R) -> Result<R, ServiceError> {
        if let Some(why) = &self.paused {
            return Err(ServiceError::Paused(why.clone()));
        }
        if self.board.is_none() {
            return Err(ServiceError::NotConnected);
        }
        if task.rewrites() && self.stop_running() {
            self.publish(|s| s.live = None);
        }
        let r = f(self.board.as_deref().expect("checked above"));
        if task.rewrites() {
            // the keyboard's layers and side light may all have changed
            self.cache = [None, None, None];
            self.side_cache = None;
            if let Some(b) = self.board.as_deref() {
                b.settle();
            }
            self.read_back(if task == Task::Restore { Origin::Restored } else { Origin::Reset });
        }
        Ok(r)
    }

    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Job(job) => {
                // Keep ordering intuitive: a job sees lighting requested before it.
                self.apply_pending();
                job(self);
            }
            // requests arrive in id order; keep the newest if that ever changes
            Msg::Lighting(p) => {
                if self.pending.as_ref().is_none_or(|old| old.id < p.id) {
                    self.pending = Some(p);
                }
            }
            Msg::Side(s) => self.pending_side = Some(s),
            Msg::StopLive => {
                if self.stop_running() {
                    self.publish(|s| s.live = None);
                }
            }
            Msg::Inputs(p) => self.inputs = p,
            Msg::LiveLayer(l) => self.live_layer = l,
            Msg::TrueColors(v) => self.true_colors = v,
            Msg::Pause(reason) => self.set_paused(reason),
            Msg::Shutdown => return false,
        }
        true
    }

    fn run(mut self, rx: Receiver<Msg>) {
        loop {
            self.ensure_connected();
            let timeout = if self.live.is_some() || self.sequence.is_some() || self.host.is_some() { LIVE_TICK } else { IDLE_TICK };
            match rx.recv_timeout(timeout) {
                Ok(msg) => {
                    if !self.handle(msg) {
                        break;
                    }
                    // Drain whatever else is queued so lighting requests coalesce.
                    while let Ok(msg) = rx.try_recv() {
                        if !self.handle(msg) {
                            if let Some(b) = self.board.as_deref() {
                                b.settle();
                                b.release();
                            }
                            return;
                        }
                    }
                    if self.pending.is_some() || self.pending_side.is_some() {
                        if let Some(b) = self.board.as_deref() {
                            b.settle();
                        }
                        // Newer requests may have arrived while we waited.
                        while let Ok(msg) = rx.try_recv() {
                            if !self.handle(msg) {
                                return;
                            }
                        }
                        self.apply_pending();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.live.is_some() {
                self.tick_live();
            }
            if self.sequence.is_some() {
                self.tick_sequence();
            }
            if self.host.is_some() {
                self.tick_host();
            }
            self.check_presence();
        }
        if let Some(b) = self.board.as_deref() {
            b.settle();
            b.release();
        }
    }
}
