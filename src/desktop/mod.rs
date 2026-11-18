//! Desktop shell (Windows) and shared key-material helpers.

pub mod key_material;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::*;
