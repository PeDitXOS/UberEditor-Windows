#!/usr/bin/env python
"""DNS64 denoiser sidecar (same engine as the Youtubers-toolkit `denoise`).

Usage: python denoise_dns64.py IN.wav OUT.wav

Reads a PCM wav (the app's conforms are 48 kHz stereo s16le), denoises
speech with Facebook's pretrained DNS64 model and writes OUT.wav at the
INPUT sample rate / channel count, so the output replaces the input 1:1 in
time. Long files are processed in overlapping chunks with a crossfade to
bound memory.

WAV I/O uses the stdlib `wave` + numpy on purpose: torchaudio ≥ 2.9 dropped
its built-in decoder (requires torchcodec), so depending on it made the
sidecar fragile across versions. Resampling uses julius (pure torch, a
denoiser dependency).

Exit code 0 and a final "ok" line on success; anything else makes the
caller fall back to the ffmpeg afftdn filter.
"""

import sys
import wave

import numpy as np
import torch
from denoiser import pretrained
from denoiser.dsp import convert_audio
from julius import resample_frac

CHUNK_S = 60.0
OVERLAP_S = 0.5


def read_wav(path: str) -> tuple[torch.Tensor, int]:
    with wave.open(path, "rb") as f:
        channels = f.getnchannels()
        width = f.getsampwidth()
        sr = f.getframerate()
        raw = f.readframes(f.getnframes())
    if width == 2:
        x = np.frombuffer(raw, dtype="<i2").astype(np.float32) / 32768.0
    elif width == 4:
        x = np.frombuffer(raw, dtype="<i4").astype(np.float32) / 2147483648.0
    elif width == 1:
        x = (np.frombuffer(raw, dtype=np.uint8).astype(np.float32) - 128.0) / 128.0
    else:
        raise ValueError(f"unsupported sample width: {width}")
    wav = torch.from_numpy(x.reshape(-1, channels).T.copy())
    return wav, sr


def write_wav(path: str, wav: torch.Tensor, sr: int) -> None:
    data = (wav.clamp(-1.0, 1.0).numpy().T * 32767.0).astype("<i2")
    with wave.open(path, "wb") as f:
        f.setnchannels(wav.shape[0])
        f.setsampwidth(2)
        f.setframerate(sr)
        f.writeframes(data.tobytes())


def pick_device() -> str:
    if torch.cuda.is_available():
        return "cuda"
    if torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: denoise_dns64.py IN.wav OUT.wav", file=sys.stderr)
        return 2
    inp, out = sys.argv[1], sys.argv[2]

    wav, sr = read_wav(inp)
    channels = wav.shape[0]

    device = pick_device()
    model = pretrained.dns64().eval()
    try:
        model = model.to(device)
        # warmup: some ops may be unsupported on MPS; fail early and retry on CPU
        with torch.no_grad():
            model(torch.zeros(1, model.chin, 1024, device=device))
    except Exception:
        device = "cpu"
        model = model.cpu()

    x = convert_audio(wav.to(device), sr, model.sample_rate, model.chin)[0]
    msr = model.sample_rate
    chunk = int(CHUNK_S * msr)
    overlap = int(OVERLAP_S * msr)

    pieces = []
    pos = 0
    prev_tail = None
    with torch.no_grad():
        while pos < x.shape[-1]:
            seg = x[pos : pos + chunk + overlap]
            den = model(seg[None, None])[0][0].cpu()
            if prev_tail is not None:
                # linear crossfade over the overlap region
                n = min(overlap, den.shape[-1], prev_tail.shape[-1])
                fade = torch.linspace(0.0, 1.0, n)
                den[:n] = prev_tail[:n] * (1 - fade) + den[:n] * fade
            if pos + chunk < x.shape[-1]:
                pieces.append(den[:chunk])
                prev_tail = den[chunk : chunk + overlap]
            else:
                pieces.append(den)
                prev_tail = None
            pos += chunk

    den = torch.cat(pieces)[None]
    den = resample_frac(den, msr, sr)
    if channels > 1:
        den = den.repeat(channels, 1)
    # match the input length exactly (resampling can drift by a few samples)
    target = wav.shape[-1]
    if den.shape[-1] < target:
        den = torch.nn.functional.pad(den, (0, target - den.shape[-1]))
    den = den[:, :target]
    write_wav(out, den, sr)
    print(f"ok device={device} sr={sr} samples={target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
