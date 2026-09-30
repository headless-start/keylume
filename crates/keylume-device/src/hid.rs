//! The real USB transport (hidapi).
//!
//! Opening a keyboard only reads what the OS lists and the report descriptor; nothing is
//! sent to a device until [`crate::identity`] has confirmed it's exactly one supported
//! keyboard.

use hidapi::{HidApi, HidDevice, MAX_REPORT_DESCRIPTOR_SIZE};
use keylume_proto::packet::REPORT_LEN;

use crate::identity::{self, Candidate, Choice, Model};
use crate::lamparray::{FeatureIo, LampArray};
use crate::{boards, DeviceError, DeviceInfo, Keyboard, Result, Timing, Transport};
use keylume_proto::lamparray::{usage, LampArrayKind};

pub struct HidTransport(HidDevice);

impl Transport for HidTransport {
    fn set_feature(&self, buf: &[u8; REPORT_LEN + 1]) -> std::result::Result<(), String> {
        self.0.send_feature_report(buf).map_err(|e| e.to_string())
    }

    fn get_feature(&self, buf: &mut [u8; REPORT_LEN + 1]) -> std::result::Result<usize, String> {
        buf[0] = 0;
        self.0.get_feature_report(buf).map_err(|e| e.to_string())
    }
}

fn candidate(d: &hidapi::DeviceInfo) -> Candidate {
    Candidate {
        vid: d.vendor_id(),
        pid: d.product_id(),
        interface: d.interface_number(),
        usage_page: d.usage_page(),
        usage: d.usage(),
        manufacturer: d.manufacturer_string().unwrap_or_default().to_string(),
        product: d.product_string().unwrap_or_default().to_string(),
        path: d.path().to_string_lossy().into_owned(),
    }
}

/// HID collections that carry a supported keyboard's ids (identity not yet checked).
pub fn candidates(api: &HidApi) -> Vec<Candidate> {
    api.device_list().map(candidate).filter(|c| c.model().is_some()).collect()
}

fn open_path(api: &HidApi, path: &str) -> Result<HidDevice> {
    let path = std::ffi::CString::new(path).map_err(|_| DeviceError::Transport("bad device path".into()))?;
    api.open_path(&path).map_err(|e| DeviceError::Transport(e.to_string()))
}

/// The report descriptor of an opened device (a read: nothing is sent to the keyboard).
fn descriptor(dev: &HidDevice) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; MAX_REPORT_DESCRIPTOR_SIZE];
    let n = dev.get_report_descriptor(&mut buf).ok()?;
    buf.truncate(n);
    Some(buf)
}

/// Every candidate with its verdict: the model it proved to be, or why it isn't one.
pub fn identify_all(api: &HidApi) -> Vec<(Candidate, std::result::Result<&'static Model, String>)> {
    candidates(api)
        .into_iter()
        .map(|c| {
            let desc = open_path(api, &c.path).ok().and_then(|d| descriptor(&d));
            let verdict = identity::identify(&c, desc.as_deref());
            (c, verdict)
        })
        .collect()
}

fn info(c: &Candidate, m: &Model) -> DeviceInfo {
    DeviceInfo::of(m, &c.product, &c.manufacturer, &c.path, false)
}

/// Connected keyboards that passed every identity check.
pub fn discover(api: &HidApi) -> Vec<DeviceInfo> {
    identify_all(api).iter().filter_map(|(c, v)| v.as_ref().ok().map(|m| info(c, m))).collect()
}

/// Feature reports with their own report ids, for the LampArray standard.
pub struct HidFeatures(HidDevice);

impl FeatureIo for HidFeatures {
    fn set_feature(&self, buf: &[u8]) -> std::result::Result<(), String> {
        self.0.send_feature_report(buf).map_err(|e| e.to_string())
    }

    fn get_feature(&self, buf: &mut [u8]) -> std::result::Result<usize, String> {
        self.0.get_feature_report(buf).map_err(|e| e.to_string())
    }
}

