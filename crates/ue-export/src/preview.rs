//! Single-frame preview compositor.
//!
//! The paused preview MUST match the export 1:1 (the golden rule). The export
//! is a linear whole-timeline render, so sampling one frame at time `t`
//! through it would decode everything up to `t` — far too slow for a 20-minute
//! timeline. Instead this composites the SAME instant directly:
//!
//! - every video clip active at `t`, bottom-to-top, its source frame reached
//!   with an input-side `-ss` (fast even for a deep source offset),
//! - each transform/effect chain SAMPLED at `t` (a baked constant, so a single
//!   still renders exactly as the moving export frame would),
//! - the titles and subtitles from the shared `text_overlays_at`.
//!
//! The base clip uses the export's opaque `norm` (fit + pad to canvas); upper
//! layers use its transparent PiP fit and centre overlay — the very strings
//! `build_ffmpeg_args` emits — so the composite is identical by construction.

use std::path::{Path, PathBuf};
use std::process::Command;

use ue_core::model::{ClipPayload, Id, Project, TrackKind};
use ue_core::TimeUs;
use ue_render::EffectDef;

use crate::graph::{resolve_path_pub, text_overlays_at};
use crate::{ExportError, ExportResult};

fn secs(us: TimeUs) -> String {
    format!("{:.6}", us as f64 / 1_000_000.0)
}

/// One video clip visible at the sampled instant.
struct ActiveLayer {
    /// Media input path + the source time to seek to, or a generator source.
    source: LayerSource,
    /// Sampled effect+transform chain (`None` = no chain).
    chain: Option<String>,
}

enum LayerSource {
    Media { path: PathBuf, src_time: TimeUs },
    Gen { source: String },
}

