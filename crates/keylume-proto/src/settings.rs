//! Small device settings: report rate, debounce, sleep timers, onboard profile, reset.

use serde::{Deserialize, Serialize};

use crate::packet::{cmd, read_request, with_ck7, Packet, REPORT_LEN};
use crate::ProtoError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportRate {
    Hz125,
    Hz250,
    Hz500,
    Hz1000,
}

impl ReportRate {
    pub fn hz(self) -> u16 {
        match self {
            Self::Hz125 => 125,
            Self::Hz250 => 250,
            Self::Hz500 => 500,
            Self::Hz1000 => 1000,
        }
    }
    pub fn from_hz(hz: u16) -> Option<Self> {
        match hz {
            125 => Some(Self::Hz125),
            250 => Some(Self::Hz250),
            500 => Some(Self::Hz500),
            1000 => Some(Self::Hz1000),
            _ => None,
        }
    }
    fn code(self) -> u8 {
        match self {
            Self::Hz1000 => 1,
            Self::Hz500 => 2,
            Self::Hz250 => 4,
            Self::Hz125 => 8,
        }
    }
    fn from_code(c: u8) -> Option<Self> {
        match c {
            1 => Some(Self::Hz1000),
            2 => Some(Self::Hz500),
            4 => Some(Self::Hz250),
            8 => Some(Self::Hz125),
            _ => None,
        }
    }
}

pub fn set_report_rate(profile: u8, rate: ReportRate) -> Packet {
    with_ck7(&[cmd::REPORT_RATE, profile, rate.code()])
}
pub fn get_report_rate() -> Packet {
    read_request(cmd::REPORT_RATE, 0, 0)
}
pub fn parse_report_rate(reply: &[u8]) -> Result<ReportRate, ProtoError> {
    check(reply, cmd::REPORT_RATE)?;
    ReportRate::from_code(reply[2]).ok_or(ProtoError::BadReply { cmd: reply[0] })
}

/// Highest debounce value Keylume sends (the editor offers 0-10; the firmware's own
/// limit isn't known, so values from files are held to this).
pub const MAX_DEBOUNCE: u8 = 50;

pub fn set_debounce(value: u8) -> Packet {
    with_ck7(&[cmd::DEBOUNCE, 0, value])
}
pub fn get_debounce() -> Packet {
    read_request(cmd::DEBOUNCE, 0, 0)
}
pub fn parse_debounce(reply: &[u8]) -> Result<u8, ProtoError> {
    check(reply, cmd::DEBOUNCE)?;
    Ok(reply[2])
}

pub fn set_profile(profile: u8) -> Packet {
    with_ck7(&[cmd::PROFILE, profile])
}
pub fn get_profile() -> Packet {
    read_request(cmd::PROFILE, 0, 0)
}
pub fn parse_profile(reply: &[u8]) -> Result<u8, ProtoError> {
    check(reply, cmd::PROFILE)?;
    Ok(reply[1].max(reply[2]))
}

pub fn get_version() -> Packet {
    read_request(cmd::VERSION, 0, 0)
}
pub fn parse_version(reply: &[u8]) -> Result<u16, ProtoError> {
    check(reply, cmd::VERSION)?;
    Ok(u16::from_le_bytes([reply[1], reply[2]]))
}

/// Idle timers in seconds (0 = never).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepTimers {
    pub bluetooth: u16,
    pub wireless: u16,
    pub deep_bluetooth: u16,
    pub deep_wireless: u16,
}

pub fn set_sleep(t: SleepTimers) -> Packet {
    let mut p = [0u8; REPORT_LEN];
    p[0] = cmd::SLEEP;
    for (i, v) in [t.bluetooth, t.wireless, t.deep_bluetooth, t.deep_wireless].into_iter().enumerate() {
        p[8 + i * 2..10 + i * 2].copy_from_slice(&v.to_le_bytes());
    }
    crate::packet::ck7(&mut p); // header-only, like every BIT7 packet
    p
}
pub fn get_sleep() -> Packet {
    read_request(cmd::SLEEP, 0, 0)
}
pub fn parse_sleep(reply: &[u8]) -> Result<SleepTimers, ProtoError> {
    check(reply, cmd::SLEEP)?;
    let v = |i: usize| u16::from_le_bytes([reply[1 + i * 2], reply[2 + i * 2]]);
    Ok(SleepTimers { bluetooth: v(0), wireless: v(1), deep_bluetooth: v(2), deep_wireless: v(3) })
}

