"""Generate the README algorithm figures (animated SVG) from evals/bench.md and the engine constants.

    python -I tools/algo_figs.py            # writes docs/img/*.svg

Numbers come only from `casefile bench` output; nothing is typed in by hand.
"""
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "img"
INK, MUTED, LINE, ACCENT, PASS, FAIL, NAVY, REF = "#16131f", "#555c58", "#d5ddd7", "#6425ff", "#0f6a2e", "#b01836", "#16131f", "#c9d2cc"
FONT = 'font-family="Segoe UI, Helvetica, Arial, sans-serif"'
# crates/core/src/linkage.rs
INTERCEPT, W_MO, W_DIST, W_TIME, W_SIG = -0.4, 6.0, -1.3, -0.9, 0.6


def parse_bench(md: str):
    rows, funnel = [], []
    for line in md.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) == 8 and cells[0].isdigit():
            rows.append(dict(k=int(cells[0]), cases=int(cells[1]), step=cells[3].split(" (")[0].split(" k=")[0],
                             ref=float(cells[4]), opt=float(cells[5]), same=cells[7] == "yes"))
        elif len(cells) == 6 and re.match(r"^0\.\d+$", cells[0]):
            funnel.append((float(cells[0]), [int(c) for c in cells[1:]]))
    return rows, funnel


def svg(w, h, body, extra_style=""):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" {FONT}>'
            f'<style>text{{fill:{INK}}} .m{{fill:{MUTED}}} {extra_style}</style>'
            f'<rect width="{w}" height="{h}" rx="10" fill="#fff" stroke="{LINE}"/>{body}</svg>')


def bench_svg(rows):
    w, top, rh = 900, 58, 30
    h = top + rh * len(rows) + 46
    lo, hi = math.log10(5), math.log10(max(r["ref"] for r in rows) * 1.4)
    x0, x1 = 290, w - 70
    X = lambda ms: x0 + (math.log10(max(ms, 5)) - lo) / (hi - lo) * (x1 - x0)
    b = [f'<text x="20" y="30" font-size="17" font-weight="650">Brute force vs optimised (log scale, median of 3, identical output)</text>',
         f'<rect x="20" y="40" width="10" height="10" fill="{REF}"/><text x="36" y="49" font-size="12" class="m">reference</text>'
         f'<rect x="110" y="40" width="10" height="10" fill="{ACCENT}"/><text x="126" y="49" font-size="12" class="m">optimised</text>']
    for t in (10, 100, 1000, 10000, 100000):
        if math.log10(t) <= hi:
            b.append(f'<line x1="{X(t):.1f}" x2="{X(t):.1f}" y1="{top - 4}" y2="{h - 36}" stroke="{LINE}"/>'
                     f'<text x="{X(t) - 14:.1f}" y="{h - 18}" font-size="11" class="m">{f"{t // 1000}s" if t >= 1000 else f"{t}ms"}</text>')
    for i, r in enumerate(rows):
        y = top + i * rh
        d = 0.08 * i
        b.append(f'<text x="20" y="{y + 15}" font-size="12">{r["step"]} · {r["cases"]:,} cases</text>')
        for v, fill, dy in ((r["ref"], REF, 0), (r["opt"], ACCENT, 12)):
            wd = X(v) - x0
            b.append(f'<rect x="{x0}" y="{y + dy}" height="10" rx="2" fill="{fill}" width="{wd:.1f}">'
                     f'<animate attributeName="width" from="0" to="{wd:.1f}" dur="1.2s" begin="{d:.2f}s" fill="freeze" calcMode="spline" keySplines=".2 .8 .2 1" keyTimes="0;1"/></rect>')
        b.append(f'<text x="{max(X(r["ref"]), X(r["opt"])) + 8:.1f}" y="{y + 16}" font-size="12" font-weight="700" fill="{PASS}">{r["ref"] / r["opt"]:.1f}×</text>')
    return svg(w, h, "".join(b))


def funnel_svg(funnel):
    th, f = next((t, f) for t, f in funnel if abs(t - 0.9) < 1e-9)
    labels = ["all pairs  n(n−1)/2", "inside 180-day window (sweep)", "inside 30 km box", "pass score upper bound", "edges ≥ 0.9"]
    w, h, x0, x1 = 900, 300, 300, 690
    lo, hi = 0, math.log10(f[0])
    X = lambda v: x0 + (math.log10(max(v, 1)) - lo) / (hi - lo) * (x1 - x0)
    b = [f'<text x="20" y="30" font-size="17" font-weight="650">Pair funnel: 2,000 seed cases, threshold {th} (log scale)</text>']
    colors = [REF, "#d2c2ff", "#d2c2ff", ACCENT, PASS]
    for i, (lab, v) in enumerate(zip(labels, f)):
        y = 56 + i * 44
        wd = X(v) - x0
        b.append(f'<text x="20" y="{y + 19}" font-size="13">{lab}</text>'
                 f'<rect x="{x0}" y="{y}" height="26" rx="3" fill="{colors[i]}" width="{wd:.1f}">'
                 f'<animate attributeName="width" from="0" to="{wd:.1f}" dur="1s" begin="{0.25 * i:.2f}s" fill="freeze"/></rect>'
                 f'<text x="{x0 + wd + 8:.1f}" y="{y + 18}" font-size="13" font-weight="650">{v:,}'
                 f'<tspan class="m" font-weight="400"> ({100 * v / f[0]:.2f}%)</tspan></text>')
    b.append(f'<text x="20" y="{h - 14}" font-size="12" class="m">Only the {f[3]:,} pairs that pass the bound get MO scoring; brute force scored all {f[2]:,} in the window.</text>')
    return svg(w, h, "".join(b))


