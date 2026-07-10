#!/usr/bin/env python
"""DNS64 denoiser sidecar (same engine as the Youtubers-toolkit `denoise`).

Usage: python denoise_dns64.py IN.wav OUT.wav

Reads any wav, denoises speech with Facebook's pretrained DNS64 model and
writes OUT.wav at the INPUT sample rate / channel count (so the output can
replace the input 1:1 in time). Long files are processed in overlapping
chunks with a crossfade to bound memory.

Exit code 0 and a final "ok" line on success; any other output/exit code
makes the caller fall back to the ffmpeg afftdn filter.
"""

import sys

import torch
import torchaudio
from denoiser import pretrained
from denoiser.dsp import convert_audio

CHUNK_S = 60.0
OVERLAP_S = 0.5


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

    wav, sr = torchaudio.load(inp)
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
    den = torchaudio.functional.resample(den, msr, sr)
    if channels > 1:
        den = den.repeat(channels, 1)
    # match the input length exactly (resampling can drift by a few samples)
    target = wav.shape[-1]
    if den.shape[-1] < target:
        den = torch.nn.functional.pad(den, (0, target - den.shape[-1]))
    den = den[:, :target].clamp(-1.0, 1.0)
    torchaudio.save(out, den, sr, encoding="PCM_S", bits_per_sample=16)
    print(f"ok device={device} sr={sr} samples={target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
