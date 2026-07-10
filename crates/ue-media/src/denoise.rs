//! Background-noise reduction of a conformed WAV.
//!
//! Primary engine: the SAME neural denoiser as the Youtubers-toolkit —
//! Facebook's pretrained DNS64 via the `denoiser` Python package — run as a
//! sidecar (scripts/denoise_dns64.py, embedded here). Verified on real voice:
//! the speech level is untouched while the noise floor drops dramatically.
//!
//! Fallback engine when no Python environment is available: ffmpeg's afftdn
//! (dependency-free spectral denoiser). `UE_DENOISER_PYTHON` overrides the
//! interpreter ("off" disables the neural path).

use std::path::Path;
use std::process::Command;

use crate::{ffmpeg_bin, MediaError, MediaResult};

/// The exact filter used in BOTH live (pre-rendered conform variant) and
/// export (inline in the audio chain) so they sound identical.
// nf must start NEAR the real noise floor (tn refines it); a too-low nf
// makes the filter treat the noise as signal. Measured on a speech+white
// noise fixture: -25 dB of noise for ~1 dB of voice.
pub const DENOISE_FILTER: &str = "afftdn=nr=30:nf=-25:tn=1";

/// Path of the denoised sibling of a conform WAV (`x.wav` → `x.denoise.wav`).
pub fn denoised_path(conform: &Path) -> std::path::PathBuf {
    conform.with_extension("denoise.wav")
}

/// The DNS64 sidecar script, embedded so packaged builds carry it.
const DNS64_SCRIPT: &str = include_str!("../../../scripts/denoise_dns64.py");

/// Python interpreter for the neural denoiser, if any.
/// Priority: `UE_DENOISER_PYTHON` (value "off"/"none"/"0" disables), then the
/// Youtubers-toolkit venv in its standard location.
pub fn denoiser_python() -> Option<std::path::PathBuf> {
    match std::env::var("UE_DENOISER_PYTHON") {
        Ok(v) if matches!(v.as_str(), "off" | "none" | "0" | "") => return None,
        Ok(v) => return Some(std::path::PathBuf::from(v)),
        Err(_) => {}
    }
    let home = std::env::var("HOME").ok()?;
    let toolkit = std::path::Path::new(&home)
        .join("Videos Reel/Youtubers-toolkit/venv/bin/python");
    toolkit.exists().then_some(toolkit)
}

/// Runs the DNS64 sidecar. Any failure is reported as Err (caller falls back).
fn denoise_dns64(python: &Path, src: &Path, out: &Path) -> MediaResult<()> {
    let script = std::env::temp_dir().join("ue_denoise_dns64.py");
    std::fs::write(&script, DNS64_SCRIPT)?;
    let output = Command::new(python)
        .arg(&script)
        .arg(src)
        .arg(out)
        .output()
        .map_err(|e| MediaError::Spawn("python (denoiser)".into(), e.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !stdout.contains("ok ") || !out.exists() {
        return Err(MediaError::Tool(
            "dns64".into(),
            format!(
                "denoiser sidecar failed: {}{}",
                stdout,
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    Ok(())
}

/// Renders the denoised variant (48 kHz stereo s16le, like the conform):
/// DNS64 neural denoiser when a Python env is available, afftdn otherwise.
/// No-op if it already exists.
pub fn denoise_wav(conform: &Path) -> MediaResult<std::path::PathBuf> {
    let out = denoised_path(conform);
    if out.exists() {
        return Ok(out);
    }
    let tmp = out.with_extension("part.wav");
    if let Some(python) = denoiser_python() {
        match denoise_dns64(&python, conform, &tmp) {
            Ok(()) => {
                std::fs::rename(&tmp, &out)?;
                eprintln!("[denoise] DNS64 (neural) → {out:?}");
                return Ok(out);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                eprintln!("[denoise] DNS64 unavailable ({e}); falling back to afftdn");
            }
        }
    }
    let status = Command::new(ffmpeg_bin())
        .args(["-y", "-v", "error", "-i"])
        .arg(conform)
        .args(["-af", DENOISE_FILTER, "-ar", "48000", "-ac", "2", "-c:a", "pcm_s16le"])
        .arg(&tmp)
        .status()
        .map_err(|e| MediaError::Spawn("ffmpeg".into(), e.to_string()))?;
    if !status.success() || !tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
        return Err(MediaError::Tool("ffmpeg".into(), "denoise failed".into()));
    }
    std::fs::rename(&tmp, &out)?;
    Ok(out)
}
