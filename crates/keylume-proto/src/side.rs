//! The side light strip (command 0x08, CK8; read 0x88).
//!
//! Same layout as the LED command: `08 mode (4-speed) brightness flags R G B [ck8]`
//! with `flags = direction << 4 | (rainbow ? 8 : 7)`. The strip has its own, smaller
//! set of modes. Verified on a TK68 (fw 772): writes read back and the key lighting
//! is untouched. The vendor app hides this panel for the TK68, but the firmware has it.

use serde::{Deserialize, Serialize};

use crate::led::{MAX_BRIGHTNESS, MAX_SPEED};
use crate::packet::{ck8, cmd, Packet, REPORT_LEN};
use crate::{ProtoError, Rgb};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum SideMode {
    Off = 0,
    Static = 1,
    Breathing = 2,
    /// Rainbow colour cycle.
    Neon = 3,
    Wave = 4,
    Snake = 5,
}

impl SideMode {
    pub const ALL: [SideMode; 6] = [SideMode::Off, SideMode::Static, SideMode::Breathing, SideMode::Neon, SideMode::Wave, SideMode::Snake];

    pub fn from_u8(v: u8) -> Option<SideMode> {
        SideMode::ALL.iter().copied().find(|m| *m as u8 == v)
    }

    /// Valid directions (index = the `direction` value).
    pub fn directions(self) -> &'static [&'static str] {
        match self {
            SideMode::Wave => &["right", "left", "down", "up"],
            SideMode::Snake => &["zigzag", "return"],
            _ => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideLight {
    pub mode: SideMode,
    /// 0 (slowest) ..= 4 (fastest)
    #[serde(default = "two")]
    pub speed: u8,
    /// 0 ..= 4
    #[serde(default = "four")]
    pub brightness: u8,
    #[serde(default)]
    pub direction: u8,
    #[serde(default)]
    pub rainbow: bool,
    #[serde(default = "blue")]
    pub color: Rgb,
}

fn two() -> u8 {
    2
}
fn four() -> u8 {
    MAX_BRIGHTNESS
}
fn blue() -> Rgb {
    Rgb(0x1F, 0x45, 0xFF)
}

impl SideLight {
    pub fn new(mode: SideMode, color: Rgb) -> Self {
        SideLight { mode, speed: 2, brightness: MAX_BRIGHTNESS, direction: 0, rainbow: false, color }
    }

    pub fn off() -> Self {
        SideLight { brightness: 0, ..SideLight::new(SideMode::Off, Rgb::BLACK) }
    }

    pub fn validate(&self) -> Result<(), ProtoError> {
        if self.speed > MAX_SPEED {
            return Err(ProtoError::OutOfRange("speed"));
        }
        if self.brightness > MAX_BRIGHTNESS {
            return Err(ProtoError::OutOfRange("brightness"));
        }
        let dirs = self.mode.directions().len() as u8;
        if self.direction > 0 && self.direction >= dirs {
            return Err(ProtoError::OutOfRange("direction"));
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Packet, ProtoError> {
        self.validate()?;
        let mut p = [0u8; REPORT_LEN];
        let flags = self.direction << 4 | if self.rainbow { 8 } else { 7 };
        let c = self.color;
        p[..8].copy_from_slice(&[cmd::SIDE_LED, self.mode as u8, MAX_SPEED - self.speed, self.brightness, flags, c.0, c.1, c.2]);
        ck8(&mut p);
        Ok(p)
    }

    /// Parse the reply to a `0x88` read.
    pub fn decode(reply: &[u8]) -> Result<Self, ProtoError> {
        let bad = ProtoError::BadReply { cmd: cmd::SIDE_LED | cmd::READ };
        if reply.len() < 8 || reply[0] != cmd::SIDE_LED | cmd::READ {
            return Err(bad);
        }
        let mode = SideMode::from_u8(reply[1]).ok_or(bad)?;
        let flags = reply[4];
        Ok(SideLight {
            mode,
            speed: MAX_SPEED.saturating_sub(reply[2]),
            brightness: reply[3].min(MAX_BRIGHTNESS),
            direction: (flags >> 4).min(mode.directions().len().saturating_sub(1) as u8),
            rainbow: flags & 0x0F == 8,
            color: Rgb(reply[5], reply[6], reply[7]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_the_hardware_probe() {
        // `send8:08,02,02,04,07,1f,45,ff` read back verbatim from a TK68.
        let s = SideLight { mode: SideMode::Breathing, speed: 2, brightness: 4, direction: 0, rainbow: false, color: Rgb(0x1F, 0x45, 0xFF) };
        assert_eq!(&s.encode().unwrap()[..8], &[0x08, 0x02, 0x02, 0x04, 0x07, 0x1F, 0x45, 0xFF]);
    }

    #[test]
    fn decodes_the_factory_state() {
        // What the TK68 reported before we touched it: rainbow neon.
        let s = SideLight::decode(&[0x88, 0x03, 0x00, 0x04, 0x01, 0xFF, 0xFF, 0xFF]).unwrap();
        assert_eq!(s.mode, SideMode::Neon);
        assert_eq!(s.speed, 4);
        assert_eq!(s.brightness, 4);
    }

    #[test]
    fn round_trips() {
        for m in SideMode::ALL {
            for dir in 0..m.directions().len().max(1) as u8 {
                let s = SideLight { direction: dir, rainbow: dir % 2 == 1, ..SideLight::new(m, Rgb(1, 2, 3)) };
                let mut r = s.encode().unwrap();
                r[0] |= cmd::READ;
                assert_eq!(SideLight::decode(&r).unwrap(), s);
            }
        }
    }

    #[test]
    fn rejects_bad_values() {
        assert!(SideLight { speed: 5, ..SideLight::new(SideMode::Wave, Rgb::BLACK) }.encode().is_err());
        assert!(SideLight { direction: 1, ..SideLight::new(SideMode::Static, Rgb::BLACK) }.encode().is_err());
        assert!(SideLight::decode(&[0x88, 0x09, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