/// Keyboard behaviour switches (command 0x06 / 0x86).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardOptions {
    /// Disable the Windows key (gaming mode).
    pub win_key_lock: bool,
    /// macOS layout (Cmd/Opt) instead of Windows.
    pub mac_mode: bool,
    /// Swap WASD with the arrow keys.
    pub wasd_arrows_swap: bool,
    /// Turn the main backlight off.
    pub led_off: bool,
    /// Turn the side/underglow LEDs off (boards that have them).
    pub side_led_off: bool,
    pub keyboard_mode: bool,
    /// Lock the whole keyboard.
    pub keyboard_lock: bool,
    pub fn_matrix: bool,
    pub power_save: bool,
}

pub fn set_options(profile: u8, o: KeyboardOptions) -> Packet {
    let b2 = o.win_key_lock as u8
        | (o.mac_mode as u8) << 1
        | (o.wasd_arrows_swap as u8) << 3
        | (o.led_off as u8) << 4
        | (o.side_led_off as u8) << 5
        | (o.keyboard_mode as u8) << 6
        | (o.keyboard_lock as u8) << 7;
    with_ck7(&[cmd::KB_OPTION, profile, b2, o.fn_matrix as u8, o.power_save as u8])
}
pub fn get_options() -> Packet {
    read_request(cmd::KB_OPTION, 0, 0)
}
pub fn parse_options(reply: &[u8]) -> Result<KeyboardOptions, ProtoError> {
    check(reply, cmd::KB_OPTION)?;
    let (b, fnm, ps) = (reply[2], reply[3], reply[4]);
    Ok(KeyboardOptions {
        win_key_lock: b & 1 != 0,
        mac_mode: b & 2 != 0,
        wasd_arrows_swap: b & 8 != 0,
        led_off: b & 16 != 0,
        side_led_off: b & 32 != 0,
        keyboard_mode: b & 64 != 0,
        keyboard_lock: b & 128 != 0,
        fn_matrix: fnm & 1 != 0,
        power_save: ps != 0,
    })
}

/// Restores factory keymap, lighting and settings.
pub fn factory_reset() -> Packet {
    with_ck7(&[cmd::RESET])
}

fn check(reply: &[u8], c: u8) -> Result<(), ProtoError> {
    if reply.len() < 10 || reply.len() > REPORT_LEN + 1 || reply[0] != c | cmd::READ {
        return Err(ProtoError::BadReply { cmd: c | cmd::READ });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(b: &[u8]) -> Vec<u8> {
        let mut v = b.to_vec();
        v.resize(64, 0);
        v
    }

    #[test]
    fn matches_captures() {
        assert_eq!(&set_report_rate(0, ReportRate::Hz1000)[..3], &[0x04, 0x00, 0x01]);
        assert_eq!(&set_debounce(2)[..3], &[0x11, 0x00, 0x02]);
        let s = set_sleep(SleepTimers { bluetooth: 3600, wireless: 3600, deep_bluetooth: 3600, deep_wireless: 3600 });
        assert_eq!(&s[8..16], &[0x10, 0x0E, 0x10, 0x0E, 0x10, 0x0E, 0x10, 0x0E]);
    }

    #[test]
    fn parses_replies() {
        assert_eq!(parse_report_rate(&pad(&[0x84, 0x00, 0x01])).unwrap(), ReportRate::Hz1000);
        assert_eq!(parse_debounce(&pad(&[0x91, 0x00, 0x02])).unwrap(), 2);
        assert_eq!(parse_version(&pad(&[0x80, 0x04, 0x03])).unwrap(), 772);
        assert_eq!(parse_sleep(&pad(&[0x92, 0x10, 0x0E, 0x10, 0x0E, 0x10, 0x0E, 0x10, 0x0E])).unwrap().bluetooth, 3600);
        assert!(parse_debounce(&pad(&[0x84])).is_err());
    }

    #[test]
    fn options_round_trip() {
        let o = KeyboardOptions { win_key_lock: true, wasd_arrows_swap: true, fn_matrix: true, ..Default::default() };
        let p = set_options(0, o);
        assert_eq!(&p[..5], &[0x06, 0x00, 0b0000_1001, 1, 0]);
        let mut reply = p;
        reply[0] = 0x86;
        assert_eq!(parse_options(&reply).unwrap(), o);
        // captured default reply: 86 00 00 80 -> everything off
        assert_eq!(parse_options(&pad(&[0x86, 0x00, 0x00, 0x80])).unwrap(), KeyboardOptions::default());
    }
}
