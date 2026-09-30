//! HID LampArray (usage page 0x59, "Lighting and Illumination"): the standard lighting
//! interface behind Windows Dynamic Lighting. Pure report building and parsing.
//!
//! A LampArray descriptor declares six feature reports, each its own HID collection
//! carrying a fixed usage (`LampArrayAttributesReport`, `LampAttributesRequestReport`, …).
//! [`ReportMap::parse`] walks the report descriptor once and records, for every field of
//! every one of those six reports, its report id and its bit position and size — never
//! assuming a byte layout, since the standard leaves that to each device. The six
//! encode/decode methods on [`ReportMap`] then pack and unpack buffers using that map.

use std::collections::HashMap;

use crate::Rgb;

/// Usage ids on page 0x59 ("Lighting and Illumination"). The six `*_REPORT` usages name
/// the collection that groups each report's fields; the rest are field usages.
pub mod usage {
    /// Lighting and Illumination.
    pub const PAGE: u16 = 0x59;

    pub const LAMP_ARRAY: u16 = 0x01;

    pub const ATTRIBUTES_REPORT: u16 = 0x02;
    pub const LAMP_COUNT: u16 = 0x03;
    pub const BOUNDING_BOX_WIDTH_UM: u16 = 0x04;
    pub const BOUNDING_BOX_HEIGHT_UM: u16 = 0x05;
    pub const BOUNDING_BOX_DEPTH_UM: u16 = 0x06;
    pub const LAMP_ARRAY_KIND: u16 = 0x07;
    pub const MIN_UPDATE_INTERVAL_US: u16 = 0x08;

    pub const LAMP_REQUEST_REPORT: u16 = 0x20;
    pub const LAMP_ID: u16 = 0x21;

    pub const LAMP_RESPONSE_REPORT: u16 = 0x22;
    pub const POSITION_X_UM: u16 = 0x23;
    pub const POSITION_Y_UM: u16 = 0x24;
    pub const POSITION_Z_UM: u16 = 0x25;
    pub const LAMP_PURPOSES: u16 = 0x26;
    pub const UPDATE_LATENCY_US: u16 = 0x27;
    pub const RED_LEVEL_COUNT: u16 = 0x28;
    pub const GREEN_LEVEL_COUNT: u16 = 0x29;
    pub const BLUE_LEVEL_COUNT: u16 = 0x2A;
    pub const INTENSITY_LEVEL_COUNT: u16 = 0x2B;
    pub const IS_PROGRAMMABLE: u16 = 0x2C;
    /// A HID Keyboard/Keypad page (0x07) usage; 0 when the lamp has no key.
    pub const INPUT_BINDING: u16 = 0x2D;

    pub const MULTI_UPDATE_REPORT: u16 = 0x50;
    pub const RED_UPDATE_CHANNEL: u16 = 0x51;
    pub const GREEN_UPDATE_CHANNEL: u16 = 0x52;
    pub const BLUE_UPDATE_CHANNEL: u16 = 0x53;
    pub const INTENSITY_UPDATE_CHANNEL: u16 = 0x54;
    pub const UPDATE_FLAGS: u16 = 0x55;

    pub const RANGE_UPDATE_REPORT: u16 = 0x60;
    pub const LAMP_ID_START: u16 = 0x61;
    pub const LAMP_ID_END: u16 = 0x62;

    pub const CONTROL_REPORT: u16 = 0x70;
    pub const AUTONOMOUS_MODE: u16 = 0x71;
}

/// `LampUpdateFlags`: the device latches the colours only once it sees this bit, so it
/// belongs on the last report of a frame.
pub const UPDATE_COMPLETE: u64 = 0x01;

/// `LampPurposes` bit flags.
pub mod purpose {
    pub const CONTROL: u32 = 1;
    pub const ACCENT: u32 = 2;
    pub const BRANDING: u32 = 4;
    pub const STATUS: u32 = 8;
    pub const ILLUMINATION: u32 = 16;
    pub const PRESENTATION: u32 = 32;
}

/// `LampArrayKind`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LampArrayKind {
    #[default]
    Undefined,
    Keyboard,
    Mouse,
    GameController,
    Peripheral,
    Scene,
    Notification,
    Chassis,
    Wearable,
    Furniture,
    Art,
    /// A value outside the standard's 0..=10, for future revisions.
    Other(u32),
}

impl LampArrayKind {
    pub fn from_u32(v: u32) -> Self {
        use LampArrayKind::*;
        match v {
            0 => Undefined,
            1 => Keyboard,
            2 => Mouse,
            3 => GameController,
            4 => Peripheral,
            5 => Scene,
            6 => Notification,
            7 => Chassis,
            8 => Wearable,
            9 => Furniture,
            10 => Art,
            other => Other(other),
        }
    }

    pub fn as_u32(self) -> u32 {
        use LampArrayKind::*;
        match self {
            Undefined => 0,
            Keyboard => 1,
            Mouse => 2,
            GameController => 3,
            Peripheral => 4,
            Scene => 5,
            Notification => 6,
            Chassis => 7,
            Wearable => 8,
            Furniture => 9,
            Art => 10,
            Other(v) => v,
        }
    }
}

// ---- little-endian, LSB-first bit packing (HID's field layout) ------------------------

