//! Image backends selected at compile time. macOS keeps the system encoder;
//! Linux and Windows use libheif. The portable feature enables interoperability
//! testing of the libheif backend on macOS as well.
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
pub mod macos;
#[cfg(any(not(target_os = "macos"), feature = "portable"))]
#[path = "portable.rs"]
pub mod portable;

#[cfg(all(target_os = "macos", not(feature = "portable")))]
pub use macos::*;
#[cfg(any(not(target_os = "macos"), feature = "portable"))]
pub use portable::*;
