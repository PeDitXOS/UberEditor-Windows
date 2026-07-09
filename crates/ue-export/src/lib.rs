//! ue-export v0: exportación real del timeline a MP4 vía un único proceso
//! ffmpeg con filter_complex.
//!
//! Alcance v0 (documentado en PLAN §5.6; el grafo wgpu lo sustituirá):
//! - Video: EDL plana "gana el clip de la pista superior" por tramo; huecos → negro.
//!   Texto/efectos/transform aún no se queman (Fase 2).
//! - Audio: todos los clips con audio (pistas de audio y video) con ganancia,
//!   fades y volumen de pista; mezcla con amix.
//! - speed != 1.0 no soportado todavía (error explícito).

pub mod edl;
pub mod graph;

use std::path::Path;
use std::process::Command;

use thiserror::Error;
use ue_core::model::{Id, Project};

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("secuencia {0} no existe")]
    NoSequence(Id),
    #[error("no hay nada que exportar (timeline vacío)")]
    EmptyTimeline,
    #[error("clip con speed {0} ≠ 1.0: aún no soportado en export v0")]
    SpeedUnsupported(f64),
    #[error("asset {0} no existe en el pool")]
    MissingAsset(Id),
    #[error("no se pudo ejecutar ffmpeg: {0}")]
    Spawn(String),
    #[error("ffmpeg falló:\n{0}")]
    Ffmpeg(String),
}

pub type ExportResult<T> = Result<T, ExportError>;

#[derive(Debug, Clone)]
pub struct ExportSettings {
    /// Altura máxima de salida (None = resolución de la secuencia).
    pub max_height: Option<u32>,
    pub crf: u8,
    pub preset: String,
    pub audio_bitrate_k: u32,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self { max_height: None, crf: 18, preset: "veryfast".into(), audio_bitrate_k: 256 }
    }
}

/// Exporta la secuencia activa a `output` (mp4). Bloqueante.
pub fn export_sequence(
    project: &Project,
    sequence_id: Id,
    base_dir: &Path,
    output: &Path,
    settings: &ExportSettings,
) -> ExportResult<()> {
    let plan = graph::build_ffmpeg_args(project, sequence_id, base_dir, output, settings)?;
    let out = Command::new(ue_media::ffmpeg_bin())
        .args(&plan.args)
        .output()
        .map_err(|e| ExportError::Spawn(e.to_string()))?;
    if !out.status.success() {
        return Err(ExportError::Ffmpeg(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(())
}
