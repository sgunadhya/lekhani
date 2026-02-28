#[cfg(target_os = "macos")]
pub mod fmrs;
pub mod stub;

#[cfg(target_os = "macos")]
pub use fmrs::FmRsNarrativeEngine;
pub use stub::StubNarrativeEngine;
