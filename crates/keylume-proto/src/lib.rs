//! Wire protocol for Rongyuan-based keyboards (verified on the Epomaker TK68).
//!
//! Every command is a 64-byte HID feature report (report ID 0). This crate only
//! builds and parses those buffers; talking to the device is `keylume-device`'s job.
//! See `docs/PROTOCOL.md` for the byte-level description.

pub mod color;
pub mod keymap;
pub mod lamparray;
pub mod layout;
pub mod led;
pub mod macros;
pub mod packet;
pub mod picture;
pub mod settings;
pub mod side;
pub mod standard;
pub mod stream;

pub use color::Rgb;
pub use keymap::{KeyAction, Keymap};
pub use layout::{Finish, Layout};
pub use led::{Effect, Mode};
pub use macros::{Macro, MacroEvent};
pub use packet::{Packet, REPORT_LEN};
pub use picture::Frame;
pub use side::{SideLight, SideMode};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtoError {
    #[error("value out of range: {0}")]
    OutOfRange(&'static str),
    #[error("macro too long ({0} bytes, max {1})")]
    MacroTooLong(usize, usize),
    #[error("unexpected reply for command {cmd:#04x}")]
    BadReply { cmd: u8 },
    #[error("invalid colour {0:?}")]
    BadColor(String),
}
