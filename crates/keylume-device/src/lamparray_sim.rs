//! A simulated HID LampArray keyboard, for tests and the demo keyboard.
//!
//! Builds its own report descriptor (Microsoft-style: report ids 1..=6, a configurable
//! multi-update capacity) and, independently of `keylume-proto`'s bit-packer, knows the
//! plain byte-aligned layout it chose for every field — so decoding a request here is a
//! genuinely separate implementation from encoding one in `keylume-proto` or the device
//! client, and a bug in either side shows up as an end-to-end test failure.

use std::sync::Mutex;

use keylume_proto::lamparray::{usage, LampArrayKind, LampColor, LampInfo};

use crate::lamparray::FeatureIo;

fn item(tag: u8, ty: u8, data: &[u8]) -> Vec<u8> {
    let size_code = match data.len() {
        0 => 0,
        1 => 1,
        2 => 2,
        4 => 3,
        n => panic!("bad item size {n}"),
    };
    let mut v = vec![(tag << 4) | (ty << 2) | size_code];
    v.extend_from_slice(data);
    v
}
fn usage_page(p: u16) -> Vec<u8> {
    item(0x0, 1, &p.to_le_bytes())
}
fn usage(u: u16) -> Vec<u8> {
    item(0x0, 2, &[u as u8]) // every usage this file needs fits a byte
}
fn report_id(id: u8) -> Vec<u8> {
    item(0x8, 1, &[id])
}
fn report_size(n: u8) -> Vec<u8> {
    item(0x7, 1, &[n])
}
fn report_count(n: u8) -> Vec<u8> {
    item(0x9, 1, &[n])
}
fn collection(v: u8) -> Vec<u8> {
    item(0xA, 0, &[v])
}
fn end_collection() -> Vec<u8> {
    item(0xC, 0, &[])
}
fn feature() -> Vec<u8> {
    item(0xB, 0, &[0x02]) // Data, Variable, Absolute
}
fn field(u: u16, size: u8, count: u8) -> Vec<u8> {
    [usage(u), report_size(size), report_count(count), feature()].concat()
}

/// Bytes of a `LampMultiUpdateReport` for `cap` lamps: id + count + flags + `cap` lamp
/// ids (16-bit) + `cap` interleaved R,G,B,I channels (8-bit each).
fn multi_update_len(cap: u8) -> usize {
    3 + 6 * cap as usize
}

/// A Microsoft-style LampArray descriptor: report ids 1..=6, 16-bit lamp ids, 32-bit
/// positions/purposes/latency, 8-bit level counts, 16-bit input binding, `multi_cap`
/// lamps per multi-update. Every field is byte-aligned, matching the fixed offsets
/// [`SimLampArray`]'s `FeatureIo` impl reads and writes directly.
fn build_descriptor(multi_cap: u8) -> Vec<u8> {
    let mut channels = Vec::new();
    for _ in 0..multi_cap {
        channels.extend(usage(usage::RED_UPDATE_CHANNEL));
        channels.extend(usage(usage::GREEN_UPDATE_CHANNEL));
        channels.extend(usage(usage::BLUE_UPDATE_CHANNEL));
        channels.extend(usage(usage::INTENSITY_UPDATE_CHANNEL));
    }
    [
        usage_page(usage::PAGE),
        usage(usage::LAMP_ARRAY),
        collection(0x01),
        usage(usage::ATTRIBUTES_REPORT),
        collection(0x02),
        report_id(1),
        field(usage::LAMP_COUNT, 16, 1),
        field(usage::BOUNDING_BOX_WIDTH_UM, 32, 1),
        field(usage::BOUNDING_BOX_HEIGHT_UM, 32, 1),
        field(usage::BOUNDING_BOX_DEPTH_UM, 32, 1),
        field(usage::LAMP_ARRAY_KIND, 32, 1),
        field(usage::MIN_UPDATE_INTERVAL_US, 32, 1),
        end_collection(),
        usage(usage::LAMP_REQUEST_REPORT),
        collection(0x02),
        report_id(2),
        field(usage::LAMP_ID, 16, 1),
        end_collection(),
        usage(usage::LAMP_RESPONSE_REPORT),
        collection(0x02),
        report_id(3),
        field(usage::LAMP_ID, 16, 1),
        field(usage::POSITION_X_UM, 32, 1),
        field(usage::POSITION_Y_UM, 32, 1),
        field(usage::POSITION_Z_UM, 32, 1),
        field(usage::LAMP_PURPOSES, 32, 1),
        field(usage::UPDATE_LATENCY_US, 32, 1),
        field(usage::RED_LEVEL_COUNT, 8, 1),
        field(usage::GREEN_LEVEL_COUNT, 8, 1),
        field(usage::BLUE_LEVEL_COUNT, 8, 1),
        field(usage::INTENSITY_LEVEL_COUNT, 8, 1),
        field(usage::IS_PROGRAMMABLE, 8, 1),
        field(usage::INPUT_BINDING, 16, 1),
        end_collection(),
        usage(usage::MULTI_UPDATE_REPORT),
        collection(0x02),
        report_id(4),
        field(usage::LAMP_COUNT, 8, 1),
        field(usage::UPDATE_FLAGS, 8, 1),
        field(usage::LAMP_ID, 16, multi_cap),
        channels,
        report_size(8),
        report_count(multi_cap.saturating_mul(4)),
        feature(),
        end_collection(),
        usage(usage::RANGE_UPDATE_REPORT),
        collection(0x02),
        report_id(5),
        field(usage::UPDATE_FLAGS, 8, 1),
        field(usage::LAMP_ID_START, 16, 1),
        field(usage::LAMP_ID_END, 16, 1),
        usage(usage::RED_UPDATE_CHANNEL),
        usage(usage::GREEN_UPDATE_CHANNEL),
        usage(usage::BLUE_UPDATE_CHANNEL),
        usage(usage::INTENSITY_UPDATE_CHANNEL),
        report_size(8),
        report_count(4),
        feature(),
        end_collection(),
        usage(usage::CONTROL_REPORT),
        collection(0x02),
        report_id(6),
        field(usage::AUTONOMOUS_MODE, 8, 1),
        end_collection(),
        end_collection(),
    ]
    .concat()
}

