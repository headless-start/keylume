//! A HID LampArray keyboard (the standard behind Windows Dynamic Lighting) as a [`Board`].
//! Keylume drives every lamp itself: pictures show at once, animations are drawn frame by
//! frame (the service's job), and nothing is stored on the keyboard. The layout comes
//! from the keyboard's own description of its lamps (position and the key above each).

use std::sync::Arc;

use keylume_device::hid::lamparray_info;
use keylume_device::identity::Candidate;
use keylume_device::lamparray::{FeatureIo, LampArray};
use keylume_device::lamparray_sim::SimLampArray;
use keylume_device::{DeviceInfo, Result};
use keylume_proto::lamparray::{purpose, quantize, LampArrayKind, LampColor, LampInfo};
use keylume_proto::picture::Frame;
use keylume_proto::standard::{self, Lamp, UNIT_UM};
use keylume_proto::stream::BANDS;
use keylume_proto::{Layout, Rgb};

use crate::board::Board;

pub struct LampArrayBoard<T: FeatureIo> {
    lamps: LampArray<T>,
    info: DeviceInfo,
    layout: Layout,
}

/// A channel at `level` of 4 (brightness), scaled on the host so it works on every board.
fn scale(v: u8, level: u8) -> u8 {
    (v as u32 * level.min(4) as u32 / 4) as u8
}

impl<T: FeatureIo> LampArrayBoard<T> {
    /// Take control of the lamps: the keyboard stops its own lighting until
    /// [`Board::release`].
    pub fn new(lamps: LampArray<T>, info: DeviceInfo) -> Result<Self> {
        let spots: Vec<Lamp> = lamps
            .lamps()
            .iter()
            .map(|l| Lamp {
                slot: l.id as usize,
                x_um: l.x_um as f32,
                y_um: l.y_um as f32,
                usage: l.input_binding,
                key: l.input_binding != 0 || l.purposes & purpose::CONTROL != 0,
            })
            .collect();
        let layout = standard::from_lamps(&info.board, &info.name, &spots);
        lamps.set_autonomous(false)?;
        Ok(LampArrayBoard { lamps, info, layout })
    }

    fn colors(&self, frame: &Frame, brightness: u8) -> Vec<(u16, LampColor)> {
        self.lamps
            .lamps()
            .iter()
            .map(|l| {
                let c = frame.0.get(l.id as usize).copied().unwrap_or_default();
                (l.id, quantize(Rgb(scale(c.0, brightness), scale(c.1, brightness), scale(c.2, brightness)), 255, l))
            })
            .collect()
    }
}

impl<T: FeatureIo> Board for LampArrayBoard<T> {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }
    fn layout(&self) -> &Layout {
        &self.layout
    }
    fn write_picture(&self, _layer: u8, frame: &Frame, brightness: u8) -> Result<()> {
        self.lamps.set_colors(&self.colors(frame, brightness))
    }
    fn stream_color(&self, c: Rgb) -> Result<()> {
        let first = self.lamps.lamps().first().copied().unwrap_or_default();
        self.lamps.set_all(quantize(c, 255, &first))
    }
    fn stream_bars(&self, levels: &[u8; BANDS], color: Rgb, rainbow: bool) -> Result<()> {
        let keys = keylume_live::effects::bars(&self.layout, levels, color, rainbow);
        self.write_picture(0, &self.layout.frame(|k| keys.get(&k.id).copied()), 4)
    }
    fn probe(&self) -> Result<()> {
        self.lamps.probe()
    }
    fn release(&self) {
        let _ = self.lamps.set_autonomous(true);
    }
}

/// The demo keyboard: a full-size board whose every key has a lamp, plus an underglow
/// strip along its front edge, described the way a real LampArray keyboard describes
/// itself. `sim` is the simulated device, for tests to look at.
pub fn demo() -> (LampArrayBoard<Arc<SimLampArray>>, Arc<SimLampArray>) {
    let full = standard::standard("ansi-full").expect("built in");
    let um = |v: f32| (v * UNIT_UM) as i32;
    let lamp = |x: f32, y: f32, purposes: u32, binding: u16| LampInfo {
        x_um: um(x),
        y_um: um(y),
        purposes,
        latency_us: 1000,
        programmable: true,
        input_binding: binding,
        ..Default::default()
    };
    let mut lamps: Vec<LampInfo> =
        full.keys.iter().map(|k| lamp(k.x + k.w / 2.0 + 0.25, k.y + k.h / 2.0 + 0.25, purpose::CONTROL, k.hid as u16)).collect();
    for i in 0..16 {
        lamps.push(lamp(0.75 + i as f32 * (full.width - 1.0) / 15.0, full.height + 0.45, purpose::ACCENT, 0));
    }
    let bbox = (um(full.width + 0.5) as u32, um(full.height + 0.7) as u32, um(1.5) as u32);
    let sim = Arc::new(SimLampArray::new(lamps, LampArrayKind::Keyboard, bbox, 8, 0));
    let descriptor = sim.descriptor().to_vec();
    let device = LampArray::open(sim.clone(), &descriptor).expect("the simulator answers");
    let candidate = Candidate {
        vid: 0,
        pid: 1,
        interface: 0,
        usage_page: keylume_proto::lamparray::usage::PAGE,
        usage: keylume_proto::lamparray::usage::LAMP_ARRAY,
        manufacturer: "Keylume".into(),
        product: "Full-size keyboard (simulated)".into(),
        path: "sim://lamparray".into(),
    };
    let board = LampArrayBoard::new(device, lamparray_info(&candidate, true)).expect("the simulator answers");
    (board, sim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_keyboard_is_laid_out_from_its_lamps() {
        let (board, sim) = demo();
        let l = board.layout();
        assert_eq!(l.keys.len(), 104, "every key lamp became a key");
        assert_eq!(l.hidden_leds.len(), 16, "the underglow follows the keys nearest it");
        let full = standard::standard("ansi-full").unwrap();
        assert_eq!((l.width, l.height), (full.width, full.height));
        assert!(!sim.autonomous(), "Keylume took control of the lamps");
        board.release();
        assert!(sim.autonomous(), "and handed them back");
    }

    #[test]
    fn pictures_and_live_frames_reach_the_lamps() {
        let (board, sim) = demo();
        let red = Rgb(255, 0, 0);
        let frame = board.layout().frame(|k| (k.id == "esc").then_some(red));
        board.write_picture(0, &frame, 4).unwrap();
        let esc = board.layout().key("esc").unwrap().slot;
        let got = sim.colors();
        assert_eq!((got[esc].r, got[esc].g, got[esc].b), (255, 0, 0));
        assert_eq!(got.iter().filter(|c| c.r == 255).count(), 1, "only Esc: the underglow sits by the bottom row");
        let space = board.layout().frame(|k| (k.id == "space").then_some(red));
        board.write_picture(0, &space, 4).unwrap();
        assert!(sim.colors().iter().filter(|c| c.r == 255).count() > 1, "the underglow below the space bar follows it");
        // half brightness halves the light, on the host
        board.write_picture(0, &frame, 2).unwrap();
        assert_eq!(sim.colors()[esc].r, 127);
        board.stream_color(Rgb(0, 0, 200)).unwrap();
        assert!(sim.colors().iter().all(|c| (c.r, c.g, c.b) == (0, 0, 200)));
        board.stream_bars(&[6; BANDS], Rgb(0, 255, 0), false).unwrap();
        assert!(sim.colors().iter().all(|c| (c.r, c.g, c.b) == (0, 255, 0)), "full bars light every key");
        board.probe().unwrap();
    }
}