fn get_bits(buf: &[u8], bit_offset: u32, bit_size: u32) -> u64 {
    let mut v = 0u64;
    for i in 0..bit_size {
        let bit = bit_offset + i;
        let (byte, sh) = ((bit / 8) as usize, bit % 8);
        if buf.get(byte).is_some_and(|b| (b >> sh) & 1 != 0) {
            v |= 1 << i;
        }
    }
    v
}

/// Two's-complement sign extension of a `bit_size`-wide field.
fn get_bits_signed(buf: &[u8], bit_offset: u32, bit_size: u32) -> i64 {
    let v = get_bits(buf, bit_offset, bit_size);
    if bit_size == 0 || bit_size >= 64 {
        return v as i64;
    }
    let sign = 1u64 << (bit_size - 1);
    if v & sign != 0 {
        (v as i64) - (1i64 << bit_size)
    } else {
        v as i64
    }
}

fn set_bits(buf: &mut [u8], bit_offset: u32, bit_size: u32, value: u64) {
    for i in 0..bit_size {
        let bit = bit_offset + i;
        let (byte, sh) = ((bit / 8) as usize, bit % 8);
        let Some(b) = buf.get_mut(byte) else { break };
        if (value >> i) & 1 != 0 {
            *b |= 1 << sh;
        } else {
            *b &= !(1 << sh);
        }
    }
}

// ---- report descriptor parsing ---------------------------------------------------------

/// Global item state, saved and restored together by Push/Pop.
#[derive(Clone, Copy, Default)]
struct Global {
    usage_page: u16,
    report_id: u8,
    report_size: u32,
    report_count: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Report {
    Attributes,
    LampRequest,
    LampResponse,
    MultiUpdate,
    RangeUpdate,
    Control,
}

/// Is `ext` (an extended usage: page in bits 16..32, id in bits 0..16) one of the six
/// report collections, and if so which?
fn report_for_usage(ext: u32) -> Option<Report> {
    if ext >> 16 != usage::PAGE as u32 {
        return None;
    }
    Some(match (ext & 0xFFFF) as u16 {
        usage::ATTRIBUTES_REPORT => Report::Attributes,
        usage::LAMP_REQUEST_REPORT => Report::LampRequest,
        usage::LAMP_RESPONSE_REPORT => Report::LampResponse,
        usage::MULTI_UPDATE_REPORT => Report::MultiUpdate,
        usage::RANGE_UPDATE_REPORT => Report::RangeUpdate,
        usage::CONTROL_REPORT => Report::Control,
        _ => return None,
    })
}

/// `ext`'s plain usage id, if it's on the Lighting and Illumination page.
fn page_59(ext: u32) -> Option<u16> {
    (ext >> 16 == usage::PAGE as u32).then_some((ext & 0xFFFF) as u16)
}

/// The usages a Feature item declared: the explicit list, or a Usage Minimum/Maximum
/// range (same page as the minimum) when no usages were given directly.
fn declared_usages(usages: &[u32], usage_min: Option<u32>, usage_max: Option<u32>) -> Vec<u32> {
    if !usages.is_empty() {
        return usages.to_vec();
    }
    if let (Some(mn), Some(mx)) = (usage_min, usage_max) {
        let page = mn & 0xFFFF_0000;
        let (lo, hi) = (mn & 0xFFFF, mx & 0xFFFF);
        if hi >= lo {
            return (lo..=hi).map(|id| page | id).collect();
        }
    }
    Vec::new()
}

/// The usage of occurrence `idx` of a field: `list[idx]`, or `list`'s last entry once
/// `idx` runs past it (HID: fewer usages than `ReportCount` repeats the last one). `None`
/// usages (padding/constant fields) stay `None`.
fn pick_usage(list: &[u32], idx: usize) -> Option<u32> {
    if list.is_empty() {
        None
    } else {
        Some(list[idx.min(list.len() - 1)])
    }
}

struct Reader<'a> {
    d: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    /// The next short item as `(tag, type, unsigned value, size in bytes)`. Long items
    /// (tag 0xFE; not used by LampArray descriptors) are skipped whole.
    fn next(&mut self) -> Option<(u8, u8, u32, u8)> {
        loop {
            let prefix = *self.d.get(self.i)?;
            self.i += 1;
            if prefix == 0xFE {
                let size = *self.d.get(self.i)? as usize;
                self.i = (self.i + 2 + size).min(self.d.len());
                continue;
            }
            let size = match prefix & 0x03 {
                0 => 0,
                1 => 1,
                2 => 2,
                _ => 4,
            };
            let bytes = self.d.get(self.i..self.i + size)?;
            self.i += size;
            let v = bytes.iter().enumerate().fold(0u32, |v, (k, b)| v | (*b as u32) << (8 * k));
            return Some(((prefix >> 4) & 0x0F, (prefix >> 2) & 0x03, v, size as u8));
        }
    }
}

/// One field's location within a report buffer: bit offset from the start of the buffer
/// (byte 0 is the report id), and width in bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldSlot {
    pub bit_offset: u32,
    pub bit_size: u32,
}

/// One LampArray report as a descriptor declared it: its report id, its total length in
/// bytes (including the id byte), and where every usage's occurrence(s) sit.
#[derive(Clone, Debug, Default)]
pub struct ReportFields {
    pub id: u8,
    pub len: usize,
    slots: HashMap<u16, Vec<FieldSlot>>,
}