def bound_svg():
    """Region of (distance, days) where a pair can still reach the threshold, for several thresholds."""
    w, h, L, B, T, R = 900, 380, 70, 50, 50, 30
    KM, D = 25.0, 180.0
    pw, ph = w - L - R, h - T - B
    X = lambda km: L + km / KM * pw
    Y = lambda d: T + (1 - d / D) * ph
    b = [f'<text x="20" y="30" font-size="17" font-weight="650">Upper-bound region: a pair outside its curve cannot link, so MO scoring is skipped</text>']
    for k in range(0, 26, 5):
        b.append(f'<line x1="{X(k)}" x2="{X(k)}" y1="{T}" y2="{T + ph}" stroke="{LINE}"/><text x="{X(k) - 12}" y="{T + ph + 18}" font-size="11" class="m">{k} km</text>')
    for d in range(0, 181, 30):
        b.append(f'<line x1="{L}" x2="{L + pw}" y1="{Y(d)}" y2="{Y(d)}" stroke="{LINE}"/><text x="{L - 36}" y="{Y(d) + 4}" font-size="11" class="m">{d} d</text>')
    for i, (thr, sig, col) in enumerate(((0.5, 1, "#d2c2ff"), (0.9, 1, ACCENT), (0.9, 0, NAVY), (0.99, 0, FAIL))):
        c = INTERCEPT + W_MO + W_SIG * sig - math.log(thr / (1 - thr))
        pts = []
        for s in range(0, 401):
            km = KM * s / 400
            days = math.exp((c + W_DIST * math.log1p(km)) / -W_TIME) - 1
            if 0 <= days <= D:
                pts.append(f"{X(km):.1f},{Y(days):.1f}")
            elif days > D:
                pts.append(f"{X(km):.1f},{Y(D):.1f}")
        if not pts:
            continue
        path = "M" + " L".join(pts)
        length = 1600
        b.append(f'<path d="{path}" fill="none" stroke="{col}" stroke-width="2.5" stroke-dasharray="{length}" stroke-dashoffset="{length}">'
                 f'<animate attributeName="stroke-dashoffset" from="{length}" to="0" dur="1.4s" begin="{0.35 * i:.2f}s" fill="freeze"/></path>'
                 f'<rect x="{w - 250}" y="{T + 8 + i * 20}" width="14" height="4" fill="{col}"/>'
                 f'<text x="{w - 230}" y="{T + 14 + i * 20}" font-size="12">threshold {thr}, {sig} shared signature{"s" if sig != 1 else ""}</text>')
    b.append(f'<text x="{L}" y="{h - 10}" font-size="12" class="m">Curve: 1.3·ln(1+km) + 0.9·ln(1+days) = 5.6 + 0.6·sig − logit(threshold). Below-left of a curve the pair is scored; elsewhere skipped.</text>')
    return svg(w, h, "".join(b))


def sweep_svg():
    """Animated time sweep: the window slides across time-sorted cases."""
    import random
    rnd = random.Random(7)
    w, h, L, R = 900, 200, 30, 30
    days = sorted(rnd.uniform(0, 365) for _ in range(70))
    X = lambda d: L + d / 365 * (w - L - R)
    win = 180 / 365 * (w - L - R)
    b = [f'<text x="20" y="28" font-size="17" font-weight="650">Time sweep: each case is compared only with later cases inside the window</text>',
         f'<g><rect x="{L}" y="44" width="{win:.1f}" height="110" rx="6" fill="{ACCENT}" fill-opacity=".10" stroke="{ACCENT}" stroke-opacity=".5">'
         f'<animate attributeName="x" values="{L};{w - R - 20};{L}" keyTimes="0;.92;1" dur="8s" repeatCount="indefinite"/></rect>'
         f'<line y1="40" y2="158" stroke="{NAVY}" stroke-width="2"><animate attributeName="x1" values="{L};{w - R - 20};{L}" keyTimes="0;.92;1" dur="8s" repeatCount="indefinite"/>'
         f'<animate attributeName="x2" values="{L};{w - R - 20};{L}" keyTimes="0;.92;1" dur="8s" repeatCount="indefinite"/></line></g>']
    for i, d in enumerate(days):
        b.append(f'<circle cx="{X(d):.1f}" cy="{58 + (i * 37 % 11) * 9}" r="4" fill="{MUTED}" fill-opacity=".7"/>')
    for m in range(0, 13, 2):
        b.append(f'<text x="{X(m * 30.4) - 8:.1f}" y="176" font-size="11" class="m">{round(m * 30.4)}d</text>')
    b.append(f'<text x="20" y="194" font-size="12" class="m">Sorted by offense time, the inner loop breaks at the first case past 180 days: every later case is further still.</text>')
    return svg(w, h, "".join(b))


def main():
    md = (ROOT / "evals" / "bench.md").read_text(encoding="utf-8")
    rows, funnel = parse_bench(md)
    if not rows or not funnel:
        sys.exit("evals/bench.md has no bench table; run `casefile bench` first")
    if not all(r["same"] for r in rows):
        sys.exit("bench reports a mismatch; refusing to chart it")
    OUT.mkdir(parents=True, exist_ok=True)
    for name, s in (("bench.svg", bench_svg(rows)), ("funnel.svg", funnel_svg(funnel)), ("bound.svg", bound_svg()), ("sweep.svg", sweep_svg())):
        (OUT / name).write_text(s, encoding="utf-8")
        print("wrote", OUT / name)


if __name__ == "__main__":
    main()
