# Voiceover / TTS

Voiceover from text: the **AI · Voiceover** card in the Inspector (visible
when no clip is selected), or the MCP tools `list_tts_voices` +
`generate_speech` (see `docs/MCP.md`). The audio is synthesized in the
background, imported into the media pool (conform/peaks jobs run as for any
import) and optionally dropped at the playhead on an audio track.

## Engines (modular)

Every synthesizer is a `TtsEngine` behind a registry
(`crates/ue-ai/src/tts.rs`). Built-ins:

| id | What | Notes |
|---|---|---|
| `say` | macOS system synthesizer | instant, offline, all system voices (`say -v ?`); rate = words/min |
| `kokoro` | Kokoro-82M AI voice | **self-contained**, same pattern as the DNS64 denoiser: the sidecar (`scripts/tts_kokoro.py`) is embedded in the binary and the app provisions its own venv (`<app_data>/kokoro`, `pip install kokoro`) on first use; the model (~330 MB) lands in the HF cache. `UE_TTS_PYTHON` overrides the interpreter ("off" disables). A Youtubers-toolkit venv with kokoro is used as a dev courtesy when present. Rate = speed × |

## Adding an engine without touching code

Drop a JSON manifest into `<app_config>/tts_engines/` (hover the
"AI · Voiceover" title to see the exact folder). Placeholders: `{text}`,
`{voice}`, `{rate}`, `{out}`; with `"stdin_text": true` the script is piped
to stdin instead of `{text}`. A manifest with a built-in's id replaces it
(user wins, like effect packs). Example — eSpeak NG:

```json
{
  "id": "espeak",
  "name": "eSpeak NG",
  "ext": "wav",
  "argv": ["espeak-ng", "-v", "{voice}", "-s", "{rate}", "-w", "{out}", "{text}"],
  "voices": [{ "id": "es-419", "name": "Español (LatAm)", "lang": "es" }],
  "rate": { "min": 80, "max": 300, "default": 170, "step": 5, "label": "wpm" }
}
```

## Quality upgrade path (researched 2026-07)

Kokoro remains the best *fast* local engine, but its Spanish voices are its
weak spot. The best local quality upgrade today is **Qwen3-TTS via
[mlx-audio](https://github.com/Blaizzy/mlx-audio)** (Apache 2.0, native Apple
Silicon, ~1× real time on M-series, voice cloning from a 3 s sample):

```bash
python -m venv ~/tts-qwen && ~/tts-qwen/bin/pip install mlx-audio
```

then register it as a user engine, e.g.:

```json
{
  "id": "qwen3",
  "name": "Qwen3-TTS (MLX)",
  "ext": "wav",
  "argv": ["/Users/you/tts-qwen/bin/python", "-m", "mlx_audio.tts.generate",
           "--model", "mlx-community/Qwen3-TTS-12Hz-0.6B-CustomVoice-8bit",
           "--voice", "{voice}", "--text", "{text}", "--output", "{out}"],
  "voices": [{ "id": "serena", "name": "Serena", "lang": "es" }]
}
```

(Check `mlx_audio.tts.generate --help` for the exact flags of the installed
version; the CLI has changed between releases.)