impl ReportFields {
    /// Every bit-slot `usage` occupies in this report, in descriptor order.
    pub fn occurrences(&self, usage: u16) -> &[FieldSlot] {
        match self.slots.get(&usage) {
            Some(v) => v,
            None => &[],
        }
    }

    fn slot(&self, usage: u16, at: usize) -> Result<FieldSlot, String> {
        self.occurrences(usage)
            .get(at)
            .copied()
            .ok_or_else(|| format!("report {:#04x}: no usage {usage:#04x} at index {at}", self.id))
    }

    fn read_u(&self, buf: &[u8], usage: u16, at: usize) -> Result<u64, String> {
        let s = self.slot(usage, at)?;
        Ok(get_bits(buf, s.bit_offset, s.bit_size))
    }

    fn read_i(&self, buf: &[u8], usage: u16, at: usize) -> Result<i64, String> {
        let s = self.slot(usage, at)?;
        Ok(get_bits_signed(buf, s.bit_offset, s.bit_size))
    }

    fn write_u(&self, buf: &mut [u8], usage: u16, at: usize, v: u64) -> Result<(), String> {
        let s = self.slot(usage, at)?;
        set_bits(buf, s.bit_offset, s.bit_size, v);
        Ok(())
    }

    fn buffer(&self) -> Vec<u8> {
        let mut b = vec![0u8; self.len.max(1)];
        b[0] = self.id;
        b
    }
}

fn require<'a>(r: &'a Option<ReportFields>, name: &str) -> Result<&'a ReportFields, String> {
    r.as_ref().ok_or_else(|| format!("descriptor has no {name}"))
}

/// Every LampArray report a descriptor declares, and where their fields live.
/// [`ReportMap::parse`] never fails on its own; call [`ReportMap::validate`] before
/// treating the result as a usable device.
#[derive(Clone, Debug, Default)]
pub struct ReportMap {
    pub attributes: Option<ReportFields>,
    pub lamp_request: Option<ReportFields>,
    pub lamp_response: Option<ReportFields>,
    pub multi_update: Option<ReportFields>,
    pub range_update: Option<ReportFields>,
    pub control: Option<ReportFields>,
}

impl ReportMap {
    /// Parse a HID report descriptor's feature fields into the six LampArray reports.
    /// Reports this descriptor doesn't declare are left `None`.
    pub fn parse(descriptor: &[u8]) -> ReportMap {
        let mut g = Global::default();
        let mut gstack: Vec<Global> = Vec::new();
        let mut usages: Vec<u32> = Vec::new();
        let mut usage_min: Option<u32> = None;
        let mut usage_max: Option<u32> = None;
        let mut current: Option<Report> = None;
        let mut cstack: Vec<Option<Report>> = Vec::new();
        let mut offsets: HashMap<Report, u32> = HashMap::new();
        let mut built: HashMap<Report, ReportFields> = HashMap::new();

        let mut r = Reader { d: descriptor, i: 0 };
        while let Some((tag, ty, v, size)) = r.next() {
            match ty {
                // Global.
                1 => match tag {
                    0x0 => g.usage_page = v as u16,
                    0x7 => g.report_size = v,
                    0x8 => g.report_id = v as u8,
                    0x9 => g.report_count = v,
                    0xA => gstack.push(g),
                    0xB => {
                        if let Some(p) = gstack.pop() {
                            g = p;
                        }
                    }
                    _ => {} // logical/physical min & max, unit, unit exponent: not needed
                },
                // Local.
                2 => {
                    let ext = if size == 4 { v } else { ((g.usage_page as u32) << 16) | v };
                    match tag {
                        0x0 => usages.push(ext),
                        0x1 => usage_min = Some(ext),
                        0x2 => usage_max = Some(ext),
                        _ => {}
                    }
                }
                // Main.
                _ => {
                    match tag {
                        0x8 | 0x9 | 0xB => {
                            // Input, Output, Feature: only Feature (0xB) is ours to record,
                            // but all three consume local state and, for the report we're
                            // inside, occupy bits (Input/Output don't share Feature's layout,
                            // so we only advance the offset for 0xB).
                            if tag == 0xB {
                                if let Some(rep) = current {
                                    let list = declared_usages(&usages, usage_min, usage_max);
                                    let base = *offsets.get(&rep).unwrap_or(&0);
                                    let rf = built.entry(rep).or_default();
                                    rf.id = g.report_id;
                                    for idx in 0..g.report_count {
                                        let bit_offset = 8 + base + idx.saturating_mul(g.report_size);
                                        if let Some(u) = pick_usage(&list, idx as usize).and_then(page_59) {
                                            rf.slots.entry(u).or_default().push(FieldSlot { bit_offset, bit_size: g.report_size });
                                        }
                                    }
                                    let new_off = base.saturating_add(g.report_count.saturating_mul(g.report_size));
                                    offsets.insert(rep, new_off);
                                    rf.len = 1 + (new_off as usize).div_ceil(8);
                                }
                            }
                        }
                        0xA => {
                            let kind = usages.first().copied().and_then(report_for_usage);
                            cstack.push(current);
                            current = kind.or(current);
                        }
                        0xC => current = cstack.pop().flatten(),
                        _ => {}
                    }
                    usages.clear();
                    usage_min = None;
                    usage_max = None;
                }
            }
        }

        ReportMap {
            attributes: built.remove(&Report::Attributes),
            lamp_request: built.remove(&Report::LampRequest),
            lamp_response: built.remove(&Report::LampResponse),
            multi_update: built.remove(&Report::MultiUpdate),
            range_update: built.remove(&Report::RangeUpdate),
            control: built.remove(&Report::Control),
        }
    }

