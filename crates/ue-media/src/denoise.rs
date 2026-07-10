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
/// venv the app provisions itself under its data dir (self-contained), then —
/// as a courtesy on dev machines that have it — the Youtubers-toolkit venv.
pub fn denoiser_python(app_env_dir: Option<&Path>) -> Option<std::path::PathBuf> {
    match std::env::var("UE_DENOISER_PYTHON") {
        Ok(v) if matches!(v.as_str(), "off" | "none" | "0" | "") => return None,
        Ok(v) => return Some(std::path::PathBuf::from(v)),
        Err(_) => {}
    }
    if let Some(dir) = app_env_dir {
        let own = venv_python(dir);
        if own.exists() {
            return Some(own);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let toolkit = std::path::Path::new(&home)
        .join("Videos Reel/Youtubers-toolkit/venv/bin/python");
    toolkit.exists().then_some(toolkit)
}

/// First working system interpreter: tries `python3`, `python` (and `py` on
/// Windows), accepting only Python ≥ 3.9.
fn find_system_python() -> Option<String> {
    let candidates: &[&str] =
        if cfg!(windows) { &["python3", "python", "py"] } else { &["python3", "python"] };
    for cand in candidates {
        let ok = Command::new(cand)
            .args(["-c", "import sys; assert sys.version_info >= (3, 9)"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some((*cand).to_string());
        }
    }
    None
}

fn venv_python(env_dir: &Path) -> std::path::PathBuf {
    if cfg!(windows) {
        env_dir.join("venv/Scripts/python.exe")
    } else {
        env_dir.join("venv/bin/python")
    }
}

/// Provisions the app-owned denoiser venv (one-time, self-contained: only a
/// system `python3` is required): `python3 -m venv` + `pip install denoiser`
/// (which pulls torch/torchaudio). Validated by importing the packages;
/// a broken half-install is removed so the next attempt starts clean.
pub fn ensure_denoiser_env(env_dir: &Path) -> MediaResult<std::path::PathBuf> {
    let python = venv_python(env_dir);
    if python.exists() {
        return Ok(python);
    }
    std::fs::create_dir_all(env_dir)?;
    let venv = env_dir.join("venv");
    eprintln!("[denoise] provisioning the DNS64 environment in {venv:?} (one-time)…");
    let system = find_system_python()
        .ok_or_else(|| MediaError::Tool("python".into(), "no python3/python ≥ 3.9 found".into()))?;
    let ok = Command::new(&system)
        .args(["-m", "venv"])
        .arg(&venv)
        .status()
        .map_err(|e| MediaError::Spawn(system.clone(), e.to_string()))?
        .success();
    if !ok {
        return Err(MediaError::Tool(system, "venv creation failed".into()));
    }
    let ok = Command::new(&python)
        .args(["-m", "pip", "install", "--quiet", "denoiser==0.1.5"])
        .status()
        .map_err(|e| MediaError::Spawn("pip".into(), e.to_string()))?
        .success();
    let importable = ok
        && Command::new(&python)
            .args(["-c", "import denoiser, julius, numpy"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    if !importable {
        let _ = std::fs::remove_dir_all(&venv);
        return Err(MediaError::Tool(
            "pip".into(),
            "denoiser install failed (network? python < 3.9?)".into(),
        ));
    }
    eprintln!("[denoise] DNS64 environment ready");
    Ok(python)
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
/// DNS64 neural denoiser when a Python env is available (provisioning the
/// app-owned venv on demand when `provision` is set), afftdn otherwise.
/// No-op if it already exists.
pub fn denoise_wav(
    conform: &Path,
    app_env_dir: Option<&Path>,
    provision: bool,
) -> MediaResult<std::path::PathBuf> {
    let out = denoised_path(conform);
    if out.exists() {
        return Ok(out);
    }
    let tmp = out.with_extension("part.wav");
    let python = denoiser_python(app_env_dir).or_else(|| {
        if provision && std::env::var("UE_DENOISER_PYTHON").is_err() {
            app_env_dir.and_then(|d| match ensure_denoiser_env(d) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("[denoise] could not provision the env: {e}");
                    None
                }
            })
        } else {
            None
        }
    });
    if let Some(python) = python {
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
