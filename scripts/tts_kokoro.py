#!/usr/bin/env python
"""Kokoro TTS sidecar (hexgrad/Kokoro-82M, the toolkit's voice engine).

Usage: python tts_kokoro.py VOICE SPEED OUT.wav   (script text on stdin)

Synthesizes the stdin text at 24 kHz mono and writes OUT.wav. The language
pipeline is derived from the voice id's first letter (a=en-US, b=en-GB,
e=es, ...). ALL chunks are concatenated — unlike the toolkit's
`audio_generator`, which kept only the first one — so long scripts come out
complete. The model (~330 MB) is downloaded to the HF cache on first use.

Exit code 0 and a final "ok" line on success; anything else is an error the
caller surfaces to the user.
"""

import inspect
import sys
import warnings

warnings.filterwarnings("ignore")

import numpy as np
import soundfile as sf
from kokoro import KPipeline

SAMPLE_RATE = 24000


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: tts_kokoro.py VOICE SPEED OUT.wav (text on stdin)", file=sys.stderr)
        return 2
    voice, speed, out = sys.argv[1], float(sys.argv[2]), sys.argv[3]
    text = sys.stdin.read()

    # kokoro >= 0.9 takes repo_id (and warns without it); 0.7 does not
    kwargs = {}
    if "repo_id" in inspect.signature(KPipeline.__init__).parameters:
        kwargs["repo_id"] = "hexgrad/Kokoro-82M"
    pipe = KPipeline(lang_code=voice[0], **kwargs)

    chunks = [np.asarray(audio) for _, _, audio in pipe(text, voice=voice, speed=speed)]
    if not chunks:
        print("kokoro produced no audio", file=sys.stderr)
        return 1
    sf.write(out, np.concatenate(chunks), SAMPLE_RATE)
    print(f"ok seconds={sum(c.shape[0] for c in chunks) / SAMPLE_RATE:.2f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
