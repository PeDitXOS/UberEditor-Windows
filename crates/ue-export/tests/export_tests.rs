//! Tests de ue-export: EDL (unitarios) y exportación real con ffmpeg
//! (integración, verificable visualmente con el contador de testsrc).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use ue_core::model::*;
use ue_core::ops::InsertMode;
use ue_core::ProjectStore;
use ue_export::edl::{build_video_edl, edl_duration, Segment};
use ue_export::{export_sequence, ExportError, ExportSettings};

const SEC: i64 = 1_000_000;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn fake_asset(kind: MediaKind, path: &str, dur_s: i64, with_audio: bool) -> MediaAsset {
    MediaAsset {
        id: Id::new(),
        kind,
        path: path.into(),
        content_hash: format!("xxh3:{path}"),
        probe: ProbeInfo {
            duration_us: dur_s * SEC,
            fps: if kind == MediaKind::Video { Some((30, 1)) } else { None },
            width: if kind == MediaKind::Video { 640 } else { 0 },
            height: if kind == MediaKind::Video { 360 } else { 0 },
            rotation: 0,
            vcodec: None,
            acodec: None,
            audio_channels: if with_audio { 2 } else { 0 },
            vfr: false,
        },
        proxy: None,
        audio_conform: None,
        peaks: None,
        thumbnails: None,
        transcript: None,
        offline: false,
    }
}

/// (store, seq, v1, v2, a1, asset_a, asset_b)
fn project_two_video_tracks() -> (ProjectStore, Id, Id, Id, Id, Id, Id) {
    let mut p = Project::new("t");
    let seq_id = p.active_sequence;
    let a = fake_asset(MediaKind::Video, "a.mp4", 10, false);
    let b = fake_asset(MediaKind::Video, "b.mp4", 10, false);
    let (aid, bid) = (a.id, b.id);
    p.assets.push(a);
    p.assets.push(b);
    // añadir V2 sobre V1
    let seq = p.sequence_mut(seq_id).unwrap();
    seq.tracks.push(Track::new(TrackKind::Video, "V2"));
    let v2 = seq.tracks.last().unwrap().id;
    let v1 = seq.tracks.iter().find(|t| t.kind == TrackKind::Video && t.name == "V1").unwrap().id;
    let a1 = seq.tracks.iter().find(|t| t.kind == TrackKind::Audio).unwrap().id;
    (ProjectStore::new(p), seq_id, v1, v2, a1, aid, bid)
}

// ---------------------------------------------------------------------------
// EDL
// ---------------------------------------------------------------------------

#[test]
fn edl_with_gap_and_two_sources() {
    let (mut store, seq, v1, _v2, _a1, a, b) = project_two_video_tracks();
    store.insert_clip(v1, Clip::new_media(a, 1 * SEC, 3 * SEC, 0), InsertMode::Strict).unwrap();
    store.insert_clip(v1, Clip::new_media(b, 4 * SEC, 6 * SEC, 2 * SEC), InsertMode::Strict).unwrap();
    // hueco [4, 5) y luego otra vez A
    store.insert_clip(v1, Clip::new_media(a, 8 * SEC, 9 * SEC, 5 * SEC), InsertMode::Strict).unwrap();

    let edl = build_video_edl(&store.project, seq).unwrap();
    assert_eq!(
        edl,
        vec![
            Segment::Source { asset_id: a, src_in: 1 * SEC, src_out: 3 * SEC },
            Segment::Source { asset_id: b, src_in: 4 * SEC, src_out: 6 * SEC },
            Segment::Black { duration: 1 * SEC },
            Segment::Source { asset_id: a, src_in: 8 * SEC, src_out: 9 * SEC },
        ]
    );
    assert_eq!(edl_duration(&edl), 6 * SEC);
}

#[test]
fn edl_top_track_wins() {
    let (mut store, seq, v1, v2, _a1, a, b) = project_two_video_tracks();
    // V1: A cubre [0, 6); V2: B cubre [2, 4) → B debe ganar en el medio
    store.insert_clip(v1, Clip::new_media(a, 0, 6 * SEC, 0), InsertMode::Strict).unwrap();
    store.insert_clip(v2, Clip::new_media(b, 0, 2 * SEC, 2 * SEC), InsertMode::Strict).unwrap();

    let edl = build_video_edl(&store.project, seq).unwrap();
    assert_eq!(
        edl,
        vec![
            Segment::Source { asset_id: a, src_in: 0, src_out: 2 * SEC },
            Segment::Source { asset_id: b, src_in: 0, src_out: 2 * SEC },
            Segment::Source { asset_id: a, src_in: 4 * SEC, src_out: 6 * SEC },
        ]
    );
}

#[test]
fn edl_merges_contiguous_and_trims_trailing_black() {
    let (mut store, seq, v1, _v2, _a1, a, _b) = project_two_video_tracks();
    // dos clips contiguos con fuente continua → se fusionan
    store.insert_clip(v1, Clip::new_media(a, 1 * SEC, 3 * SEC, 0), InsertMode::Strict).unwrap();
    store.insert_clip(v1, Clip::new_media(a, 3 * SEC, 5 * SEC, 2 * SEC), InsertMode::Strict).unwrap();
    let edl = build_video_edl(&store.project, seq).unwrap();
    assert_eq!(
        edl,
        vec![Segment::Source { asset_id: a, src_in: 1 * SEC, src_out: 5 * SEC }]
    );
}

