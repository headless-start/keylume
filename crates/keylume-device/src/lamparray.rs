//! A keyboard that implements the HID LampArray standard, over feature reports.
//!
//! No flash, no firmware effects: every colour is set by the host, so [`LampArray`] only
//! has to respect the device's own [`Attributes::min_update_interval_us`] between frames.
//! [`LampArray::open`] never touches autonomous mode; callers decide whether Keylume or
//! the device's own onboard effects drive the lights.

use std::cell::Cell;
use std::thread::sleep;
use std::time::{Duration, Instant};

use keylume_proto::lamparray::{Attributes, LampColor, LampInfo, ReportFields, ReportMap};

use crate::{DeviceError, Result};

/// Raw feature-report I/O for a LampArray device. Buffers carry the report id in byte 0,
/// like [`crate::Transport`], but LampArray's six reports each have their own length.
pub trait FeatureIo: Send {
    fn set_feature(&self, buf: &[u8]) -> std::result::Result<(), String>;
    fn get_feature(&self, buf: &mut [u8]) -> std::result::Result<usize, String>;
}

/// An `Arc`'d device is still a device: lets a test (or the app) keep its own handle to
/// a [`crate::lamparray_sim::SimLampArray`] while a [`LampArray`] built from another
/// clone runs on a different thread.
impl<T: FeatureIo + Sync> FeatureIo for std::sync::Arc<T> {
    fn set_feature(&self, buf: &[u8]) -> std::result::Result<(), String> {
        (**self).set_feature(buf)
    }
    fn get_feature(&self, buf: &mut [u8]) -> std::result::Result<usize, String> {
        (**self).get_feature(buf)
    }
}

fn report_id(r: &Option<ReportFields>) -> u8 {
    r.as_ref().expect("checked by ReportMap::validate at open").id
}

/// `GET_FEATURE` into a fresh buffer; a short read (a vanished device, or a backend that
/// drops the report id) is treated the same as the TK68's "no reply".
fn read<T: FeatureIo>(io: &T, mut buf: Vec<u8>, id: u8) -> Result<Vec<u8>> {
    let n = io.get_feature(&mut buf).map_err(DeviceError::Transport)?;
    if n != buf.len() {
        return Err(DeviceError::NoReply(id));
    }
    Ok(buf)
}

/// A LampArray keyboard: its own report map, its attributes, and every lamp's own
/// attributes, read once at [`LampArray::open`].
pub struct LampArray<T: FeatureIo> {
    io: T,
    map: ReportMap,
    attributes: Attributes,
    lamps: Vec<LampInfo>,
    /// When we last finished sending a colour frame, for [`Attributes::min_update_interval_us`].
    last_update: Cell<Instant>,
}

/// More lamps than any keyboard has: a device claiming more is refused before its lamps
/// are read (each takes two reports).
pub const MAX_LAMPS: usize = 1024;

impl<T: FeatureIo> LampArray<T> {
    /// Parse `descriptor`, confirm it declares all six LampArray reports, and read the
    /// array's attributes and every one of its lamps (request + response, id checked
    /// each time). Doesn't read or change autonomous mode.
    pub fn open(io: T, descriptor: &[u8]) -> Result<Self> {
        let map = ReportMap::parse(descriptor);
        map.validate().map_err(DeviceError::Transport)?;

        let buf = map.attributes_buffer().map_err(DeviceError::Transport)?;
        let buf = read(&io, buf, report_id(&map.attributes))?;
        let attributes = map.decode_attributes(&buf).map_err(DeviceError::Transport)?;
        if attributes.lamp_count as usize > MAX_LAMPS {
            return Err(DeviceError::Transport(format!("it says it has {} lamps; Keylume reads up to {MAX_LAMPS}", attributes.lamp_count)));
        }

        let resp_id = report_id(&map.lamp_response);
        let mut lamps = Vec::with_capacity(attributes.lamp_count as usize);
        for id in 0..attributes.lamp_count {
            let req = map.encode_lamp_request(id).map_err(DeviceError::Transport)?;
            io.set_feature(&req).map_err(DeviceError::Transport)?;
            let buf = map.response_buffer().map_err(DeviceError::Transport)?;
            let buf = read(&io, buf, resp_id)?;
            let info = map.decode_lamp_response(&buf).map_err(DeviceError::Transport)?;
            if info.id != id {
                return Err(DeviceError::Transport(format!("lamp {id}: device answered with lamp {} instead", info.id)));
            }
            lamps.push(info);
        }

        Ok(LampArray { io, map, attributes, lamps, last_update: Cell::new(Instant::now()) })
    }

    pub fn attributes(&self) -> Attributes {
        self.attributes
    }

    pub fn lamps(&self) -> &[LampInfo] {
        &self.lamps
    }

    fn min_interval(&self) -> Duration {
        Duration::from_micros(self.attributes.min_update_interval_us as u64)
    }

    /// Block until [`Attributes::min_update_interval_us`] has passed since the last
    /// frame this instance sent.
    fn wait_turn(&self) {
        let min = self.min_interval();
        let elapsed = self.last_update.get().elapsed();
        if elapsed < min {
            sleep(min - elapsed);
        }
    }

    fn mark_sent(&self) {
        self.last_update.set(Instant::now());
    }

    /// Set the given lamps' colours, as one frame: batched into as many
    /// `LampMultiUpdateReport`s as [`ReportMap::multi_capacity`] needs, with the
    /// complete flag (the device latches the colours) on the last one only. Waits out
    /// the device's minimum update interval first, so frames can be sent back to back.
    pub fn set_colors(&self, colors: &[(u16, LampColor)]) -> Result<()> {
        self.wait_turn();
        let cap = self.map.multi_capacity().max(1);
        let mut chunks = colors.chunks(cap).peekable();
        while let Some(chunk) = chunks.next() {
            let complete = chunks.peek().is_none();
            let buf = self.map.encode_multi_update(chunk, complete).map_err(DeviceError::Transport)?;
            self.io.set_feature(&buf).map_err(DeviceError::Transport)?;
        }
        self.mark_sent();
        Ok(())
    }

    /// Set every lamp to one colour, as one frame (a single range update).
    pub fn set_all(&self, color: LampColor) -> Result<()> {
        self.wait_turn();
        let last = self.attributes.lamp_count.saturating_sub(1);
        let buf = self.map.encode_range_update(0, last, color, true).map_err(DeviceError::Transport)?;
        self.io.set_feature(&buf).map_err(DeviceError::Transport)?;
        self.mark_sent();
        Ok(())
    }

    /// Switch autonomous (onboard-effect) mode. Not a colour frame: not rate-limited.
    pub fn set_autonomous(&self, on: bool) -> Result<()> {
        let buf = self.map.encode_control(on).map_err(DeviceError::Transport)?;
        self.io.set_feature(&buf).map_err(DeviceError::Transport)
    }

    /// Is the device still there? A bare GET of its attributes, changing nothing.
    pub fn probe(&self) -> Result<()> {
        let buf = self.map.attributes_buffer().map_err(DeviceError::Transport)?;
        read(&self.io, buf, report_id(&self.map.attributes))?;
        Ok(())
    }
}