/// Renders the composited frame at timeline time `t_us`, scaled to at most
/// `max_width`, as a JPEG. `None` when no video clip is active at `t_us`.
///
/// Mirrors `build_ffmpeg_args`: base track (bottom) fills the canvas, upper
/// tracks overlay centred, then titles/subtitles are burned on top.
pub fn render_preview_frame(
    project: &Project,
    sequence_id: Id,
    base_dir: &Path,
    t_us: TimeUs,
    max_width: u32,
    extra_packs: &[EffectDef],
) -> ExportResult<Option<Vec<u8>>> {
    let seq = project
        .sequence(sequence_id)
        .ok_or(ExportError::NoSequence(sequence_id))?;
    let (cw, ch) = seq.resolution;
    let fps = format!("{}/{}", seq.fps.0, seq.fps.1);
    let registry =
        ue_render::merge_registries(ue_render::core_registry(), extra_packs.to_vec());

    // active video clips, bottom-to-top (track order). One per track: clips on
    // a track never overlap, so at most one covers t.
    let mut layers: Vec<ActiveLayer> = vec![];
    for track in seq.tracks.iter().filter(|t| t.kind == TrackKind::Video && !t.muted) {
        let Some(clip) = track.clips.iter().find(|c| c.start <= t_us && t_us < c.end()) else {
            continue;
        };
        let rel = t_us - clip.start;
        // base (first layer) is opaque; upper layers composite transparently
        let transparent = !layers.is_empty();
        let chain = ue_render::clip_vf_sampled_ex(
            &registry,
            &clip.effects,
            &clip.transform,
            Some((cw, ch)),
            rel,
            transparent,
        );
        match &clip.payload {
            ClipPayload::Media { asset_id, src_in, .. } => {
                let Some(asset) = project.asset(*asset_id) else {
                    return Err(ExportError::MissingAsset(*asset_id));
                };
                let src_time = *src_in + (rel as f64 * clip.speed).round() as TimeUs;
                layers.push(ActiveLayer {
                    source: LayerSource::Media {
                        path: resolve_path_pub(base_dir, &asset.path),
                        src_time,
                    },
                    chain,
                });
            }
            ClipPayload::Generator { generator_id, params, color_params } => {
                let generators = ue_render::core_generators();
                let Some(def) = ue_render::find_generator(&generators, generator_id) else {
                    continue;
                };
                let source =
                    ue_render::render_generator(def, params, color_params, seq.fps, clip.duration);
                layers.push(ActiveLayer { source: LayerSource::Gen { source }, chain });
            }
            _ => {} // Text/Subtitles/Avatar are not video layers here
        }
    }

    let text = text_overlays_at(project, seq, ch, cw, t_us);
    if layers.is_empty() {
        // nothing to show; a caller may still want the text over black, but the
        // export would be black here too, so match it
        if text.is_none() {
            return Ok(None);
        }
    }

    // ---- assemble the ffmpeg command ----
    let mut args: Vec<String> = vec!["-v".into(), "error".into()];
    // media inputs, in layer order (generators are filter sources, no -i)
    let mut input_idx: Vec<Option<usize>> = vec![];
    let mut next_input = 0usize;
    for layer in &layers {
        match &layer.source {
            LayerSource::Media { path, src_time } => {
                // input-side seek: fast, and frame-close (keyframe) is fine for
                // a paused preview
                args.push("-ss".into());
                args.push(secs(*src_time));
                args.push("-i".into());
                args.push(path.to_string_lossy().into_owned());
                input_idx.push(Some(next_input));
                next_input += 1;
            }
            LayerSource::Gen { .. } => input_idx.push(None),
        }
    }

    // opaque canvas base for the norm (matches the export's base normalisation)
    let norm = format!(
        "fps={fps},scale={cw}:{ch}:force_original_aspect_ratio=decrease,\
         pad={cw}:{ch}:(ow-iw)/2:(oh-ih)/2,setsar=1,format=yuv420p"
    );
    // transparent PiP fit for upper layers (native size, no pad)
    let layer_fit =
        format!("scale='min({cw},iw)':'min({ch},ih)':force_original_aspect_ratio=decrease");

    let mut fc: Vec<String> = vec![];
    let mut current = String::new();
    for (k, layer) in layers.iter().enumerate() {
        let src_label = match (&layer.source, input_idx[k]) {
            (LayerSource::Media { .. }, Some(i)) => format!("[{i}:v]"),
            (LayerSource::Gen { source }, _) => format!("{source},"),
            _ => continue,
        };
        let chain = layer.chain.as_deref().map(|c| format!("{c},")).unwrap_or_default();
        if k == 0 {
            // base: chain (opaque, fits to canvas) then the export's norm
            fc.push(format!("{src_label}{chain}{norm}[c0]"));
            current = "c0".into();
        } else {
            // layer: chain (transparent) + PiP fit + rgba, then centre overlay
            fc.push(format!("{src_label}{chain}{layer_fit},format=rgba[l{k}]"));
            let out = format!("c{k}");
            fc.push(format!(
                "[{current}][l{k}]overlay=x=(W-w)/2:y=(H-h)/2:eof_action=pass[{out}]"
            ));
            current = out;
        }
    }
    // no video layer but we have text → text over black
    if current.is_empty() {
        fc.push(format!("color=c=black:s={cw}x{ch}:d=1,format=yuv420p[c0]"));
        current = "c0".into();
    }
    // titles + subtitles on top (same builder the export burns in)
    if let Some(text) = &text {
        fc.push(format!("[{current}]{text}[txt]"));
        current = "txt".into();
    }
    // final downscale to the preview width
    fc.push(format!("[{current}]scale='min({max_width},iw)':-2[out]"));

    args.push("-filter_complex".into());
    args.push(fc.join(";"));
    args.extend(["-map".into(), "[out]".into()]);
    args.extend([
        "-frames:v".into(), "1".into(),
        "-f".into(), "image2".into(),
        "-c:v".into(), "mjpeg".into(),
        "-q:v".into(), "4".into(),
        "pipe:1".into(),
    ]);

    let out = Command::new(ue_media::ffmpeg_bin())
        .args(&args)
        .output()
        .map_err(|e| ExportError::Spawn(e.to_string()))?;
    if !out.status.success() {
        return Err(ExportError::Ffmpeg(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    if out.stdout.is_empty() {
        return Ok(None);
    }
    Ok(Some(out.stdout))
}
