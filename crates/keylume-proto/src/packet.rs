//! 64-byte packet framing and the two checksum flavours.

pub const REPORT_LEN: usize = 64;
/// Bytes of payload carried by one page of a bulk (paged) transfer.
pub const PAGE_DATA: usize = 56;

pub type Packet = [u8; REPORT_LEN];

/// Command bytes. Reads are the same value with bit 7 set.
pub mod cmd {
    pub const VERSION: u8 = 0x00;
    pub const RESET: u8 = 0x02;
    pub const REPORT_RATE: u8 = 0x04;
    pub const PROFILE: u8 = 0x05;
    pub const KB_OPTION: u8 = 0x06;
    pub const LED: u8 = 0x07;
    /// Side light strip ("SLED" in the vendor app).
    pub const SIDE_LED: u8 = 0x08;
    pub const KEYMAP: u8 = 0x09;
    pub const MACRO: u8 = 0x0B;
    pub const USER_PICTURE: u8 = 0x0C;
    pub const FN_KEYMAP: u8 = 0x10;
    pub const DEBOUNCE: u8 = 0x11;
    pub const SLEEP: u8 = 0x12;
    pub const READ: u8 = 0x80;
}

/// `b[7] = 0xFF - sum(b[0..7])`. Used by reads, small setters and key-map page headers.
pub fn ck7(p: &mut Packet) {
    p[7] = 0xFF - sum(&p[..7]);
}

/// Does `p` carry a valid CK7? (Used by the simulator.)
pub fn ck7_ok(p: &Packet) -> bool {
    p[7] == 0xFF - sum(&p[..7])
}

/// Does `p` carry a valid CK8?
pub fn ck8_ok(p: &Packet) -> bool {
    p[8] == 0xFF - sum(&p[..8])
}

/// `b[8] = 0xFF - sum(b[0..8])`. Used by the LED command, whose bytes 5..8 are RGB.
pub fn ck8(p: &mut Packet) {
    p[8] = 0xFF - sum(&p[..8]);
}

fn sum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b))
}

/// Build a packet from a prefix, zero-padded, with CK7 applied.
pub fn with_ck7(prefix: &[u8]) -> Packet {
    let mut p = [0u8; REPORT_LEN];
    p[..prefix.len()].copy_from_slice(prefix);
    ck7(&mut p);
    p
}

/// A read request: `[cmd | 0x80, a, b]` with CK7.
pub fn read_request(cmd: u8, a: u8, b: u8) -> Packet {
    with_ck7(&[cmd | cmd::READ, a, b])
}

/// Split `data` into paged writes: header `[cmd, arg, len_lo, len_hi, page, 0, 0, ck7]`
/// followed by up to 56 bytes. `len` is the total transfer size the device expects.
pub fn paged_write(cmd: u8, arg: u8, len: u16, pages: usize, data: &[u8]) -> Vec<Packet> {
    (0..pages)
        .map(|page| {
            let [lo, hi] = len.to_le_bytes();
            let mut p = [0u8; REPORT_LEN];
            p[..5].copy_from_slice(&[cmd, arg, lo, hi, page as u8]);
            let start = page * PAGE_DATA;
            if start < data.len() {
                let end = (start + PAGE_DATA).min(data.len());
                p[8..8 + (end - start)].copy_from_slice(&data[start..end]);
            }
            ck7(&mut p); // header only
            p
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ck7_matches_device_replies() {
        // Replies captured from a TK68: the device echoes our CK7.
        assert_eq!(read_request(cmd::REPORT_RATE, 0, 0)[7], 0x7B);
        assert_eq!(read_request(cmd::VERSION, 0, 0)[7], 0x7F);
        assert_eq!(read_request(cmd::PROFILE, 0, 0)[7], 0x7A);
    }

    #[test]
    fn paged_header_matches_capture() {
        // Captured key-map write header: 09 00 f8 01 00 00 00 fd
        let pages = paged_write(cmd::KEYMAP, 0, 0x01F8, 9, &[0u8; 512]);
        assert_eq!(pages.len(), 9);
        assert_eq!(&pages[0][..8], &[0x09, 0x00, 0xF8, 0x01, 0x00, 0x00, 0x00, 0xFD]);
        assert_eq!(&pages[8][..8], &[0x09, 0x00, 0xF8, 0x01, 0x08, 0x00, 0x00, 0xF5]);
    }
}
