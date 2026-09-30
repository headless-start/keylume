//! What the keyboard is showing, as far as Keylume knows it.
//!
//! The device worker owns this: a look counts as shown only once the keyboard accepted
//! it. Every lighting request gets an id, and the state keeps the newest request apart
//! from what's on the keys, so a slow older request can never overwrite a newer choice.
//! Live effects also publish their frames (the ones actually sent) through a
//! [`FrameTap`] the worker never waits on.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

use keylume_live::LiveFrame;
use keylume_profiles::Lighting;
use serde::Serialize;

/// Where the lighting stands, in one word for the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// The newest request is queued (rapid changes coalesce here).
    Requested,
    /// The newest request is being written to the keyboard.
    Pending,
    /// The keyboard shows `shown`.
    Applied,
    /// The newest request failed (`error`); `shown` is what's still known to be there.
    Failed,
    /// The lights are off.
    Off,
    /// Connected, but what the keyboard shows couldn't be read.
    Unknown,
    Disconnected,
    /// Another app has the keyboard.
    Paused,
}

/// Where what's shown came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// A profile from the library.
    Profile,
    /// An unsaved design from an editor.
    Preview,
    Off,
    /// A backup was restored.
    Restored,
    /// A factory reset.
    Reset,
    /// Read back from the keyboard (on connecting).
    Keyboard,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub id: u64,
    /// The profile asked for (None for editor previews and lights off).
    pub profile_id: Option<String>,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: u32,
    pub total: u32,
}

/// What the keys show, known because the keyboard accepted it (or it was read back).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    /// The request that put it there (0 when it didn't come from one).
    pub request: u64,
    pub origin: Origin,
    pub profile_id: Option<String>,
    pub name: String,
    /// In the colours it was designed with: True colours are applied on the way to the
    /// keyboard, never here. None when the lights are off.
    pub lighting: Option<Lighting>,
    /// 0..=4 as the keyboard has it (0 is dark).
    pub brightness: u8,
    /// A live effect is streaming, or a spell is still spelling. False once stopped: the
    /// keyboard then holds the last frame.
    pub running: bool,
    /// Spells: pictures written so far, of how many.
    pub progress: Option<Progress>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingState {
    /// Bumped on every change: keep the newest.
    pub seq: u64,
    pub phase: Phase,
    /// The newest lighting request.
    pub request: Option<Request>,
    /// Why the newest request failed.
    pub error: Option<String>,
    /// What the keys show (None: unknown, or nothing connected).
    pub shown: Option<Shown>,
}

/// The worker's view, from which [`LightingState`] is published.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Tracker {
    pub seq: u64,
    pub connected: bool,
    pub paused: Option<String>,
    pub request: Option<Request>,
    pub status: RequestStatus,
    pub shown: Option<Shown>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum RequestStatus {
    #[default]
    None,
    Requested,
    Pending,
    Done,
    Failed(String),
}

impl Tracker {
    pub fn state(&self) -> LightingState {
        let off = self.shown.as_ref().is_some_and(|s| s.origin == Origin::Off || s.brightness == 0 || is_off_effect(s));
        let phase = if self.paused.is_some() {
            Phase::Paused
        } else if !self.connected {
            Phase::Disconnected
        } else {
            match self.status {
                RequestStatus::Requested => Phase::Requested,
                RequestStatus::Pending => Phase::Pending,
                RequestStatus::Failed(_) => Phase::Failed,
                RequestStatus::Done | RequestStatus::None if off => Phase::Off,
                RequestStatus::Done | RequestStatus::None if self.shown.is_some() => Phase::Applied,
                _ => Phase::Unknown,
            }
        };
        let error = match &self.status {
            RequestStatus::Failed(e) => Some(e.clone()),
            _ => None,
        };
        LightingState { seq: self.seq, phase, request: self.request.clone(), error, shown: self.shown.clone() }
    }

    /// Is `id` still the newest request?
    pub fn is_newest(&self, id: u64) -> bool {
        self.request.as_ref().is_some_and(|r| r.id == id)
    }
}

