# Architecture

## Crates

| Crate | What it does |
|---|---|
| `ue-core` | Pure model, actions with mechanical inverses, transactional history, keyframe curves. No media IO, no GPU, no Tauri |
| `ue-media` | ffprobe, hashing, preview frames (MJPEG), proxies, thumbnails, audio conforming, denoise |
| `ue-audio` | mmap'd WAV, pure mixer (testable), cpal output (master clock), peaks |
| `ue-render` | Effect and generator packs (manifest → ffmpeg chain), transform and animation expressions |
| `ue-text` | Own glyph rasterization, so emoji and CJK actually render (drawtext has no font fallback, libass has no color-font support) |
| `ue-export` | EDL, ffmpeg graph (multi-layer, transitions, text/karaoke), single-frame preview compositor, avatar-video generation, progress and cancellation |
| `ue-ai` | Silence detection (hysteresis + padding), emotion classification, TTS engines |
| `ue-whisper` | whisper-rs (Metal/CUDA), per-word timestamps, model downloads |
| `src-tauri` | IPC commands, MCP server, autosave |

## The golden rule

**One compositor.** The paused preview, live playback and the export all go
through the same ffmpeg chains — the same effect templates, the same transform
and keyframe math, the same text rasterization. A feature that renders
differently in any of the three is a bug, and the pixel tests exist to catch
exactly that.

The corollary is that effects and generators are *data*
([`PACKS.md`](PACKS.md)): a `manifest.json` can't drift between preview and
export because there is only one of it.

## Conventions

- **Time is microseconds** (`i64`) everywhere in the model; fps is rational.
- **Ids are ULIDs.**
- **Every action has a mechanical inverse**, so undo is exact and one user
  operation — including the AI ones and anything an agent does over MCP — is
  **one** history entry.
- The frontend **mirrors the serde types by hand** (`src/engine/types.ts`) and
  has two interchangeable engines: `TauriEngine` (real IPC) and `MockEngine`
  (browser demo, `npm run dev`).

## Tests

`cargo test` — 182 tests: unit, plus **pixel tests over real ffmpeg exports**
(render a frame, assert on the actual pixels). `npm run screenshot` drives the
real UI through 15 steps with functional assertions.

MCP parity is enforced too: a UI feature reachable through no tool fails
`mcp_tests.rs` on purpose ([`MCP.md`](MCP.md)).
