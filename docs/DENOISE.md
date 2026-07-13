# Background noise removal

`Inspector → Audio → Reduce background noise`, per clip. The clean audio is
rendered **in the background**; when it's ready, playback and export both switch
to the exact same file, so what you hear is what you ship. The result is a
sibling of the conformed audio (`x.wav` → `x.denoise.wav`) and lives in the
cache, not in the project.

## Two engines

**Primary — DNS64 (neural).** Facebook's pretrained denoiser, the same one the
Youtubers-toolkit uses, via the `denoiser` Python package. It runs as a sidecar
script that is embedded in the binary, so packaged builds carry it. On real
voice the speech level is untouched while the noise floor drops dramatically.

**Fallback — ffmpeg `afftdn`.** Dependency-free spectral denoiser
(`afftdn=nr=30:nf=-25:tn=1`), used when no Python is available. Weaker, but it
needs nothing installed.

## Self-contained: the app builds its own venv

There is no "install these packages first" step. On first use the app
provisions its **own** virtualenv under its data directory. All it needs from
you is a system **`python3` / `python` ≥ 3.9** on the `PATH` (`brew install
python`); the one-time download is ~200 MB. Without any Python, the app quietly
falls back to `afftdn`.

The Inspector checkbox tells you which case you're in — it disables itself and
explains why when the neural path can't run.

## Choosing the interpreter

`UE_DENOISER_PYTHON` wins over everything:

```bash
UE_DENOISER_PYTHON=/path/to/venv/bin/python   # use this interpreter
UE_DENOISER_PYTHON=off                        # disable the neural path (also: none, 0)
```

Otherwise the order is: the venv the app provisioned for itself → as a courtesy
on dev machines that have one, a Youtubers-toolkit venv.

The Kokoro TTS voice ([`TTS.md`](TTS.md)) is provisioned the same way, with its
own `UE_TTS_PYTHON`.
