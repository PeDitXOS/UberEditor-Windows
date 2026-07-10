//! Throwaway verification against the user's real proxy at the exact
//! timestamps from his bug report (reopen storm at ~18.4s).
fn main() {
    let path = std::env::args().nth(1).expect("usage: verify_playback_fix <video>");
    let path = std::path::PathBuf::from(path);
    let mut transform = ue_core::model::Transform2D::default();
    transform.position.0 = 110.0.into();
    let canvas = Some((1920u32, 1080u32));
    let reg = ue_render::core_registry();

    let mut session: Option<ue_media::stream::MjpegSession> = None;
    let mut session_key: Option<String> = None;
    let (mut opens, mut frames) = (0usize, 0usize);
    for tick in 0..48i64 {
        // his logs: 18.373s advancing ~90 ms per line; we tick at 24 fps
        let t = 18_373_000 + tick * 41_667;
        let resolved = ue_media::frame::ResolvedFrame {
            asset_path: path.to_string_lossy().into_owned(),
            src_t_us: t,
            clip_rel_us: t,
            speed: 1.0,
            effects: vec![],
            transform: transform.clone(),
        };
        let key = ue_tauri_lib::playback_session_key(&resolved, canvas, None);
        let reusable = ue_tauri_lib::should_reuse_session(
            session.as_ref().map(|s| (s.asset_path.as_path(), s.next_src_us())),
            session_key.as_deref() == Some(key.as_str()),
            &path,
            resolved.src_t_us,
        );
        if !reusable {
            let tvar = format!("(t+{:.6})", resolved.clip_rel_us as f64 / 1e6);
            let vf =
                ue_render::clip_vf_at(&reg, &resolved.effects, &resolved.transform, canvas, &tvar);
            session = Some(
                ue_media::stream::MjpegSession::open(&path, t, 960, 24, vf.as_deref()).unwrap(),
            );
            session_key = Some(key);
            opens += 1;
            println!("[open] tick {tick} @ {:.3}s", t as f64 / 1e6);
        }
        if let Some(s) = session.as_mut() {
            while s.next_src_us() <= t {
                match s.next_frame() {
                    Ok(Some(jpeg)) => {
                        frames += 1;
                        if frames % 12 == 0 {
                            println!("[frame] {} bytes @ {:.3}s", jpeg.len(), t as f64 / 1e6);
                        }
                    }
                    other => panic!("stream died: {other:?}"),
                }
            }
        }
    }
    println!("RESULT: opens={opens} frames={frames} over 2 seconds of playback");
    assert_eq!(opens, 1);
}