    /// All six reports LampArray needs; names what's missing otherwise.
    pub fn validate(&self) -> Result<(), String> {
        let named: [(&Option<ReportFields>, &str); 6] = [
            (&self.attributes, "LampArrayAttributesReport"),
            (&self.lamp_request, "LampAttributesRequestReport"),
            (&self.lamp_response, "LampAttributesResponseReport"),
            (&self.multi_update, "LampMultiUpdateReport"),
            (&self.range_update, "LampRangeUpdateReport"),
            (&self.control, "LampArrayControlReport"),
        ];
        let missing: Vec<&str> = named.into_iter().filter(|(r, _)| r.is_none()).map(|(_, n)| n).collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!("not a usable LampArray: missing {}", missing.join(", ")))
        }
    }

    /// Lamps one [`ReportMap::encode_multi_update`] call can carry.
    pub fn multi_capacity(&self) -> usize {
        self.multi_update.as_ref().map(|r| r.occurrences(usage::LAMP_ID).len()).unwrap_or(0)
    }

    /// A zeroed, correctly sized, report-id-stamped buffer to `GET_FEATURE` the
    /// attributes report into.
    pub fn attributes_buffer(&self) -> Result<Vec<u8>, String> {
        Ok(require(&self.attributes, "LampArrayAttributesReport")?.buffer())
    }

    pub fn decode_attributes(&self, buf: &[u8]) -> Result<Attributes, String> {
        let r = require(&self.attributes, "LampArrayAttributesReport")?;
        Ok(Attributes {
            lamp_count: r.read_u(buf, usage::LAMP_COUNT, 0)? as u16,
            bounding_box_width_um: r.read_u(buf, usage::BOUNDING_BOX_WIDTH_UM, 0)? as u32,
            bounding_box_height_um: r.read_u(buf, usage::BOUNDING_BOX_HEIGHT_UM, 0)? as u32,
            bounding_box_depth_um: r.read_u(buf, usage::BOUNDING_BOX_DEPTH_UM, 0)? as u32,
            kind: LampArrayKind::from_u32(r.read_u(buf, usage::LAMP_ARRAY_KIND, 0)? as u32),
            min_update_interval_us: r.read_u(buf, usage::MIN_UPDATE_INTERVAL_US, 0)? as u32,
        })
    }

    /// Ask the device to make lamp `id` the subject of the next
    /// `LampAttributesResponseReport` GET.
    pub fn encode_lamp_request(&self, id: u16) -> Result<Vec<u8>, String> {
        let r = require(&self.lamp_request, "LampAttributesRequestReport")?;
        let mut b = r.buffer();
        r.write_u(&mut b, usage::LAMP_ID, 0, id as u64)?;
        Ok(b)
    }

    /// A zeroed, correctly sized, report-id-stamped buffer to `GET_FEATURE` a lamp's
    /// attributes into (after [`ReportMap::encode_lamp_request`]).
    pub fn response_buffer(&self) -> Result<Vec<u8>, String> {
        Ok(require(&self.lamp_response, "LampAttributesResponseReport")?.buffer())
    }

    pub fn decode_lamp_response(&self, buf: &[u8]) -> Result<LampInfo, String> {
        let r = require(&self.lamp_response, "LampAttributesResponseReport")?;
        Ok(LampInfo {
            id: r.read_u(buf, usage::LAMP_ID, 0)? as u16,
            x_um: r.read_i(buf, usage::POSITION_X_UM, 0)? as i32,
            y_um: r.read_i(buf, usage::POSITION_Y_UM, 0)? as i32,
            z_um: r.read_i(buf, usage::POSITION_Z_UM, 0)? as i32,
            purposes: r.read_u(buf, usage::LAMP_PURPOSES, 0)? as u32,
            latency_us: r.read_u(buf, usage::UPDATE_LATENCY_US, 0)? as u32,
            red_level_count: r.read_u(buf, usage::RED_LEVEL_COUNT, 0)? as u8,
            green_level_count: r.read_u(buf, usage::GREEN_LEVEL_COUNT, 0)? as u8,
            blue_level_count: r.read_u(buf, usage::BLUE_LEVEL_COUNT, 0)? as u8,
            intensity_level_count: r.read_u(buf, usage::INTENSITY_LEVEL_COUNT, 0)? as u8,
            programmable: r.read_u(buf, usage::IS_PROGRAMMABLE, 0)? != 0,
            input_binding: r.read_u(buf, usage::INPUT_BINDING, 0)? as u16,
        })
    }

    /// `SET_FEATURE` buffer updating up to [`ReportMap::multi_capacity`] lamps. Set
    /// `complete` only on the last report of a frame, so the device latches the colours.
    pub fn encode_multi_update(&self, lamps: &[(u16, LampColor)], complete: bool) -> Result<Vec<u8>, String> {
        let r = require(&self.multi_update, "LampMultiUpdateReport")?;
        let cap = self.multi_capacity();
        if lamps.len() > cap {
            return Err(format!("{} lamps given but this report updates at most {cap}", lamps.len()));
        }
        let mut b = r.buffer();
        r.write_u(&mut b, usage::LAMP_COUNT, 0, lamps.len() as u64)?;
        r.write_u(&mut b, usage::UPDATE_FLAGS, 0, complete as u64)?;
        for (i, (id, c)) in lamps.iter().enumerate() {
            r.write_u(&mut b, usage::LAMP_ID, i, *id as u64)?;
            r.write_u(&mut b, usage::RED_UPDATE_CHANNEL, i, c.r as u64)?;
            r.write_u(&mut b, usage::GREEN_UPDATE_CHANNEL, i, c.g as u64)?;
            r.write_u(&mut b, usage::BLUE_UPDATE_CHANNEL, i, c.b as u64)?;
            r.write_u(&mut b, usage::INTENSITY_UPDATE_CHANNEL, i, c.i as u64)?;
        }
        Ok(b)
    }

    /// `SET_FEATURE` buffer painting lamps `start..=end` one colour.
    pub fn encode_range_update(&self, start: u16, end: u16, color: LampColor, complete: bool) -> Result<Vec<u8>, String> {
        let r = require(&self.range_update, "LampRangeUpdateReport")?;
        let mut b = r.buffer();
        r.write_u(&mut b, usage::UPDATE_FLAGS, 0, complete as u64)?;
        r.write_u(&mut b, usage::LAMP_ID_START, 0, start as u64)?;
        r.write_u(&mut b, usage::LAMP_ID_END, 0, end as u64)?;
        r.write_u(&mut b, usage::RED_UPDATE_CHANNEL, 0, color.r as u64)?;
        r.write_u(&mut b, usage::GREEN_UPDATE_CHANNEL, 0, color.g as u64)?;
        r.write_u(&mut b, usage::BLUE_UPDATE_CHANNEL, 0, color.b as u64)?;
        r.write_u(&mut b, usage::INTENSITY_UPDATE_CHANNEL, 0, color.i as u64)?;
        Ok(b)
    }

    /// `SET_FEATURE` buffer switching autonomous (onboard-effect) mode.
    pub fn encode_control(&self, autonomous: bool) -> Result<Vec<u8>, String> {
        let r = require(&self.control, "LampArrayControlReport")?;
        let mut b = r.buffer();
        r.write_u(&mut b, usage::AUTONOMOUS_MODE, 0, autonomous as u64)?;
        Ok(b)
    }
}