/// Counters exposed for tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SimStats {
    pub gets: u32,
    pub sets: u32,
    pub multi_updates: u32,
    pub range_updates: u32,
    /// Malformed or out-of-range writes this device refused.
    pub rejected: u32,
}

struct Inner {
    colors: Vec<LampColor>,
    autonomous: bool,
    /// The lamp id the next `LampAttributesResponseReport` GET answers with; a SET of
    /// `LampAttributesRequestReport` overwrites it, and each response GET auto-increments
    /// it (capped at the last lamp), per the standard.
    next_request: u16,
    stats: SimStats,
    /// The `LampUpdateComplete` flag of every accepted multi-update, in order (tests
    /// check it's set on the last report of a batch only).
    multi_update_log: Vec<bool>,
}

/// A simulated LampArray device: a fixed lamp table (positions, purposes, level counts,
/// input bindings) plus the live colours and autonomous flag a host can change.
pub struct SimLampArray {
    descriptor: Vec<u8>,
    lamps: Vec<LampInfo>,
    kind: LampArrayKind,
    bounding_box_um: (u32, u32, u32),
    min_update_interval_us: u32,
    multi_cap: u8,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for SimLampArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimLampArray").field("lamps", &self.colors().len()).finish()
    }
}

impl SimLampArray {
    /// `lamps`' own `id` fields are overwritten with their position in the table, since
    /// that's what the wire protocol (and this device) actually indexes by. `multi_cap`
    /// is how many lamps one `LampMultiUpdateReport` carries.
    pub fn new(lamps: Vec<LampInfo>, kind: LampArrayKind, bounding_box_um: (u32, u32, u32), multi_cap: u8, min_update_interval_us: u32) -> Self {
        let lamps: Vec<LampInfo> = lamps.into_iter().enumerate().map(|(i, l)| LampInfo { id: i as u16, ..l }).collect();
        let n = lamps.len();
        SimLampArray {
            descriptor: build_descriptor(multi_cap),
            lamps,
            kind,
            bounding_box_um,
            min_update_interval_us,
            multi_cap,
            inner: Mutex::new(Inner {
                colors: vec![LampColor::default(); n],
                autonomous: false,
                next_request: 0,
                stats: SimStats::default(),
                multi_update_log: Vec::new(),
            }),
        }
    }

    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    /// Every lamp's current colour, in lamp id order.
    pub fn colors(&self) -> Vec<LampColor> {
        self.inner.lock().unwrap().colors.clone()
    }

    pub fn autonomous(&self) -> bool {
        self.inner.lock().unwrap().autonomous
    }

    pub fn stats(&self) -> SimStats {
        self.inner.lock().unwrap().stats.clone()
    }

    pub fn multi_update_log(&self) -> Vec<bool> {
        self.inner.lock().unwrap().multi_update_log.clone()
    }

