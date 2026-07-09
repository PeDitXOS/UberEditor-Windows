//! Tests del detector de silencios con WAVs sintéticos exactos.

use std::path::PathBuf;

use ue_ai::silence::{clip_silences_on_timeline, detect_silences, detect_speech, SilenceParams};
use ue_audio::wav::WavMap;
use ue_audio::RATE;

const SEC: i64 = 1_000_000;

/// WAV estéreo 48k: por cada tramo (duración_ms, amplitud 0..1) genera ruido
/// blanco determinista a esa amplitud (0 = silencio digital).
fn write_pattern(name: &str, spans: &[(i64, f64)]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ue-ai-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let mut rng: u32 = 0x12345678;
    for &(ms, amp) in spans {
        let frames = ms * RATE as i64 / 1000;
        for _ in 0..frames {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = ((rng >> 8) as f64 / 8_388_608.0) - 1.0; // [-1, 1)
            let v = (noise * amp * 30000.0) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
    }
    w.finalize().unwrap();
    path
}

#[test]
fn detects_speech_and_silence_blocks() {
    // 1s habla, 1s silencio, 1s habla
    let path = write_pattern("basic.wav", &[(1000, 0.3), (1000, 0.0), (1000, 0.3)]);
    let wav = WavMap::open(&path).unwrap();
    let p = SilenceParams::default();

    let speech = detect_speech(&wav, 0, 3 * SEC, &p);
    assert_eq!(speech.len(), 2, "dos bloques de habla: {speech:?}");
    // márgenes: el detector con padding debe cubrir aproximadamente [0..1s+pad] y [2s-pad..3s]
    let (s1, e1) = speech[0];
    let (s2, e2) = speech[1];
    assert!(s1 < 100_000, "el primer bloque empieza al inicio: {s1}");
    assert!((900_000..=1_350_000).contains(&e1), "fin del primer bloque ≈1s+pad: {e1}");
    assert!((1_700_000..=2_100_000).contains(&s2), "inicio del segundo ≈2s-pad: {s2}");
    assert!(e2 > 2_900_000, "el segundo bloque llega al final: {e2}");

    let silences = detect_silences(&wav, 0, 3 * SEC, &p);
    assert_eq!(silences.len(), 1, "un silencio central: {silences:?}");
    let (ss, se) = silences[0];
    assert!(ss >= e1 && se <= s2 + 1, "el silencio es el hueco entre bloques");
}

#[test]
fn short_pauses_are_breathing_not_silence() {
    // pausa de 250 ms (< min_silence de 400 ms) → NO se corta
    let path = write_pattern("breath.wav", &[(800, 0.3), (250, 0.0), (800, 0.3)]);
    let wav = WavMap::open(&path).unwrap();
    let speech = detect_speech(&wav, 0, 2 * SEC, &SilenceParams::default());
    assert_eq!(speech.len(), 1, "una respiración no parte la habla: {speech:?}");
}

#[test]
fn short_speech_islands_are_dropped() {
    // click de 60 ms entre silencios largos → se descarta
    let path = write_pattern("click.wav", &[(1000, 0.0), (60, 0.5), (1000, 0.0)]);
    let wav = WavMap::open(&path).unwrap();
    let speech = detect_speech(&wav, 0, 2 * SEC, &SilenceParams::default());
    assert!(speech.is_empty(), "un click no es habla: {speech:?}");
}

#[test]
fn all_silence_and_all_speech_edges() {
    let silent = write_pattern("allsilent.wav", &[(1500, 0.0)]);
    let wav = WavMap::open(&silent).unwrap();
    let p = SilenceParams::default();
    assert!(detect_speech(&wav, 0, 2 * SEC, &p).is_empty());
    let sil = detect_silences(&wav, 0, 1_500_000, &p);
    assert_eq!(sil, vec![(0, 1_500_000)]);

    let loud = write_pattern("allloud.wav", &[(1500, 0.4)]);
    let wav = WavMap::open(&loud).unwrap();
    assert!(detect_silences(&wav, 0, 1_500_000, &p).is_empty());
}

#[test]
fn maps_to_timeline_with_clip_offsets() {
    // el clip usa [1s..3s) del archivo y está en el timeline en t=10s
    let path = write_pattern("mapped.wav", &[(1000, 0.3), (1000, 0.0), (1000, 0.3)]);
    let wav = WavMap::open(&path).unwrap();
    let ranges =
        clip_silences_on_timeline(&wav, 10 * SEC, 1 * SEC, 3 * SEC, &SilenceParams::default());
    assert_eq!(ranges.len(), 1, "{ranges:?}");
    let (s, e) = ranges[0];
    // el silencio del archivo [~1s..~2s] visto desde src_in=1s empieza ~0 → timeline ~10s
    assert!((10 * SEC..=10 * SEC + 400_000).contains(&s), "inicio en timeline: {s}");
    assert!((10_800_000..=11_300_000).contains(&e), "fin en timeline: {e}");
}