/// `LampArrayAttributesReport`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Attributes {
    pub lamp_count: u16,
    pub bounding_box_width_um: u32,
    pub bounding_box_height_um: u32,
    pub bounding_box_depth_um: u32,
    pub kind: LampArrayKind,
    pub min_update_interval_us: u32,
}

/// `LampAttributesResponseReport`: everything the device knows about one of its lamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LampInfo {
    pub id: u16,
    pub x_um: i32,
    pub y_um: i32,
    pub z_um: i32,
    /// [`purpose`] bit flags.
    pub purposes: u32,
    pub latency_us: u32,
    pub red_level_count: u8,
    pub green_level_count: u8,
    pub blue_level_count: u8,
    pub intensity_level_count: u8,
    pub programmable: bool,
    /// A HID Keyboard/Keypad page usage, or 0 when this lamp isn't under a key.
    pub input_binding: u16,
}

/// One lamp's colour, at whatever resolution its own level counts support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LampColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub i: u8,
}

/// Wire levels a channel with level-count byte `raw` supports (the standard: 0 means 256).
fn levels(raw: u8) -> u32 {
    if raw == 0 {
        256
    } else {
        raw as u32
    }
}

/// Quantise one 8-bit channel to the levels a `raw_count` byte allows: a count of 1 is
/// on/off (any non-zero value becomes the single "on" level); 0 means 256 levels, i.e.
/// full resolution; anything else scales `0..=255` onto `0..=(levels - 1)`, rounded to
/// the nearest level (so both 0 and 255 map to an exact end of the range).
fn quantise_channel(value: u8, raw_count: u8) -> u8 {
    let n = levels(raw_count);
    if n <= 1 {
        return u8::from(value > 0);
    }
    if n >= 256 {
        return value;
    }
    (((value as u32) * (n - 1) + 127) / 255) as u8
}

