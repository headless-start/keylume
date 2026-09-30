//! Which connected HID devices are boards from [`crate::boards`] that Keylume may drive.
//!
//! The TK68 reuses Apple's vendor id (0x05AC) and the product id of Apple's own wired
//! Aluminium Keyboard (0x024F), so the ids alone prove nothing. Before anything is
//! written, a device must also be the right interface and collection, report the
//! manufacturer and product strings its board file lists, and declare the protocol's
//! feature report in its HID report descriptor. Reading strings and descriptors sends
//! nothing to the device. More than one match is refused as well: the app drives one
//! keyboard and can't tell which one was meant.

use crate::boards::{self, BoardDef};

/// A board description (kept under its old name for the identity checks).
pub type Model = BoardDef;

/// One HID collection as the OS lists it (plain data, so identification is testable).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub vid: u16,
    pub pid: u16,
    pub interface: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub manufacturer: String,
    pub product: String,
    pub path: String,
}

impl Candidate {
    /// The board whose ids, interface and collection this is (not yet proof it's one).
    pub fn model(&self) -> Option<&'static Model> {
        boards::all().iter().find(|m| {
            let u = &m.usb;
            u.vid == self.vid && u.pid == self.pid && u.interface == self.interface && u.usage_page == self.usage_page && u.usage == self.usage
        })
    }
}

/// Is `c`, with report descriptor `descriptor`, really the model its ids claim?
/// Err explains why not, in words for the UI and the CLI.
pub fn identify(c: &Candidate, descriptor: Option<&[u8]>) -> Result<&'static Model, String> {
    let m = c.model().ok_or("not a supported keyboard's configuration interface")?;
    let u = &m.usb;
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b);
    if !same(&c.manufacturer, &u.manufacturer) || !u.products.iter().any(|p| same(&c.product, p)) {
        return Err(format!(
            "it has the {} ids but calls itself {:?} by {:?}, not {:?} by {:?}",
            m.name, c.product, c.manufacturer, u.products[0], u.manufacturer
        ));
    }
    let desc = descriptor.ok_or("its HID descriptor can't be read")?;
    let reports = feature_reports(desc)?;
    if reports != [(0, u.feature_report)] {
        return Err(format!("its HID descriptor declares feature reports {reports:?}, not one of {} bytes without an id", u.feature_report));
    }
    Ok(m)
}

/// The one device to open among `found` (each with its verdict from [`identify`]).
/// Err when none is verified or when several are.
pub fn choose<'a>(found: &'a [(Candidate, Result<&'static Model, String>)]) -> Result<(&'a Candidate, &'static Model), Choice> {
    let ok: Vec<_> = found.iter().filter_map(|(c, v)| v.as_ref().ok().map(|m| (c, *m))).collect();
    match ok.len() {
        1 => Ok(ok[0]),
        0 => match found.iter().find_map(|(_, v)| v.as_ref().err()) {
            Some(why) => Err(Choice::Unrecognised(why.clone())),
            None => Err(Choice::None),
        },
        n => Err(Choice::Ambiguous(n)),
    }
}

/// Why no device was chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Nothing with a supported keyboard's ids is connected.
    None,
    /// Something with the ids is connected but isn't the keyboard.
    Unrecognised(String),
    /// Several supported keyboards are connected.
    Ambiguous(usize),
}

