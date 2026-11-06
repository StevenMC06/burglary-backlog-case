"""Smoke-check ui/algo.html in a real browser: no JS errors, every panel drew, controls respond.

    .venv/Scripts/python tools/check_algo_page.py http://127.0.0.1:8091/algo.html [shot.png]
"""
import sys

from playwright.sync_api import sync_playwright

url = sys.argv[1]
shot = sys.argv[2] if len(sys.argv) > 2 else None
errors = []
with sync_playwright() as p:
    b = p.chromium.launch(channel="chrome")
    pg = b.new_page(viewport={"width": 1280, "height": 900})
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("console", lambda m: m.type == "error" and errors.append(m.text))
    pg.goto(url, wait_until="networkidle")
    checks = {
        "score": pg.inner_text("#score-out"),
        "waterfall bars": pg.locator("#waterfall rect").count(),
        "cluster nodes": pg.locator("#c-svg circle").count(),
        "cluster groups": pg.locator("#c-legend span").count(),
        "pawn cards": pg.locator("#pw-cards .card").count(),
        "bench bars": pg.locator("#bench-svg rect").count(),
        "bound share": pg.inner_text("#b-share"),
    }
    pg.fill("#s-mo", "0.2")
    pg.dispatch_event("#s-mo", "input")
    checks["score after slider"] = pg.inner_text("#score-out")
    pg.click("#sw-play")
    pg.wait_for_timeout(1500)
    checks["sweep visits"] = pg.inner_text("#sw-visits")
    for _ in range(40):
        pg.click("#tk-step")
    checks["topk skipped"] = pg.inner_text("#tk-skip")
    pg.click("#pw-brute")
    checks["pawn compared (brute)"] = pg.inner_text("#pw-cmp")
    pg.click("#pw-index")
    checks["pawn compared (index)"] = pg.inner_text("#pw-cmp")
    if shot:
        pg.screenshot(path=shot, full_page=True)
    b.close()
for k, v in checks.items():
    print(f"{k}: {v}")
print("errors:", errors or "none")
sys.exit(1 if errors else 0)