/// HID collections that say they're a LampArray (the standard behind Windows Dynamic
/// Lighting), keyboards or not.
pub fn lamparray_candidates(api: &HidApi) -> Vec<Candidate> {
    api.device_list().filter(|d| d.usage_page() == usage::PAGE && d.usage() == usage::LAMP_ARRAY).map(candidate).collect()
}

/// Open the first LampArray keyboard: its descriptor must declare every LampArray report
/// and its attributes must say it's a keyboard. Opening reads its description of every
/// lamp (the standard asks for each with a request report); it changes no lights.
pub fn open_lamparray(api: &HidApi) -> Result<(LampArray<HidFeatures>, DeviceInfo)> {
    let mut refused = None;
    for c in lamparray_candidates(api) {
        let dev = match open_path(api, &c.path) {
            Ok(d) => d,
            Err(e) => {
                refused = Some(permission_hint(&c, &e));
                continue;
            }
        };
        let Some(desc) = descriptor(&dev) else { continue };
        let Ok(lamps) = LampArray::open(HidFeatures(dev), &desc) else { continue };
        if lamps.attributes().kind != LampArrayKind::Keyboard {
            continue;
        }
        return Ok((lamps, lamparray_info(&c, false)));
    }
    Err(refused.unwrap_or(DeviceError::NotFound))
}

/// What Keylume calls a LampArray keyboard: its own USB strings, and the features every
/// LampArray keyboard gets.
pub fn lamparray_info(c: &Candidate, simulated: bool) -> DeviceInfo {
    let board = format!("lamparray-{:04x}-{:04x}", c.vid, c.pid);
    let name = if c.product.trim().is_empty() { "LampArray keyboard".to_string() } else { c.product.trim().to_string() };
    DeviceInfo {
        board: board.clone(),
        name,
        maker: c.manufacturer.trim().to_string(),
        support: boards::Support::Experimental,
        features: boards::lamparray_features(),
        layout_id: board,
        vid: c.vid,
        pid: c.pid,
        product: c.product.clone(),
        manufacturer: c.manufacturer.clone(),
        path: c.path.clone(),
        simulated,
    }
}

/// Why a keyboard that's there couldn't be opened, with the fix where there is one.
fn permission_hint(c: &Candidate, e: &DeviceError) -> DeviceError {
    let text = e.to_string();
    if cfg!(target_os = "linux") && text.to_lowercase().contains("permission") {
        return DeviceError::Permission(format!(
            "{} is connected, but Linux doesn't let Keylume use it yet. Save this line as /etc/udev/rules.d/71-keylume-local.rules (as root), then unplug and replug it: {}",
            if c.product.trim().is_empty() { "A lighting keyboard" } else { c.product.trim() },
            udev_rule(c.vid, c.pid)
        ));
    }
    DeviceError::Transport(text)
}

/// A udev rule that lets the logged-in user open one keyboard's HID interfaces (Linux).
pub fn udev_rule(vid: u16, pid: u16) -> String {
    format!(r#"SUBSYSTEM=="hidraw", ATTRS{{idVendor}}=="{vid:04x}", ATTRS{{idProduct}}=="{pid:04x}", MODE="0660", TAG+="uaccess""#)
}

/// Open the one verified keyboard. Refuses when none is verified, or when several are.
pub fn open(api: &mut HidApi) -> Result<Keyboard<HidTransport>> {
    api.refresh_devices().map_err(|e| DeviceError::Transport(e.to_string()))?;
    let found = identify_all(api);
    let (c, m) = identity::choose(&found).map_err(|e| match e {
        Choice::None => DeviceError::NotFound,
        Choice::Unrecognised(why) => DeviceError::Unrecognised(why),
        Choice::Ambiguous(n) => DeviceError::Ambiguous(n),
    })?;
    let dev = open_path(api, &c.path)?;
    Ok(Keyboard::new(HidTransport(dev), info(c, m), Timing::TK68))
}
