#[cfg(target_os = "macos")]
pub mod fmrs;
pub mod lmstudio;
pub mod openai_compatible;
pub mod stub;

#[cfg(target_os = "macos")]
pub use fmrs::FmRsNarrativeEngine;
pub use lmstudio::LmStudioNarrativeEngine;
pub use openai_compatible::OpenAiCompatibleNarrativeEngine;
pub use stub::StubNarrativeEngine;