/// Feature reports a HID report descriptor declares, as (report id, bytes), in id order.
/// Report id 0 means the descriptor uses no report ids.
pub fn feature_reports(desc: &[u8]) -> Result<Vec<(u8, usize)>, String> {
    #[derive(Clone, Copy, Default)]
    struct Globals {
        size: u32,
        count: u32,
        id: u8,
    }
    let mut g = Globals::default();
    let mut stack: Vec<Globals> = Vec::new();
    let mut bits: std::collections::BTreeMap<u8, u64> = Default::default();
    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            // long item: size, tag, data (none are defined; skip)
            let len = *desc.get(i + 1).ok_or("truncated long item")? as usize;
            i += 3 + len;
            continue;
        }
        let len = [0, 1, 2, 4][(prefix & 0x03) as usize];
        let data = desc.get(i + 1..i + 1 + len).ok_or("truncated item")?;
        let value = data.iter().rev().fold(0u32, |v, b| v << 8 | *b as u32);
        match prefix & 0xFC {
            0xB0 => *bits.entry(g.id).or_default() += g.size as u64 * g.count as u64, // Feature
            0x74 => g.size = value,                                                   // Report Size
            0x94 => g.count = value,                                                  // Report Count
            0x84 => g.id = u8::try_from(value).map_err(|_| "report id out of range")?, // Report ID
            0xA4 => stack.push(g),                                                    // Push
            0xB4 => g = stack.pop().ok_or("pop without push")?,                       // Pop
            _ => {}
        }
        i += 1 + len;
    }
    Ok(bits.into_iter().map(|(id, b)| (id, b.div_ceil(8) as usize)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keyboard collection with the TK68's configuration report: boot keyboard input,
    /// LED output, and a 64-byte vendor feature report, no report ids.
    pub(crate) const TK68_DESCRIPTOR: &[u8] = &[
        0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, // Generic Desktop, Keyboard, Collection (Application)
        0x05, 0x07, 0x19, 0xE0, 0x29, 0xE7, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, // modifiers
        0x95, 0x01, 0x75, 0x08, 0x81, 0x01, // reserved byte
        0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x95, 0x05, 0x75, 0x01, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01, // LEDs
        0x05, 0x07, 0x19, 0x00, 0x2A, 0xFF, 0x00, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x95, 0x06, 0x75, 0x08, 0x81, 0x00, // keys
        0x06, 0x00, 0xFF, 0x09, 0x01, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x40, 0xB1, 0x02, // 64-byte feature
        0xC0,
    ];

    /// Apple's wired Aluminium Keyboard: the same ids, a plain boot keyboard collection.
    const APPLE_DESCRIPTOR: &[u8] = &[
        0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x05, 0x07, 0x19, 0xE0, 0x29, 0xE7, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95,
        0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x05, 0x75, 0x01, 0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x01,
        0x95, 0x06, 0x75, 0x08, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x05, 0x07, 0x19, 0x00, 0x2A, 0xFF, 0x00, 0x81, 0x00, 0xC0,
    ];

    pub(crate) fn tk68() -> Candidate {
        Candidate {
            vid: 0x05AC,
            pid: 0x024F,
            interface: 0,
            usage_page: 0x01,
            usage: 0x06,
            manufacturer: "ROYUAN".into(),
            product: "Acrylic68".into(),
            path: "tk68".into(),
        }
    }

    fn apple() -> Candidate {
        Candidate { manufacturer: "Apple Inc.".into(), product: "Apple Keyboard".into(), path: "apple".into(), ..tk68() }
    }

    #[test]
    fn reads_feature_reports_from_descriptors() {
        assert_eq!(feature_reports(TK68_DESCRIPTOR).unwrap(), [(0, 64)]);
        assert_eq!(feature_reports(APPLE_DESCRIPTOR).unwrap(), []);
        // report ids, push/pop and a truncated item
        let with_ids = [0x85, 0x05, 0x75, 0x08, 0x95, 0x10, 0xB1, 0x02, 0xA4, 0x85, 0x06, 0x95, 0x20, 0xB1, 0x02, 0xB4, 0xB1, 0x02];
        assert_eq!(feature_reports(&with_ids).unwrap(), [(5, 32), (6, 32)]);
        assert!(feature_reports(&[0x75]).is_err());
        assert!(feature_reports(&[0xB4]).is_err());
    }

    #[test]
    fn accepts_the_verified_tk68() {
        assert_eq!(identify(&tk68(), Some(TK68_DESCRIPTOR)).unwrap().id, "epomaker-tk68");
        // strings compare ignoring case and padding
        let loose = Candidate { manufacturer: " royuan ".into(), product: "ACRYLIC68".into(), ..tk68() };
        assert!(identify(&loose, Some(TK68_DESCRIPTOR)).is_ok());
    }

    #[test]
    fn refuses_a_device_with_the_same_ids_that_is_not_a_tk68() {
        let why = identify(&apple(), Some(APPLE_DESCRIPTOR)).unwrap_err();
        assert!(why.contains("Apple Keyboard"), "{why}");
        // right strings, wrong descriptor (no 64-byte feature report): still refused
        assert!(identify(&tk68(), Some(APPLE_DESCRIPTOR)).is_err());
        assert!(identify(&tk68(), None).is_err(), "an unreadable descriptor proves nothing");
        // another interface or collection of the same keyboard isn't the config channel
        let media = Candidate { interface: 1, usage_page: 0x0C, usage: 0x01, ..tk68() };
        assert!(identify(&media, Some(TK68_DESCRIPTOR)).is_err());
    }

    #[test]
    fn chooses_exactly_one_keyboard() {
        let verdict = |c: Candidate, d: &[u8]| {
            let v = identify(&c, Some(d));
            (c, v)
        };
        let one = [verdict(apple(), APPLE_DESCRIPTOR), verdict(tk68(), TK68_DESCRIPTOR)];
        assert_eq!(choose(&one).unwrap().0.path, "tk68", "the Apple keyboard beside it is ignored");
        let only_apple = [verdict(apple(), APPLE_DESCRIPTOR)];
        assert!(matches!(choose(&only_apple), Err(Choice::Unrecognised(_))));
        let two = [verdict(tk68(), TK68_DESCRIPTOR), verdict(Candidate { path: "tk68-b".into(), ..tk68() }, TK68_DESCRIPTOR)];
        assert_eq!(choose(&two).unwrap_err(), Choice::Ambiguous(2));
        assert_eq!(choose(&[]).unwrap_err(), Choice::None);
    }
}
