//! Lighting effects (command 0x07, CK8).
//!
//! `07 mode (4-speed) brightness flags R G B [ck8]`
//! where `flags = direction << 4 | (rainbow ? 8 : 7)` for normal effects.

use serde::{Deserialize, Serialize};

use crate::packet::{ck8, cmd, Packet, REPORT_LEN};
use crate::{ProtoError, Rgb};

pub const MAX_SPEED: u8 = 4;
pub const MAX_BRIGHTNESS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum Mode {
    Off = 0x00,
    Static = 0x01,
    Breathing = 0x02,
    Spectrum = 0x03,
    Wave = 0x04,
    Ripple = 0x05,
    Raindrop = 0x06,
    Snake = 0x07,
    Reactive = 0x08,
    Converge = 0x09,
    SineWave = 0x0A,
    Kaleidoscope = 0x0B,
    LineWave = 0x0C,
    UserPicture = 0x0D,
    Laser = 0x0E,
    CircleWave = 0x0F,
    Dazzle = 0x10,
    RainDown = 0x11,
    Meteor = 0x12,
    ReactiveOff = 0x13,
    MusicBars = 0x14,
    ScreenSync = 0x15,
    MusicPulse = 0x16,
}

/// Static description of a mode, used by the UI to decide which controls to show.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeInfo {
    pub mode: Mode,
    pub name: &'static str,
    pub has_color: bool,
    pub has_speed: bool,
    pub has_rainbow: bool,
    pub directions: &'static [&'static str],
    /// Needs the host to stream data (audio levels / screen colour) to look alive.
    pub host_driven: bool,
}

impl Mode {
    pub const ALL: [Mode; 23] = [
        Mode::Off,
        Mode::Static,
        Mode::Breathing,
        Mode::Spectrum,
        Mode::Wave,
        Mode::Ripple,
        Mode::Raindrop,
        Mode::Snake,
        Mode::Reactive,
        Mode::Converge,
        Mode::SineWave,
        Mode::Kaleidoscope,
        Mode::LineWave,
        Mode::UserPicture,
        Mode::Laser,
        Mode::CircleWave,
        Mode::Dazzle,
        Mode::RainDown,
        Mode::Meteor,
        Mode::ReactiveOff,
        Mode::MusicBars,
        Mode::ScreenSync,
        Mode::MusicPulse,
    ];

    pub fn from_u8(v: u8) -> Option<Mode> {
        Mode::ALL.iter().copied().find(|m| *m as u8 == v)
    }

    pub fn info(self) -> ModeInfo {
        use Mode::*;
        let (name, color, speed, rainbow, dirs, host): (&str, bool, bool, bool, &'static [&'static str], bool) = match self {
            Off => ("Off", false, false, false, &[], false),
            Static => ("Static", true, false, true, &[], false),
            Breathing => ("Breathing", true, true, true, &[], false),
            Spectrum => ("Spectrum Cycle", false, true, false, &[], false),
            Wave => ("Wave", true, true, true, &["right", "left", "down", "up"], false),
            Ripple => ("Ripple", true, true, true, &[], false),
            Raindrop => ("Starlight", true, true, true, &[], false),
            Snake => ("Snake", true, true, true, &["zigzag", "return"], false),
            Reactive => ("Reactive", true, true, true, &[], false),
            Converge => ("Converge", true, true, true, &[], false),
            SineWave => ("Sine Wave", true, true, true, &[], false),
            Kaleidoscope => ("Kaleidoscope", true, true, true, &["out", "in"], false),
            LineWave => ("Line Wave", true, true, true, &["right", "left"], false),
            UserPicture => ("Custom Picture", false, false, false, &["layer 1", "layer 2", "layer 3"], false),
            Laser => ("Laser", true, true, true, &[], false),
            CircleWave => ("Circle Wave", true, true, true, &["anticlockwise", "clockwise"], false),
            Dazzle => ("Dazzle", true, true, true, &[], false),
            RainDown => ("Rain", true, true, true, &[], false),
            Meteor => ("Meteor", true, true, true, &[], false),
            ReactiveOff => ("Reactive Fade", true, true, true, &[], false),
            MusicBars => ("Music Bars", true, false, true, &["upright", "separate", "intersect"], true),
            ScreenSync => ("Screen Sync", false, false, false, &[], true),
            MusicPulse => ("Music Pulse", true, false, true, &["upright", "separate", "intersect"], true),
        };
        ModeInfo { mode: self, name, has_color: color, has_speed: speed, has_rainbow: rainbow, directions: dirs, host_driven: host }
    }
}

/// A complete lighting state as the keyboard stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub mode: Mode,
    /// 0 (slowest) ..= 4 (fastest)
    #[serde(default = "default_speed")]
    pub speed: u8,
    /// 0 (dimmest) ..= 4 (brightest)
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    /// Index into `ModeInfo::directions` (for `UserPicture`: the layer, 0-based).
    #[serde(default)]
    pub direction: u8,
    #[serde(default)]
    pub rainbow: bool,
    #[serde(default = "default_color")]
    pub color: Rgb,
}

fn default_speed() -> u8 {
    2
}
fn default_brightness() -> u8 {
    MAX_BRIGHTNESS
}
fn default_color() -> Rgb {
    Rgb(0, 0x40, 0xFF)
}

impl Effect {
    pub fn new(mode: Mode) -> Self {
        Effect { mode, speed: 2, brightness: MAX_BRIGHTNESS, direction: 0, rainbow: false, color: default_color() }
    }

    pub fn user_picture(layer: u8) -> Self {
        Effect { direction: layer, ..Effect::new(Mode::UserPicture) }
    }

