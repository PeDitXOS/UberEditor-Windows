//! ue-ai: content analysis and generation.
//! - silence detection (PLAN §7.C), an improved port of the toolkit's
//!   `trim_by_silence`: fine RMS windows, hysteresis, minimums and padding.
//! - per-segment emotion analysis for the avatar (PLAN §7.E).
//! - text-to-speech voiceover (macOS `say` + the toolkit's Kokoro).

pub mod emotion;
pub mod silence;
pub mod tts;