#[test]
fn edl_rejects_speed_and_empty() {
    let (mut store, seq, v1, _v2, _a1, a, _b) = project_two_video_tracks();
    assert!(matches!(
        build_video_edl(&store.project, seq),
        Err(ExportError::EmptyTimeline)
    ));
    let mut clip = Clip::new_media(a, 0, 2 * SEC, 0);
    clip.speed = 2.0;
    clip.duration = 1 * SEC;
    store.insert_clip(v1, clip, InsertMode::Strict).unwrap();
    assert!(matches!(
        build_video_edl(&store.project, seq),
        Err(ExportError::SpeedUnsupported(_))
    ));
}

#[test]
fn muted_track_is_invisible() {
    let (mut store, seq, v1, v2, _a1, a, b) = project_two_video_tracks();
    store.insert_clip(v1, Clip::new_media(a, 0, 2 * SEC, 0), InsertMode::Strict).unwrap();
    store.insert_clip(v2, Clip::new_media(b, 0, 2 * SEC, 0), InsertMode::Strict).unwrap();
    // sin mute gana V2 (B)
    let edl = build_video_edl(&store.project, seq).unwrap();
    assert!(matches!(edl[0], Segment::Source { asset_id, .. } if asset_id == b));
    // con V2 muteada gana V1 (A)
    store
        .dispatch(
            "mute",
            vec![ue_core::Action::SetTrackProp {
                track_id: v2,
                prop: ue_core::action::TrackProp::Muted(true),
            }],
        )
        .unwrap();
    let edl = build_video_edl(&store.project, seq).unwrap();
    assert!(matches!(edl[0], Segment::Source { asset_id, .. } if asset_id == a));
}

// ---------------------------------------------------------------------------
// Exportación real (ffmpeg)
// ---------------------------------------------------------------------------

fn ffmpeg_available() -> bool {
    Command::new(ue_media::ffmpeg_bin())
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn media_dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        if !ffmpeg_available() {
            eprintln!("AVISO: ffmpeg no disponible; test de export saltado");
            return None;
        }
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ue-export-media");
        std::fs::create_dir_all(&dir).unwrap();
        let st = Command::new(ue_media::ffmpeg_bin())
            .args([
                "-y", "-v", "error",
                "-f", "lavfi", "-i", "testsrc=duration=8:size=640x360:rate=30",
                "-f", "lavfi", "-i", "sine=frequency=440:duration=8",
                "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p",
                "-c:a", "aac", "-shortest",
            ])
            .arg(dir.join("counter.mp4"))
            .status()
            .unwrap();
        assert!(st.success());
        Some(dir)
    })
    .as_ref()
}

fn ffprobe_json(path: &Path) -> serde_json::Value {
    let out = Command::new(ue_media::ffprobe_bin())
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(path)
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Edición: [1..3s) + [4..6s) del mismo archivo, pegadas. El resultado dura 4 s
/// y el contador quemado debe saltar 1,2 → 4,5. Se validan metadatos con
/// ffprobe y se extraen frames para verificación visual.
#[test]
fn export_cut_and_reordered_timeline() {
    let Some(dir) = media_dir() else { return };
    let mut project = Project::new("export-test");
    let seq_id = project.active_sequence;
    let asset = ue_media::import_file(&dir.join("counter.mp4")).unwrap();
    let asset_id = asset.id;
    project.assets.push(asset);
    let v1 = project
        .sequence(seq_id)
        .unwrap()
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .unwrap()
        .id;
    let mut store = ProjectStore::new(project);
    store.insert_clip(v1, Clip::new_media(asset_id, 1 * SEC, 3 * SEC, 0), InsertMode::Strict).unwrap();
    store.insert_clip(v1, Clip::new_media(asset_id, 4 * SEC, 6 * SEC, 2 * SEC), InsertMode::Strict).unwrap();

    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ue-export-out.mp4");
    export_sequence(&store.project, seq_id, dir, &out, &ExportSettings::default()).unwrap();

    // metadatos
    let meta = ffprobe_json(&out);
    let dur: f64 = meta["format"]["duration"].as_str().unwrap().parse().unwrap();
    assert!((3.9..=4.2).contains(&dur), "duración ≈ 4 s, fue {dur}");
    let streams = meta["streams"].as_array().unwrap();
    let v = streams.iter().find(|s| s["codec_type"] == "video").unwrap();
    assert_eq!(v["codec_name"], "h264");
    assert_eq!(v["width"], 1920); // resolución de la secuencia (con pad)
    assert!(streams.iter().any(|s| s["codec_type"] == "audio"), "lleva el audio del clip");

    // frames para verificación visual: 0.5s → contador "1"; 2.5s → contador "4"
    let frames_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ue-export-frames");
    std::fs::create_dir_all(&frames_dir).unwrap();
    for (name, t) in [("salida_0.5s", 0.5f64), ("salida_2.5s", 2.5f64)] {
        let st = Command::new(ue_media::ffmpeg_bin())
            .args(["-y", "-v", "error", "-ss", &t.to_string(), "-i"])
            .arg(&out)
            .args(["-frames:v", "1"])
            .arg(frames_dir.join(format!("{name}.jpg")))
            .status()
            .unwrap();
        assert!(st.success());
    }
    eprintln!("export verificable en {} y frames en {}", out.display(), frames_dir.display());
}