    pub fn validate(&self) -> Result<(), ProtoError> {
        if self.speed > MAX_SPEED {
            return Err(ProtoError::OutOfRange("speed"));
        }
        if self.brightness > MAX_BRIGHTNESS {
            return Err(ProtoError::OutOfRange("brightness"));
        }
        let dirs = self.mode.info().directions.len() as u8;
        if self.direction > 0 && self.direction >= dirs {
            return Err(ProtoError::OutOfRange("direction"));
        }
        Ok(())
    }

    fn flags(&self) -> u8 {
        let dir = self.direction << 4;
        match self.mode {
            Mode::UserPicture => dir,
            Mode::MusicBars | Mode::MusicPulse => dir | if self.rainbow { 0 } else { 4 },
            _ => dir | if self.rainbow { 8 } else { 7 },
        }
    }

    pub fn encode(&self) -> Result<Packet, ProtoError> {
        self.validate()?;
        let mut p = [0u8; REPORT_LEN];
        let (r, g, b) = match self.mode {
            Mode::UserPicture => (0x00, 0xC8, 0xC8), // ignored by firmware; matches vendor app
            _ => (self.color.0, self.color.1, self.color.2),
        };
        let speed = if self.mode == Mode::UserPicture { 0 } else { self.speed };
        p[..8].copy_from_slice(&[cmd::LED, self.mode as u8, MAX_SPEED - speed, self.brightness, self.flags(), r, g, b]);
        ck8(&mut p);
        Ok(p)
    }

    /// Parse the reply to a `0x87` read.
    pub fn decode(reply: &[u8]) -> Result<Self, ProtoError> {
        if reply.len() < 8 || reply[0] != cmd::LED | cmd::READ {
            return Err(ProtoError::BadReply { cmd: cmd::LED | cmd::READ });
        }
        let mode = Mode::from_u8(reply[1]).ok_or(ProtoError::BadReply { cmd: reply[0] })?;
        let flags = reply[4];
        let rainbow = match mode {
            Mode::MusicBars | Mode::MusicPulse => flags & 0x0F == 0,
            Mode::UserPicture => false,
            _ => flags & 0x0F == 8,
        };
        Ok(Effect {
            mode,
            speed: MAX_SPEED.saturating_sub(reply[2]),
            brightness: reply[3].min(MAX_BRIGHTNESS),
            direction: flags >> 4,
            rainbow,
            color: Rgb(reply[5], reply[6], reply[7]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eff(mode: Mode, speed: u8, bri: u8, dir: u8, rainbow: bool, c: u32) -> Effect {
        Effect { mode, speed, brightness: bri, direction: dir, rainbow, color: Rgb::from_u32(c) }
    }

    #[test]
    fn encodes_like_vendor_app() {
        // (effect, first 8 bytes captured from the vendor app)
        let cases = [
            (eff(Mode::Breathing, 1, 4, 0, false, 0x0040FF), [0x07, 0x02, 0x03, 0x04, 0x07, 0x00, 0x40, 0xFF]),
            (eff(Mode::Static, 1, 3, 0, true, 0x123456), [0x07, 0x01, 0x03, 0x03, 0x08, 0x12, 0x34, 0x56]),
            (eff(Mode::Wave, 1, 3, 3, false, 0x123456), [0x07, 0x04, 0x03, 0x03, 0x37, 0x12, 0x34, 0x56]),
            (eff(Mode::CircleWave, 1, 3, 1, false, 0x123456), [0x07, 0x0F, 0x03, 0x03, 0x17, 0x12, 0x34, 0x56]),
            (eff(Mode::MusicBars, 1, 3, 2, false, 0x123456), [0x07, 0x14, 0x03, 0x03, 0x24, 0x12, 0x34, 0x56]),
            (eff(Mode::MusicPulse, 1, 3, 0, true, 0x123456), [0x07, 0x16, 0x03, 0x03, 0x00, 0x12, 0x34, 0x56]),
            (Effect { brightness: 3, ..Effect::user_picture(2) }, [0x07, 0x0D, 0x04, 0x03, 0x20, 0x00, 0xC8, 0xC8]),
        ];
        for (e, want) in cases {
            let p = e.encode().unwrap();
            assert_eq!(&p[..8], &want, "{e:?}");
        }
    }

    #[test]
    fn ck8_of_verified_write() {
        // Breathing cyan written from Python and confirmed applied on the device.
        let p = eff(Mode::Breathing, 1, 4, 0, false, 0x00E5FF).encode().unwrap();
        assert_eq!(p[8], 0xFF - [0x07u8, 0x02, 0x03, 0x04, 0x07, 0x00, 0xE5, 0xFF].iter().fold(0u8, |a, b| a.wrapping_add(*b)));
    }

    #[test]
    fn decodes_device_reply() {
        let reply = [0x87, 0x02, 0x03, 0x04, 0x07, 0x00, 0xE5, 0xFF];
        assert_eq!(Effect::decode(&reply).unwrap(), eff(Mode::Breathing, 1, 4, 0, false, 0x00E5FF));
    }

    #[test]
    fn round_trips_every_mode() {
        for m in Mode::ALL {
            let e = Effect { direction: 0, ..Effect::new(m) };
            let p = e.encode().unwrap();
            let mut reply = p;
            reply[0] |= cmd::READ;
            let d = Effect::decode(&reply).unwrap();
            assert_eq!(d.mode, m);
            assert_eq!(d.brightness, e.brightness);
            if m != Mode::UserPicture {
                assert_eq!(d.speed, e.speed);
                assert_eq!(d.color, e.color);
            }
        }
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(Effect { speed: 5, ..Effect::new(Mode::Wave) }.encode().is_err());
        assert!(Effect { direction: 4, ..Effect::new(Mode::Wave) }.encode().is_err());
        assert!(Effect { direction: 1, ..Effect::new(Mode::Static) }.encode().is_err());
    }
}
