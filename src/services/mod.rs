//! Blocking external services. Run network and file operations on worker threads,
//! never on GPUI's foreground event loop.

pub mod export;
pub mod nasa;
pub mod voice;
pub mod voice_pipeline;