    fn last_lamp(&self) -> u16 {
        self.lamps.len().saturating_sub(1) as u16
    }
}

impl FeatureIo for SimLampArray {
    fn set_feature(&self, buf: &[u8]) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        g.stats.sets += 1;
        let Some(&id) = buf.first() else {
            g.stats.rejected += 1;
            return Err("empty report".into());
        };
        match id {
            2 => {
                if buf.len() != 3 {
                    g.stats.rejected += 1;
                    return Err(format!("report 2: expected 3 bytes, got {}", buf.len()));
                }
                let lamp = u16::from_le_bytes([buf[1], buf[2]]);
                if lamp as usize >= self.lamps.len() {
                    g.stats.rejected += 1;
                    return Err(format!("lamp {lamp} does not exist"));
                }
                g.next_request = lamp;
                Ok(())
            }
            4 => {
                let expected = multi_update_len(self.multi_cap);
                if buf.len() != expected {
                    g.stats.rejected += 1;
                    return Err(format!("report 4: expected {expected} bytes, got {}", buf.len()));
                }
                let count = buf[1] as usize;
                if count > self.multi_cap as usize {
                    g.stats.rejected += 1;
                    return Err(format!("report 4: {count} lamps exceeds this device's capacity of {}", self.multi_cap));
                }
                let complete = buf[2] & 1 != 0;
                let ch_base = 3 + 2 * self.multi_cap as usize;
                for i in 0..count {
                    let lamp = u16::from_le_bytes([buf[3 + i * 2], buf[4 + i * 2]]);
                    if lamp as usize >= self.lamps.len() {
                        g.stats.rejected += 1;
                        return Err(format!("lamp {lamp} does not exist"));
                    }
                    let o = ch_base + i * 4;
                    g.colors[lamp as usize] = LampColor { r: buf[o], g: buf[o + 1], b: buf[o + 2], i: buf[o + 3] };
                }
                g.multi_update_log.push(complete);
                g.stats.multi_updates += 1;
                Ok(())
            }
            5 => {
                if buf.len() != 10 {
                    g.stats.rejected += 1;
                    return Err(format!("report 5: expected 10 bytes, got {}", buf.len()));
                }
                let start = u16::from_le_bytes([buf[2], buf[3]]);
                let end = u16::from_le_bytes([buf[4], buf[5]]);
                if end < start || end as usize >= self.lamps.len() {
                    g.stats.rejected += 1;
                    return Err(format!("report 5: bad lamp range {start}..={end}"));
                }
                let color = LampColor { r: buf[6], g: buf[7], b: buf[8], i: buf[9] };
                for lamp in start..=end {
                    g.colors[lamp as usize] = color;
                }
                g.stats.range_updates += 1;
                Ok(())
            }
            6 => {
                if buf.len() != 2 {
                    g.stats.rejected += 1;
                    return Err(format!("report 6: expected 2 bytes, got {}", buf.len()));
                }
                g.autonomous = buf[1] != 0;
                Ok(())
            }
            other => {
                g.stats.rejected += 1;
                Err(format!("report {other} is unknown or read-only"))
            }
        }
    }

    fn get_feature(&self, buf: &mut [u8]) -> Result<usize, String> {
        let mut g = self.inner.lock().unwrap();
        g.stats.gets += 1;
        match buf.first().copied() {
            Some(1) => {
                if buf.len() != 23 {
                    g.stats.rejected += 1;
                    return Err(format!("report 1: buffer is {} bytes, expected 23", buf.len()));
                }
                buf[1..3].copy_from_slice(&(self.lamps.len() as u16).to_le_bytes());
                buf[3..7].copy_from_slice(&self.bounding_box_um.0.to_le_bytes());
                buf[7..11].copy_from_slice(&self.bounding_box_um.1.to_le_bytes());
                buf[11..15].copy_from_slice(&self.bounding_box_um.2.to_le_bytes());
                buf[15..19].copy_from_slice(&self.kind.as_u32().to_le_bytes());
                buf[19..23].copy_from_slice(&self.min_update_interval_us.to_le_bytes());
                Ok(buf.len())
            }
            Some(3) => {
                if buf.len() != 30 {
                    g.stats.rejected += 1;
                    return Err(format!("report 3: buffer is {} bytes, expected 30", buf.len()));
                }
                let lamp = &self.lamps[(g.next_request as usize).min(self.lamps.len().saturating_sub(1))];
                buf[1..3].copy_from_slice(&lamp.id.to_le_bytes());
                buf[3..7].copy_from_slice(&lamp.x_um.to_le_bytes());
                buf[7..11].copy_from_slice(&lamp.y_um.to_le_bytes());
                buf[11..15].copy_from_slice(&lamp.z_um.to_le_bytes());
                buf[15..19].copy_from_slice(&lamp.purposes.to_le_bytes());
                buf[19..23].copy_from_slice(&lamp.latency_us.to_le_bytes());
                buf[23] = lamp.red_level_count;
                buf[24] = lamp.green_level_count;
                buf[25] = lamp.blue_level_count;
                buf[26] = lamp.intensity_level_count;
                buf[27] = lamp.programmable as u8;
                buf[28..30].copy_from_slice(&lamp.input_binding.to_le_bytes());
                let last = self.last_lamp();
                g.next_request = (g.next_request + 1).min(last);
                Ok(buf.len())
            }
            Some(other) => {
                g.stats.rejected += 1;
                Err(format!("report {other} is unknown or write-only"))
            }
            None => {
                g.stats.rejected += 1;
                Err("empty buffer".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use keylume_proto::lamparray::purpose;
    use keylume_proto::standard;

    use super::*;
    use crate::lamparray::LampArray;

    fn lamp(id: u16) -> LampInfo {
        LampInfo { id, ..LampInfo::default() }
    }

    #[test]
    fn opens_and_reads_attributes_and_lamps() {
        let lamps = vec![lamp(0), LampInfo { input_binding: 0x04, programmable: true, ..lamp(0) }];
        let sim = SimLampArray::new(lamps, LampArrayKind::Keyboard, (100_000, 40_000, 5_000), 8, 0);
        let descriptor = sim.descriptor().to_vec();
        let dev = LampArray::open(sim, &descriptor).unwrap();
        assert_eq!(dev.attributes().lamp_count, 2);
        assert_eq!(dev.attributes().kind, LampArrayKind::Keyboard);
        assert_eq!(dev.attributes().bounding_box_width_um, 100_000);
        assert_eq!(dev.lamps().len(), 2);
        assert_eq!(dev.lamps()[1].input_binding, 0x04);
        assert!(dev.lamps()[1].programmable);
    }

    /// ~20 lamps from a real key layout (some under keys, some accent lights with no
    /// binding), opened and driven from another thread while the main thread keeps its
    /// own handle to the simulator — proving `SimLampArray` works behind an `Arc` shared
    /// across threads.
    #[test]
    fn end_to_end_over_a_tkl_layout_from_another_thread() {
        let tkl = standard::standard("ansi-tkl").unwrap();
        let mut lamps: Vec<LampInfo> = tkl
            .keys
            .iter()
            .take(18)
            .map(|k| LampInfo {
                x_um: ((k.x + k.w / 2.0) * standard::UNIT_UM) as i32,
                y_um: ((k.y + k.h / 2.0) * standard::UNIT_UM) as i32,
                purposes: purpose::CONTROL,
                programmable: true,
                input_binding: k.hid as u16,
                ..lamp(0)
            })
            .collect();
        for _ in 0..2 {
            lamps.push(LampInfo { purposes: purpose::ACCENT, programmable: true, ..lamp(0) });
            // no key above these
        }
        assert_eq!(lamps.len(), 20);

        let sim = Arc::new(SimLampArray::new(lamps, LampArrayKind::Keyboard, (400_000, 150_000, 10_000), 8, 0));
        let descriptor = sim.descriptor().to_vec();
        let for_thread = sim.clone();
        let colors: Vec<(u16, LampColor)> = (0..20).map(|i| (i, LampColor { r: i as u8, g: 1, b: 2, i: 255 })).collect();
        let for_send = colors.clone();
        std::thread::spawn(move || {
            let dev = LampArray::open(for_thread, &descriptor).unwrap();
            assert_eq!(dev.lamps().iter().filter(|l| l.input_binding != 0).count(), 18);
            assert_eq!(dev.lamps().iter().filter(|l| l.input_binding == 0).count(), 2);
            dev.set_colors(&for_send).unwrap();
        })
        .join()
        .unwrap();

        let seen = sim.colors();
        for (id, want) in colors {
            assert_eq!(seen[id as usize], want, "lamp {id}");
        }
    }

    #[test]
    fn multi_update_batches_across_reports() {
        let lamps: Vec<LampInfo> = (0..13).map(lamp).collect();
        let sim = Arc::new(SimLampArray::new(lamps, LampArrayKind::Keyboard, (0, 0, 0), 8, 0));
        let descriptor = sim.descriptor().to_vec();
        let dev = LampArray::open(sim.clone(), &descriptor).unwrap();

        let colors: Vec<(u16, LampColor)> = (0..13).map(|i| (i, LampColor { r: i as u8, g: 0, b: 0, i: 255 })).collect();
        dev.set_colors(&colors).unwrap();

        assert_eq!(sim.stats().multi_updates, 2, "13 lamps at capacity 8 needs 2 reports");
        assert_eq!(sim.multi_update_log(), vec![false, true], "the complete flag is set on the last report only");
        let seen = sim.colors();
        for (id, want) in colors {
            assert_eq!(seen[id as usize], want, "lamp {id}");
        }
    }

    #[test]
    fn set_all_paints_every_lamp_with_one_range_update() {
        let lamps: Vec<LampInfo> = (0..6).map(lamp).collect();
        let sim = Arc::new(SimLampArray::new(lamps, LampArrayKind::Keyboard, (0, 0, 0), 8, 0));
        let descriptor = sim.descriptor().to_vec();
        let dev = LampArray::open(sim.clone(), &descriptor).unwrap();

        dev.set_all(LampColor { r: 5, g: 6, b: 7, i: 8 }).unwrap();
        assert_eq!(sim.stats().range_updates, 1);
        assert!(sim.colors().iter().all(|c| *c == LampColor { r: 5, g: 6, b: 7, i: 8 }));
    }

    #[test]
    fn autonomous_mode_and_probe() {
        let sim = SimLampArray::new(vec![lamp(0)], LampArrayKind::Keyboard, (0, 0, 0), 8, 0);
        let descriptor = sim.descriptor().to_vec();
        let dev = LampArray::open(sim, &descriptor).unwrap();
        dev.probe().unwrap();
        dev.set_autonomous(true).unwrap();
        // dev owns the (non-Arc) sim here, so check state through the device's own probe
        // instead; the Arc-based tests above check simulator state directly.
        dev.set_autonomous(false).unwrap();
    }

    #[test]
    fn autonomous_flag_is_visible_on_the_simulator() {
        let sim = Arc::new(SimLampArray::new(vec![lamp(0)], LampArrayKind::Keyboard, (0, 0, 0), 8, 0));
        let descriptor = sim.descriptor().to_vec();
        let dev = LampArray::open(sim.clone(), &descriptor).unwrap();
        assert!(!sim.autonomous());
        dev.set_autonomous(true).unwrap();
        assert!(sim.autonomous());
    }

    #[test]
    fn respects_minimum_update_interval() {
        let lamps: Vec<LampInfo> = (0..2).map(lamp).collect();
        let sim = SimLampArray::new(lamps, LampArrayKind::Keyboard, (0, 0, 0), 8, 20_000); // 20ms: fast, but real
        let descriptor = sim.descriptor().to_vec();
        let dev = LampArray::open(sim, &descriptor).unwrap();
        dev.set_all(LampColor::default()).unwrap();
        let start = Instant::now();
        dev.set_all(LampColor { r: 1, ..LampColor::default() }).unwrap();
        assert!(start.elapsed() >= Duration::from_micros(20_000));
    }

    #[test]
    fn rejects_malformed_and_out_of_range_writes() {
        let lamps: Vec<LampInfo> = (0..4).map(lamp).collect();
        let sim = SimLampArray::new(lamps, LampArrayKind::Keyboard, (0, 0, 0), 8, 0);

        assert!(sim.set_feature(&[4, 0, 0]).is_err(), "wrong length for its report id");
        assert!(sim.set_feature(&[99, 0]).is_err(), "unknown report id");
        let mut oob_request = vec![0u8; 3];
        oob_request[0] = 2;
        oob_request[1..3].copy_from_slice(&9u16.to_le_bytes());
        assert!(sim.set_feature(&oob_request).is_err(), "lamp 9 does not exist among 4 lamps");
        let mut oob_range = vec![0u8; 10];
        oob_range[0] = 5;
        oob_range[4..6].copy_from_slice(&9u16.to_le_bytes());
        assert!(sim.set_feature(&oob_range).is_err(), "range end 9 is out of range");

        assert_eq!(sim.stats().rejected, 4);
        assert_eq!(sim.stats().sets, 4, "every rejected call still counts as a set");
    }

    #[test]
    fn descriptor_reports_the_configured_capacity() {
        let sim = SimLampArray::new(vec![lamp(0); 20], LampArrayKind::Keyboard, (0, 0, 0), 5, 0);
        let map = keylume_proto::lamparray::ReportMap::parse(sim.descriptor());
        map.validate().unwrap();
        assert_eq!(map.multi_capacity(), 5);
    }
}
