"""Record the demo video: drives the console (must be serving on :8080) in step with the
voiceover timings, then muxes audio with ffmpeg.

  .venv/Scripts/python tools/media/record_demo.py          # needs media/tmp/vo/timings.json + voiceover.wav
Output: media/demo.mp4
"""
import json
import subprocess
import sys
import time
from pathlib import Path

import imageio_ffmpeg
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[2]
VO = ROOT / "media" / "tmp" / "vo"
VID = ROOT / "media" / "tmp" / "video"
timings = {t["id"]: t for t in json.loads((VO / "timings.json").read_text(encoding="utf-8"))["segments"]}
VID.mkdir(parents=True, exist_ok=True)
for old in VID.glob("*.webm"):
    old.unlink()

with sync_playwright() as p:
    b = p.chromium.launch(channel="chrome")
    t_ctx = time.time()
    ctx = b.new_context(viewport={"width": 1280, "height": 720}, record_video_dir=str(VID), record_video_size={"width": 1280, "height": 720})
    pg = ctx.new_page()
    pg.goto("http://localhost:8080/")
    pg.wait_for_selector(".qbtn")
    pg.wait_for_timeout(600)
    t0 = time.time()
    lead = t0 - t_ctx  # video seconds before audio zero

    def at(sid, offset=0.0):
        """Sleep until `offset` seconds into the voiceover segment."""
        target = t0 + timings[sid]["start"] + offset
        wait = target - time.time()
        if wait > 0:
            time.sleep(wait)

    # s1: queue overview
    at("s1")
    # s2: open the pawn-hit case; narrative with extracted fields highlighted
    at("s2", 0.4)
    pg.click('.qbtn[data-id="CC-26-001996"]')
    # s3: scroll to property hits
    at("s3", 0.3)
    pg.evaluate("document.querySelector('#case-card').scrollTo({top: 400, behavior: 'smooth'})")
    # s4: series tab, hero series
    at("s4", 0.2)
    pg.click("#tab-series")
    pg.wait_for_timeout(800)
    # s5: decoy group
    at("s5", 0.2)
    pg.select_option('select[aria-label="Series"]', "H-DECOY")
    # s6: back to the packet; accept one next step
    at("s6", 0.2)
    pg.click("#tab-packet")
    pg.wait_for_timeout(1200)
    try:
        pg.locator(".btn.sm.primary", has_text="Accept").first.click(timeout=3000)
    except Exception:
        pass
    # s7: audit drawer
    at("s7", 0.3)
    pg.click("#btn-audit")
    # s8: hold
    at("s8")
    end = t0 + timings["s8"]["end"] + 0.8
    time.sleep(max(0, end - time.time()))
    ctx.close()
    b.close()

webm = next(VID.glob("*.webm"))
out = ROOT / "media" / "demo.mp4"
ff = imageio_ffmpeg.get_ffmpeg_exe()
subprocess.run(
    [ff, "-y", "-ss", f"{lead:.2f}", "-i", str(webm), "-i", str(VO / "voiceover.wav"),
     "-c:v", "libx264", "-pix_fmt", "yuv420p", "-r", "30", "-c:a", "aac", "-b:a", "160k", "-shortest", str(out)],
    check=True, capture_output=True,
)
print("wrote", out)
