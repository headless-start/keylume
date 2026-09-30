//! Keylume's application core, independent of any UI framework.
//!
//! - [`service::Service`]: a worker thread that owns the keyboard, serialises every
//!   request, coalesces rapid lighting changes, runs live animations and handles
//!   hot-plug. The UI only ever talks to it through messages.
//! - [`store::Store`]: built-in + user profiles, favourites and app settings on disk.
//! - [`board::Board`]: object-safe view of a keyboard (real or simulated).
//! - [`backup`]: snapshot / restore everything stored on the keyboard.
//! - [`lighting`]: what the keyboard shows, as the worker knows it, and live frames.

pub mod backup;
pub mod board;
pub mod color;
pub mod lamparray;
pub mod lighting;
pub mod packs;
pub mod service;
pub mod store;

pub use service::{Service, Status};
pub use store::{AppSettings, Store};