fn is_off_effect(s: &Shown) -> bool {
    matches!(&s.lighting, Some(Lighting::Effect { effect }) if effect.mode == keylume_proto::Mode::Off)
}

/// One frame a live effect sent to the keyboard.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveFrameEvent {
    /// The request whose effect this is.
    pub request: u64,
    /// Counts frames of that request, from 0.
    pub seq: u64,
    /// The effect's clock, in seconds.
    pub t: f32,
    /// In design colours (before True colours), as [`Shown::lighting`].
    pub frame: LiveFrame,
}

/// The newest live frame, for the window's preview. Latest wins: the worker offers each
/// frame without ever waiting, and a reader that falls behind just skips frames.
#[derive(Default)]
pub struct FrameTap {
    slot: Mutex<Option<LiveFrameEvent>>,
    ready: Condvar,
}

impl FrameTap {
    /// Never blocks: if a reader holds the slot this very moment, the frame is skipped.
    pub(crate) fn offer(&self, ev: LiveFrameEvent) {
        if let Ok(mut slot) = self.slot.try_lock() {
            *slot = Some(ev);
            self.ready.notify_all();
        }
    }

    pub fn latest(&self) -> Option<LiveFrameEvent> {
        self.slot.lock().unwrap().clone()
    }

    /// Wait up to `timeout` for a frame other than `last` (request id, frame seq).
    pub fn next(&self, last: (u64, u64), timeout: Duration) -> Option<LiveFrameEvent> {
        let slot = self.slot.lock().unwrap();
        let (slot, _) = self.ready.wait_timeout_while(slot, timeout, |s| s.as_ref().is_none_or(|e| (e.request, e.seq) == last)).unwrap();
        slot.clone().filter(|e| (e.request, e.seq) != last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keylume_proto::Rgb;
    use std::sync::Arc;

    fn shown(origin: Origin, brightness: u8) -> Shown {
        Shown { request: 1, origin, profile_id: None, name: "x".into(), lighting: None, brightness, running: false, progress: None }
    }

    #[test]
    fn phases_follow_the_device_then_the_newest_request() {
        let mut t = Tracker::default();
        assert_eq!(t.state().phase, Phase::Disconnected);
        t.connected = true;
        assert_eq!(t.state().phase, Phase::Unknown);
        t.shown = Some(shown(Origin::Profile, 4));
        assert_eq!(t.state().phase, Phase::Applied);
        t.status = RequestStatus::Requested;
        assert_eq!(t.state().phase, Phase::Requested);
        t.status = RequestStatus::Failed("nope".into());
        let s = t.state();
        assert_eq!((s.phase, s.error.as_deref()), (Phase::Failed, Some("nope")));
        assert!(s.shown.is_some(), "a failure keeps what's known to be on the keys");
        t.status = RequestStatus::Done;
        t.shown = Some(shown(Origin::Profile, 0));
        assert_eq!(t.state().phase, Phase::Off, "brightness 0 is dark");
        t.paused = Some("EPOMAKER Driver is running".into());
        assert_eq!(t.state().phase, Phase::Paused);
    }

    #[test]
    fn the_frame_tap_hands_out_the_newest_frame() {
        let tap = Arc::new(FrameTap::default());
        let ev = |seq| LiveFrameEvent { request: 7, seq, t: seq as f32 * 0.04, frame: LiveFrame::Color(Rgb(seq as u8, 0, 0)) };
        assert!(tap.next((0, 0), Duration::from_millis(5)).is_none());
        tap.offer(ev(1));
        tap.offer(ev(2));
        assert_eq!(tap.next((0, 0), Duration::from_millis(5)).unwrap().seq, 2, "older frames are skipped");
        assert!(tap.next((7, 2), Duration::from_millis(5)).is_none(), "nothing newer yet");
        let t = tap.clone();
        let reader = std::thread::spawn(move || t.next((7, 2), Duration::from_secs(2)).map(|e| e.seq));
        std::thread::sleep(Duration::from_millis(20));
        tap.offer(ev(3));
        assert_eq!(reader.join().unwrap(), Some(3), "a waiting reader wakes up");
    }
}