/// Quantise an 8-bit colour and intensity to the wire values `lamp`'s level counts allow.
pub fn quantize(rgb: Rgb, intensity: u8, lamp: &LampInfo) -> LampColor {
    LampColor {
        r: quantise_channel(rgb.0, lamp.red_level_count),
        g: quantise_channel(rgb.1, lamp.green_level_count),
        b: quantise_channel(rgb.2, lamp.blue_level_count),
        i: quantise_channel(intensity, lamp.intensity_level_count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny HID report descriptor builder: just enough item encoding to write the
    /// LampArray layouts these tests need, independent of anything a real device sends.
    struct Build(Vec<u8>);

    impl Build {
        fn new() -> Self {
            Build(Vec::new())
        }
        fn item(mut self, tag: u8, ty: u8, data: &[u8]) -> Self {
            let size_code = match data.len() {
                0 => 0,
                1 => 1,
                2 => 2,
                4 => 3,
                n => panic!("bad item size {n}"),
            };
            self.0.push((tag << 4) | (ty << 2) | size_code);
            self.0.extend_from_slice(data);
            self
        }
        fn usage_page(self, p: u16) -> Self {
            self.item(0x0, 1, &p.to_le_bytes())
        }
        fn usage(self, u: u16) -> Self {
            if u <= 0xFF {
                self.item(0x0, 2, &[u as u8])
            } else {
                self.item(0x0, 2, &u.to_le_bytes())
            }
        }
        fn report_id(self, id: u8) -> Self {
            self.item(0x8, 1, &[id])
        }
        fn report_size(self, n: u8) -> Self {
            self.item(0x7, 1, &[n])
        }
        fn report_count(self, n: u8) -> Self {
            self.item(0x9, 1, &[n])
        }
        fn collection(self, v: u8) -> Self {
            self.item(0xA, 0, &[v])
        }
        fn end_collection(self) -> Self {
            self.item(0xC, 0, &[])
        }
        fn feature(self) -> Self {
            self.item(0xB, 0, &[0x02]) // Data, Variable, Absolute
        }
        fn field(self, u: u16, size: u8, count: u8) -> Self {
            self.usage(u).report_size(size).report_count(count).feature()
        }
        /// `count` lamps' worth of interleaved R,G,B,I usages, one Feature item covering
        /// all of them (the order real descriptors are expected to use).
        fn channels_field(mut self, lamps: usize, size: u8) -> Self {
            for _ in 0..lamps {
                self = self
                    .usage(usage::RED_UPDATE_CHANNEL)
                    .usage(usage::GREEN_UPDATE_CHANNEL)
                    .usage(usage::BLUE_UPDATE_CHANNEL)
                    .usage(usage::INTENSITY_UPDATE_CHANNEL);
            }
            self.report_size(size).report_count((lamps * 4) as u8).feature()
        }
        fn build(self) -> Vec<u8> {
            self.0
        }
    }

    /// A Microsoft-style LampArray descriptor: report ids 1..=6, 16-bit lamp ids, 32-bit
    /// positions/purposes/latency, 8-bit level counts, `multi_cap` lamps per multi-update.
    fn lamparray_descriptor(multi_cap: u8, input_binding_bits: u8) -> Vec<u8> {
        Build::new()
            .usage_page(usage::PAGE)
            .usage(usage::LAMP_ARRAY)
            .collection(0x01)
            .usage(usage::ATTRIBUTES_REPORT)
            .collection(0x02)
            .report_id(1)
            .field(usage::LAMP_COUNT, 16, 1)
            .field(usage::BOUNDING_BOX_WIDTH_UM, 32, 1)
            .field(usage::BOUNDING_BOX_HEIGHT_UM, 32, 1)
            .field(usage::BOUNDING_BOX_DEPTH_UM, 32, 1)
            .field(usage::LAMP_ARRAY_KIND, 32, 1)
            .field(usage::MIN_UPDATE_INTERVAL_US, 32, 1)
            .end_collection()
            .usage(usage::LAMP_REQUEST_REPORT)
            .collection(0x02)
            .report_id(2)
            .field(usage::LAMP_ID, 16, 1)
            .end_collection()
            .usage(usage::LAMP_RESPONSE_REPORT)
            .collection(0x02)
            .report_id(3)
            .field(usage::LAMP_ID, 16, 1)
            .field(usage::POSITION_X_UM, 32, 1)
            .field(usage::POSITION_Y_UM, 32, 1)
            .field(usage::POSITION_Z_UM, 32, 1)
            .field(usage::LAMP_PURPOSES, 32, 1)
            .field(usage::UPDATE_LATENCY_US, 32, 1)
            .field(usage::RED_LEVEL_COUNT, 8, 1)
            .field(usage::GREEN_LEVEL_COUNT, 8, 1)
            .field(usage::BLUE_LEVEL_COUNT, 8, 1)
            .field(usage::INTENSITY_LEVEL_COUNT, 8, 1)
            .field(usage::IS_PROGRAMMABLE, 8, 1)
            .field(usage::INPUT_BINDING, input_binding_bits, 1)
            .end_collection()
            .usage(usage::MULTI_UPDATE_REPORT)
            .collection(0x02)
            .report_id(4)
            .field(usage::LAMP_COUNT, 8, 1)
            .field(usage::UPDATE_FLAGS, 8, 1)
            .field(usage::LAMP_ID, 16, multi_cap)
            .channels_field(multi_cap as usize, 8)
            .end_collection()
            .usage(usage::RANGE_UPDATE_REPORT)
            .collection(0x02)
            .report_id(5)
            .field(usage::UPDATE_FLAGS, 8, 1)
            .field(usage::LAMP_ID_START, 16, 1)
            .field(usage::LAMP_ID_END, 16, 1)
            .channels_field(1, 8)
            .end_collection()
            .usage(usage::CONTROL_REPORT)
            .collection(0x02)
            .report_id(6)
            .field(usage::AUTONOMOUS_MODE, 8, 1)
            .end_collection()
            .end_collection()
            .build()
    }

    #[test]
    fn parses_the_standard_layout() {
        let map = ReportMap::parse(&lamparray_descriptor(8, 8));
        map.validate().unwrap();
        assert_eq!(map.multi_capacity(), 8);
        let ids: [u8; 6] = [&map.attributes, &map.lamp_request, &map.lamp_response, &map.multi_update, &map.range_update, &map.control]
            .map(|r| r.as_ref().unwrap().id);
        assert_eq!(ids, [1, 2, 3, 4, 5, 6]);
        // LampCount(16) + 5×32-bit fields = 176 bits = 22 bytes, + the id byte.
        assert_eq!(map.attributes.as_ref().unwrap().len, 23);
    }

    #[test]
    fn sixteen_bit_input_binding_and_small_multi_update() {
        let map = ReportMap::parse(&lamparray_descriptor(4, 16));
        map.validate().unwrap();
        assert_eq!(map.multi_capacity(), 4);
        let r = map.lamp_response.as_ref().unwrap();
        let mut buf = r.buffer();
        r.write_u(&mut buf, usage::INPUT_BINDING, 0, 0x1234).unwrap();
        assert_eq!(map.decode_lamp_response(&buf).unwrap().input_binding, 0x1234);
        let too_many = vec![(0u16, LampColor::default()); 5];
        assert!(map.encode_multi_update(&too_many, false).is_err());
    }

    #[test]
    fn usage_items_of_every_byte_size_resolve_to_the_same_field() {
        fn control_only(usage_item: &[u8]) -> Vec<u8> {
            let mut d = Build::new()
                .usage_page(usage::PAGE)
                .usage(usage::LAMP_ARRAY)
                .collection(0x01)
                .usage(usage::CONTROL_REPORT)
                .collection(0x02)
                .report_id(9)
                .build();
            d.extend_from_slice(usage_item);
            d.extend(Build::new().report_size(8).report_count(1).feature().end_collection().end_collection().build());
            d
        }
        let one = Build::new().item(0x0, 2, &[usage::AUTONOMOUS_MODE as u8]).build();
        let two = Build::new().item(0x0, 2, &usage::AUTONOMOUS_MODE.to_le_bytes()).build();
        let ext = ((usage::PAGE as u32) << 16) | usage::AUTONOMOUS_MODE as u32;
        let four = Build::new().item(0x0, 2, &ext.to_le_bytes()).build();
        for item in [one, two, four] {
            let map = ReportMap::parse(&control_only(&item));
            assert_eq!(map.control.unwrap().id, 9);
        }
    }

    #[test]
    fn missing_report_is_unusable() {
        let d = Build::new()
            .usage_page(usage::PAGE)
            .usage(usage::LAMP_ARRAY)
            .collection(0x01)
            .usage(usage::ATTRIBUTES_REPORT)
            .collection(0x02)
            .report_id(1)
            .field(usage::LAMP_COUNT, 16, 1)
            .end_collection()
            .end_collection()
            .build();
        let map = ReportMap::parse(&d);
        assert!(map.attributes.is_some());
        assert!(map.control.is_none());
        let err = map.validate().unwrap_err();
        assert!(err.contains("LampArrayControlReport"), "{err}");
        assert!(err.contains("LampMultiUpdateReport"), "{err}");
    }

    #[test]
    fn decodes_attributes() {
        let map = ReportMap::parse(&lamparray_descriptor(8, 8));
        let r = map.attributes.as_ref().unwrap();
        let mut buf = map.attributes_buffer().unwrap();
        assert_eq!(buf[0], 1);
        r.write_u(&mut buf, usage::LAMP_COUNT, 0, 68).unwrap();
        r.write_u(&mut buf, usage::BOUNDING_BOX_WIDTH_UM, 0, 300_000_000).unwrap();
        r.write_u(&mut buf, usage::BOUNDING_BOX_HEIGHT_UM, 0, 100_000_000).unwrap();
        r.write_u(&mut buf, usage::BOUNDING_BOX_DEPTH_UM, 0, 10_000_000).unwrap();
        r.write_u(&mut buf, usage::LAMP_ARRAY_KIND, 0, 1).unwrap();
        r.write_u(&mut buf, usage::MIN_UPDATE_INTERVAL_US, 0, 16_666).unwrap();
        let a = map.decode_attributes(&buf).unwrap();
        assert_eq!(
            a,
            Attributes {
                lamp_count: 68,
                bounding_box_width_um: 300_000_000,
                bounding_box_height_um: 100_000_000,
                bounding_box_depth_um: 10_000_000,
                kind: LampArrayKind::Keyboard,
                min_update_interval_us: 16_666,
            }
        );
    }

    #[test]
    fn decodes_lamp_response_with_negative_positions() {
        let map = ReportMap::parse(&lamparray_descriptor(8, 8));
        let r = map.lamp_response.as_ref().unwrap();
        let mut buf = map.response_buffer().unwrap();
        assert_eq!(buf[0], 3);
        r.write_u(&mut buf, usage::LAMP_ID, 0, 5).unwrap();
        r.write_u(&mut buf, usage::POSITION_X_UM, 0, (-1000i32 as u32) as u64).unwrap();
        r.write_u(&mut buf, usage::POSITION_Y_UM, 0, 2000).unwrap();
        r.write_u(&mut buf, usage::POSITION_Z_UM, 0, 0).unwrap();
        r.write_u(&mut buf, usage::LAMP_PURPOSES, 0, purpose::ACCENT as u64).unwrap();
        r.write_u(&mut buf, usage::UPDATE_LATENCY_US, 0, 1000).unwrap();
        r.write_u(&mut buf, usage::INTENSITY_LEVEL_COUNT, 0, 1).unwrap();
        r.write_u(&mut buf, usage::IS_PROGRAMMABLE, 0, 1).unwrap();
        r.write_u(&mut buf, usage::INPUT_BINDING, 0, 0x04).unwrap();
        let info = map.decode_lamp_response(&buf).unwrap();
        assert_eq!(
            info,
            LampInfo {
                id: 5,
                x_um: -1000,
                y_um: 2000,
                z_um: 0,
                purposes: purpose::ACCENT,
                latency_us: 1000,
                red_level_count: 0,
                green_level_count: 0,
                blue_level_count: 0,
                intensity_level_count: 1,
                programmable: true,
                input_binding: 4,
            }
        );
    }

    #[test]
    fn encodes_lamp_request() {
        let map = ReportMap::parse(&lamparray_descriptor(8, 8));
        let buf = map.encode_lamp_request(300).unwrap();
        assert_eq!(buf[0], 2);
        assert_eq!(map.lamp_request.as_ref().unwrap().read_u(&buf, usage::LAMP_ID, 0).unwrap(), 300);
    }

    #[test]
    fn multi_update_batches_within_capacity() {
        let map = ReportMap::parse(&lamparray_descriptor(8, 8));
        let lamps = [(0u16, LampColor { r: 10, g: 20, b: 30, i: 255 }), (1, LampColor { r: 1, g: 2, b: 3, i: 4 })];
        let buf = map.encode_multi_update(&lamps, true).unwrap();
        assert_eq!(buf[0], 4);
        let r = map.multi_update.as_ref().unwrap();
        assert_eq!(r.read_u(&buf, usage::LAMP_COUNT, 0).unwrap(), 2);
        assert_eq!(r.read_u(&buf, usage::UPDATE_FLAGS, 0).unwrap(), 1);
        assert_eq!(r.read_u(&buf, usage::LAMP_ID, 0).unwrap(), 0);
        assert_eq!(r.read_u(&buf, usage::LAMP_ID, 1).unwrap(), 1);
        assert_eq!(r.read_u(&buf, usage::RED_UPDATE_CHANNEL, 1).unwrap(), 1);
        assert_eq!(r.read_u(&buf, usage::GREEN_UPDATE_CHANNEL, 1).unwrap(), 2);
        assert_eq!(r.read_u(&buf, usage::BLUE_UPDATE_CHANNEL, 1).unwrap(), 3);
        assert_eq!(r.read_u(&buf, usage::INTENSITY_UPDATE_CHANNEL, 1).unwrap(), 4);

        let not_complete = map.encode_multi_update(&lamps, false).unwrap();
        assert_eq!(r.read_u(&not_complete, usage::UPDATE_FLAGS, 0).unwrap(), 0);

        let nine = vec![(0u16, LampColor::default()); 9];
        assert!(map.encode_multi_update(&nine, true).is_err(), "9 lamps exceeds this report's capacity of 8");
    }

    #[test]
    fn encodes_range_update() {
        let map = ReportMap::parse(&lamparray_descriptor(8, 8));
        let buf = map.encode_range_update(2, 9, LampColor { r: 1, g: 2, b: 3, i: 4 }, true).unwrap();
        assert_eq!(buf[0], 5);
        let r = map.range_update.as_ref().unwrap();
        assert_eq!(r.read_u(&buf, usage::LAMP_ID_START, 0).unwrap(), 2);
        assert_eq!(r.read_u(&buf, usage::LAMP_ID_END, 0).unwrap(), 9);
        assert_eq!(r.read_u(&buf, usage::UPDATE_FLAGS, 0).unwrap(), 1);
        assert_eq!(r.read_u(&buf, usage::BLUE_UPDATE_CHANNEL, 0).unwrap(), 3);
    }

    #[test]
    fn encodes_control() {
        let map = ReportMap::parse(&lamparray_descriptor(8, 8));
        let on = map.encode_control(true).unwrap();
        assert_eq!(on[0], 6);
        assert_eq!(map.control.as_ref().unwrap().read_u(&on, usage::AUTONOMOUS_MODE, 0).unwrap(), 1);
        let off = map.encode_control(false).unwrap();
        assert_eq!(map.control.as_ref().unwrap().read_u(&off, usage::AUTONOMOUS_MODE, 0).unwrap(), 0);
    }

    #[test]
    fn quantises_to_level_counts() {
        // a count of 0 means 256 levels (no quantisation) at any input value
        assert_eq!(quantise_channel(37, 0), 37);
        // a count of 1 is on/off
        assert_eq!(quantise_channel(0, 1), 0);
        assert_eq!(quantise_channel(1, 1), 1);
        assert_eq!(quantise_channel(255, 1), 1);
        // the extremes of the input range always reach the extremes of the level range
        for n in 2u8..=250 {
            assert_eq!(quantise_channel(0, n), 0, "count {n}");
            assert_eq!(quantise_channel(255, n), n - 1, "count {n}");
        }

        let lamp = LampInfo { red_level_count: 0, green_level_count: 1, blue_level_count: 3, intensity_level_count: 255, ..LampInfo::default() };
        let c = quantize(Rgb(10, 10, 255), 0, &lamp);
        assert_eq!(c, LampColor { r: 10, g: 1, b: 2, i: 0 });
    }
}
