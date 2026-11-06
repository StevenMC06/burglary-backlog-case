"""Screenshot the running console (python -I tools/media/shoot.py <out_dir>). Server must be on :8080."""
import os
import sys

from playwright.sync_api import sync_playwright

out = sys.argv[1] if len(sys.argv) > 1 else "media/tmp"
os.makedirs(out, exist_ok=True)
with sync_playwright() as p:
    b = p.chromium.launch(channel="chrome")
    pg = b.new_page(viewport={"width": 1280, "height": 720})
    pg.goto("http://localhost:8080/")
    pg.wait_for_selector(".qbtn")
    pg.click('.qbtn[data-id="CC-26-001996"]')
    pg.wait_for_timeout(1500)
    pg.screenshot(path=os.path.join(out, "case.png"))
    pg.click("#tab-series")
    pg.wait_for_timeout(1500)
    pg.screenshot(path=os.path.join(out, "series.png"))
    b.close()
print("ok")
