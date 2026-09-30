use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

use crate::ProtoError;

/// 24-bit colour. Serialises as `"#rrggbb"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const BLACK: Rgb = Rgb(0, 0, 0);

    pub fn from_u32(v: u32) -> Self {
        Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    pub fn to_u32(self) -> u32 {
        (self.0 as u32) << 16 | (self.1 as u32) << 8 | self.2 as u32
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl FromStr for Rgb {
    type Err = ProtoError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h = s.trim().trim_start_matches('#');
        let v = match h.len() {
            6 => u32::from_str_radix(h, 16).ok(),
            3 => u32::from_str_radix(h, 16).ok().map(|v| {
                let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
                ((r * 17) << 16) | ((g * 17) << 8) | (b * 17)
            }),
            _ => None,
        };
        v.map(Rgb::from_u32).ok_or_else(|| ProtoError::BadColor(s.to_string()))
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints() {
        assert_eq!("#0040ff".parse::<Rgb>().unwrap(), Rgb(0, 0x40, 0xFF));
        assert_eq!("0af".parse::<Rgb>().unwrap(), Rgb(0, 0xAA, 0xFF));
        assert_eq!(Rgb(0, 0xE5, 0xFF).to_string(), "#00e5ff");
        assert!("#12345".parse::<Rgb>().is_err());
        assert_eq!(Rgb::from_u32(0x123456).to_u32(), 0x123456);
    }
}
