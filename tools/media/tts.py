"""Voiceover synthesis for the demo video.

Usage:
  .venv/Scripts/python tools/media/tts.py <script.json> <out_dir> [--voice af_heart] [--speed 1.0]
  .venv/Scripts/python tools/media/tts.py --samples <out_dir>

script.json: {"segments": [{"id": "s1", "text": "...", "pause_after": 0.4}, ...]}
Writes <out_dir>/<id>.wav per segment, <out_dir>/voiceover.wav (concatenated) and
<out_dir>/timings.json ([{id, start, end}] in seconds) so the screen recording can be paced to it.

Engine: Kokoro-82M (Apache-2.0) via kokoro-onnx, fully local. The ElevenLabs swap only replaces
`synthesize()`; script and timings stay the same.
"""

import json
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from kokoro_onnx import Kokoro

ROOT = Path(__file__).resolve().parents[2]
MODELS = ROOT / "media" / "models"
SR = 24_000


def load() -> Kokoro:
    return Kokoro(str(MODELS / "kokoro-v1.0.onnx"), str(MODELS / "voices-v1.0.bin"))


def synthesize(k: Kokoro, text: str, voice: str, speed: float) -> np.ndarray:
    samples, sr = k.create(text, voice=voice, speed=speed, lang="en-us")
    assert sr == SR, sr
    return samples.astype(np.float32)


def render(script_path: Path, out: Path, voice: str, speed: float) -> None:
    out.mkdir(parents=True, exist_ok=True)
    script = json.loads(script_path.read_text(encoding="utf-8"))
    k = load()
    parts, timings, t = [], [], 0.0
    for seg in script["segments"]:
        audio = synthesize(k, seg["text"], voice, speed)
        sf.write(out / f"{seg['id']}.wav", audio, SR)
        dur = len(audio) / SR
        timings.append({"id": seg["id"], "start": round(t, 3), "end": round(t + dur, 3), "text": seg["text"]})
        pause = float(seg.get("pause_after", 0.35))
        parts += [audio, np.zeros(int(pause * SR), dtype=np.float32)]
        t += dur + pause
    sf.write(out / "voiceover.wav", np.concatenate(parts), SR)
    (out / "timings.json").write_text(json.dumps({"voice": voice, "speed": speed, "total": round(t, 3), "segments": timings}, indent=2))
    print(f"voiceover.wav {t:.1f}s, {len(timings)} segments -> {out}")


def samples(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    k = load()
    line = ("Cascade City doesn't have ten thousand burglaries. It has a few hundred burglars. "
            "Casefile connects their cases, and a detective makes every decision.")
    for v in ["af_heart", "af_bella", "am_michael", "am_fenrir", "bf_emma", "bm_george"]:
        sf.write(out / f"sample_{v}.wav", synthesize(k, line, v, 1.0), SR)
        print("wrote", v)


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--samples":
        samples(Path(args[1]))
    else:
        voice = args[args.index("--voice") + 1] if "--voice" in args else "af_heart"
        speed = float(args[args.index("--speed") + 1]) if "--speed" in args else 1.0
        render(Path(args[0]), Path(args[1]), voice, speed)
