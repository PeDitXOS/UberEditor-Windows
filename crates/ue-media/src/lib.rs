//! ue-media: sondeo (ffprobe), hashing e importación de archivos, y extracción
//! de frames reales (ffmpeg) para el preview. Los binarios de FFmpeg se toman
//! del PATH en desarrollo (`UE_FFMPEG`/`UE_FFPROBE` los sobreescriben); en
//! producción serán sidecars empaquetados.

pub mod frame;
pub mod hashing;
pub mod probe;

use std::path::Path;

use thiserror::Error;
use ue_core::model::{MediaAsset, MediaKind};
use ulid::Ulid;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("no se pudo ejecutar {0}: {1}")]
    Spawn(String, String),
    #[error("{0} falló: {1}")]
    Tool(String, String),
    #[error("no se pudo interpretar la salida de ffprobe: {0}")]
    Parse(String),
    #[error("archivo no soportado o sin streams: {0}")]
    Unsupported(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type MediaResult<T> = Result<T, MediaError>;

pub fn ffmpeg_bin() -> String {
    std::env::var("UE_FFMPEG").unwrap_or_else(|_| "ffmpeg".into())
}

pub fn ffprobe_bin() -> String {
    std::env::var("UE_FFPROBE").unwrap_or_else(|_| "ffprobe".into())
}

/// Importa un archivo: probe + hash → MediaAsset listo para el pool.
pub fn import_file(path: &Path) -> MediaResult<MediaAsset> {
    let (kind, probe_info) = probe::probe(path)?;
    let content_hash = hashing::content_hash(path)?;
    Ok(MediaAsset {
        id: Ulid::new(),
        kind,
        path: path.to_string_lossy().into_owned(),
        content_hash,
        probe: probe_info,
        proxy: None,
        audio_conform: None,
        peaks: None,
        thumbnails: None,
        transcript: None,
        offline: false,
    })
}

/// Duración por defecto de un clip de imagen fija.
pub const IMAGE_CLIP_DURATION_US: i64 = 5_000_000;

pub fn default_clip_duration(asset: &MediaAsset) -> i64 {
    match asset.kind {
        MediaKind::Image => IMAGE_CLIP_DURATION_US,
        _ => asset.probe.duration_us,
    }
}
