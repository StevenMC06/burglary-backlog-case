// Interactive explainer for the Casefile engine. Constants mirror crates/core/src/linkage.rs.
"use strict";
const K = { INTERCEPT: -0.4, W_MO: 6.0, W_DIST: -1.3, W_TIME: -0.9, W_SIG: 0.6, SIG_CAP: 3, NO_MO: 0.5,
  W: { premise: .20, entry: .15, method: .15, band: .10, weekday: .05, props: .15, sigs: .20 } };
// From `casefile bench --reps 3 --scales 1,2,4` (evals/bench.md). Milliseconds, median of 3.
const BENCH = [
  { step: "propose_clusters", cases: 2000, ref: 221.8, opt: 13.5 },
  { step: "all_hits", cases: 2000, ref: 5260.0, opt: 98.2 },
  { step: "similar_cases", cases: 2000, ref: 24.2, opt: 12.3 },
  { step: "propose_clusters", cases: 4000, ref: 554.3, opt: 36.9 },
  { step: "all_hits", cases: 4000, ref: 21265.8, opt: 367.1 },
  { step: "similar_cases", cases: 4000, ref: 42.5, opt: 20.2 },
  { step: "propose_clusters", cases: 8000, ref: 1109.7, opt: 61.8 },
  { step: "all_hits", cases: 8000, ref: 86401.8, opt: 1795.5 },
  { step: "similar_cases", cases: 8000, ref: 105.6, opt: 37.0 },
];

const $ = (id) => document.getElementById(id);
const sigmoid = (z) => 1 / (1 + Math.exp(-Math.max(-40, Math.min(40, z))));
const fmt = (n) => n.toLocaleString("en-US");
const NS = "http://www.w3.org/2000/svg";
function el(tag, attrs = {}, text) {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  if (text !== undefined) e.textContent = text;
  return e;
}
function terms(mo, km, days, sig) {
  return { intercept: K.INTERCEPT, mo: K.W_MO * mo, dist: K.W_DIST * Math.log1p(Math.max(0, km)),
    time: K.W_TIME * Math.log1p(Math.max(0, days)), sig: K.W_SIG * Math.min(sig, K.SIG_CAP) };
}
function score(mo, km, days, sig, hasMo = true) {
  const t = terms(mo, km, days, sig);
  const p = sigmoid(t.intercept + t.mo + t.dist + t.time + t.sig);
  return hasMo ? p : p * K.NO_MO;
}

// ---- deterministic synthetic cases (planted series + background noise) ----------------------
function rng(seed) { return () => { seed |= 0; seed = (seed + 0x6D2B79F5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }
function makeCases(seed = 7) {
  const r = rng(seed), pick = (n) => Math.floor(r() * n);
  const randMo = () => ({ premise: pick(4), entry: pick(6), method: pick(6), band: pick(5), weekday: r() < .7,
    props: (1 << pick(8)) | (r() < .5 ? 1 << pick(8) : 0), sigs: new Set(r() < .3 ? ["s" + pick(12)] : []) });
  const cases = [];
  for (let s = 0; s < 6; s++) {
    const base = randMo(); base.sigs = new Set(["sig" + s]);
    const cx = 2 + r() * 16, cy = 2 + r() * 10, t0 = r() * 300, size = 4 + pick(6);
    for (let i = 0; i < size; i++) {
      const mo = { ...base, sigs: new Set(base.sigs) };
      if (r() < .3) mo.band = pick(5);
      if (r() < .2) mo.props = 1 << pick(8);
      cases.push({ id: `S${s}-${i}`, series: s, x: cx + (r() - .5) * 2.4, y: cy + (r() - .5) * 2.4, day: t0 + r() * 45, mo });
    }
  }
  for (let i = 0; i < 150; i++) cases.push({ id: `N${i}`, series: -1, x: r() * 20, y: r() * 14, day: r() * 365, mo: randMo() });
  return cases;
}
const pop = (m) => { let c = 0; while (m) { c += m & 1; m >>>= 1; } return c; };
function jac(a, b) { const i = pop(a & b), u = pop(a) + pop(b) - i; return u ? i / u : 0; }
function moSim(a, b) {
  const x = a.mo, y = b.mo; const W = K.W;
  const inter = [...x.sigs].filter((s) => y.sigs.has(s)).length, uni = x.sigs.size + y.sigs.size - inter;
  return W.premise * (x.premise === y.premise) + W.entry * (x.entry === y.entry) + W.method * (x.method === y.method)
    + W.band * (x.band === y.band) + W.weekday * (x.weekday === y.weekday) + W.props * jac(x.props, y.props)
    + W.sigs * (uni ? inter / uni : 0);
}
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
function pairScore(a, b) {
  const shared = [...a.mo.sigs].filter((s) => b.mo.sigs.has(s)).length;
  return score(moSim(a, b), dist(a, b), Math.abs(a.day - b.day), shared);
}
function pairBound(a, b) { return score(1, dist(a, b), Math.abs(a.day - b.day), Math.min(a.mo.sigs.size, b.mo.sigs.size)); }
const CASES = makeCases();

// ---- 1. pair score waterfall ----------------------------------------------------------------
function renderScore() {
  const mo = +$("s-mo").value, km = +$("s-km").value, days = +$("s-days").value, sig = +$("s-sig").value;
  const thr = +$("s-thr").value, hasMo = $("s-hasmo").checked;
  $("v-mo").textContent = mo.toFixed(2); $("v-km").textContent = km.toFixed(1); $("v-days").textContent = days;
  $("v-sig").textContent = sig; $("v-thr").textContent = thr.toFixed(2);
  const t = terms(hasMo ? mo : 0, km, days, hasMo ? sig : 0), z = t.intercept + t.mo + t.dist + t.time + t.sig;
  const s = score(hasMo ? mo : 0, km, days, hasMo ? sig : 0, hasMo);
  $("score-out").textContent = s.toFixed(3);
  $("z-out").textContent = `z = ${z.toFixed(2)}${hasMo ? "" : " · ×0.5 no-MO discount"}${sig > K.SIG_CAP ? " · signatures capped at 3" : ""}`;
  $("score-pill").innerHTML = s >= thr ? '<span class="pill ok">edge: linked for review</span>' : '<span class="pill no">below threshold</span>';
  const svg = $("waterfall"); svg.replaceChildren();
  const rows = [["intercept", t.intercept], ["MO × 6.0", t.mo], ["ln(1+km) × −1.3", t.dist], ["ln(1+days) × −0.9", t.time], ["signatures × 0.6", t.sig]];
  const X = (v) => 320 + v * 32; let acc = 0;
  svg.append(el("line", { x1: X(0), x2: X(0), y1: 6, y2: 196, stroke: "#9aa4b1", "stroke-dasharray": "3 3" }));
  rows.forEach(([name, v], i) => {
    const y = 10 + i * 34, x0 = X(acc), x1 = X(acc + v);
    svg.append(el("text", { x: 8, y: y + 17, "font-size": 12, fill: "#657080" }, name));
    const r = el("rect", { x: Math.min(x0, x1), y, width: Math.max(1.5, Math.abs(x1 - x0)), height: 24, rx: 3, fill: v >= 0 ? "#276345" : "#a23c34", opacity: .85 });
    r.style.transition = "all .25s"; svg.append(r);
    svg.append(el("text", { x: Math.max(x0, x1) + 6, y: y + 17, "font-size": 12, fill: "#202936" }, (v >= 0 ? "+" : "") + v.toFixed(2)));
    acc += v;
  });
  svg.append(el("text", { x: 8, y: 204, "font-size": 11, fill: "#657080" }, `Σ = z = ${acc.toFixed(2)} → σ(z) = ${sigmoid(acc).toFixed(3)}`));
}
["s-mo", "s-km", "s-days", "s-sig", "s-thr", "s-hasmo"].forEach((id) => $(id).addEventListener("input", renderScore));

// ---- 2. time sweep ----------------------------------------------------------------------------
const SW = { order: [...CASES].sort((a, b) => a.day - b.day), p: 0, visits: 0, playing: false, lanes: CASES.map((_, i) => (i * 37) % 23) };
function swDraw() {
  const cv = $("sw-canvas"), g = cv.getContext("2d"), W = +$("sw-win").value, n = SW.order.length;
  $("v-win").textContent = W;
  const X = (d) => 30 + (d / 365) * (cv.width - 60), lane = new Map(CASES.map((c, i) => [c, SW.lanes[i]]));
  g.clearRect(0, 0, cv.width, cv.height);
  g.fillStyle = "#657080"; g.font = "12px Segoe UI";
  for (let m = 0; m <= 12; m++) { const x = X(m * 365 / 12); g.fillRect(x, 200, 1, 6); g.fillText(`${Math.round(m * 30.4)}d`, x - 8, 222); }
  const cur = SW.order[Math.min(SW.p, n - 1)];
  if (SW.p < n) { g.fillStyle = "rgba(36,90,181,.10)"; g.fillRect(X(cur.day), 4, X(cur.day + W) - X(cur.day), 190); }
  for (let q = 0; q < n; q++) {
    const c = SW.order[q], inWin = SW.p < n && q > SW.p && c.day - cur.day <= W;
    g.fillStyle = q === SW.p ? "#182534" : inWin ? "#245ab5" : q < SW.p ? "#d5dae1" : "#9aa4b1";
    g.beginPath(); g.arc(X(c.day), 12 + lane.get(c) * 8, q === SW.p ? 6 : 3.5, 0, 7); g.fill();
  }
  const brute = n * (n - 1) / 2;
  $("sw-n").textContent = n; $("sw-brute").textContent = fmt(brute); $("sw-visits").textContent = fmt(SW.visits);
  $("sw-ratio").textContent = SW.p >= n ? `${(100 * SW.visits / brute).toFixed(1)}%` : "…";
}
function swStep() {
  const n = SW.order.length, W = +$("sw-win").value;
  if (SW.p >= n) { SW.playing = false; $("sw-play").textContent = "Play"; return; }
  const a = SW.order[SW.p];
  for (let q = SW.p + 1; q < n && SW.order[q].day - a.day <= W; q++) SW.visits++;
  SW.p++; swDraw();
}
function swLoop() { if (!SW.playing) return; swStep(); setTimeout(swLoop, 45); }
$("sw-play").onclick = () => { if (SW.p >= SW.order.length) swReset(); SW.playing = !SW.playing; $("sw-play").textContent = SW.playing ? "Pause" : "Play"; swLoop(); };
function swReset() { SW.p = 0; SW.visits = 0; swDraw(); }
$("sw-reset").onclick = () => { SW.playing = false; $("sw-play").textContent = "Play"; swReset(); };
$("sw-win").addEventListener("input", swReset);

// ---- 3. bound heatmap with real pairs ------------------------------------------------------------
const PAIRS = (() => {
  const out = [];
  for (let i = 0; i < CASES.length; i++) for (let j = i + 1; j < CASES.length; j++) {
    const a = CASES[i], b = CASES[j], days = Math.abs(a.day - b.day);
    if (days <= 180) out.push({ km: dist(a, b), days, bound: pairBound(a, b), score: pairScore(a, b) });
  }
  return out;
})();
function renderBound() {
  const thr = +$("b-thr").value, sig = +$("b-sig").value, cv = $("b-canvas"), g = cv.getContext("2d");
  $("v-bthr").textContent = thr.toFixed(2); $("v-bsig").textContent = sig;
  const L = 50, B = 40, w = cv.width - L - 10, h = cv.height - B - 10, KM = 25, D = 180;
  const img = g.createImageData(w, h); let kept = 0;
  for (let py = 0; py < h; py++) for (let px = 0; px < w; px++) {
    const km = (px / w) * KM, days = (1 - py / h) * D, ok = score(1, km, days, sig) >= thr, o = 4 * (py * w + px);
    if (ok) kept++;
    img.data.set(ok ? [220, 231, 250, 255] : [246, 247, 249, 255], o);
  }
  g.clearRect(0, 0, cv.width, cv.height); g.putImageData(img, L, 10);
  // contour: bound == thr  ⇔  1.3 ln(1+km) + 0.9 ln(1+days) = c
  const c = K.INTERCEPT + K.W_MO + K.W_SIG * Math.min(sig, 3) - Math.log(thr / (1 - thr));
  g.strokeStyle = "#245ab5"; g.lineWidth = 2; g.beginPath(); let started = false;
  for (let px = 0; px <= w; px++) {
    const km = (px / w) * KM, days = Math.exp((c + K.W_DIST * Math.log1p(km)) / -K.W_TIME) - 1;
    if (days < 0 || days > D) { started = false; continue; }
    const y = 10 + (1 - days / D) * h; if (!started) { g.moveTo(L + px, y); started = true; } else g.lineTo(L + px, y);
  }
  g.stroke();
  let shown = 0;
  for (const p of PAIRS) {
    if (p.km > KM || (shown++ % 3 && p.bound < thr)) continue;
    g.fillStyle = p.score >= thr ? "#276345" : p.bound >= thr ? "#245ab5" : "rgba(101,112,128,.35)";
    g.beginPath(); g.arc(L + (p.km / KM) * w, 10 + (1 - p.days / D) * h, p.score >= thr ? 3.5 : 2, 0, 7); g.fill();
  }
  g.fillStyle = "#657080"; g.font = "12px Segoe UI";
  for (let k = 0; k < KM; k += 5) g.fillText(`${k} km`, L + (k / KM) * w - 10, cv.height - 18);
  for (let d = 0; d <= D; d += 30) g.fillText(`${d}d`, 12, 14 + (1 - d / D) * h);
  g.fillText("distance →", cv.width - 80, cv.height - 4); g.fillText("days apart ↑", 12, cv.height - 4);
  $("b-share").textContent = `${(100 * kept / (w * h)).toFixed(1)}%`;
}
["b-thr", "b-sig"].forEach((id) => $(id).addEventListener("input", renderBound));

// ---- 4. union-find clustering with re-split -------------------------------------------------------
const PALETTE = ["#245ab5", "#276345", "#a23c34", "#805d16", "#6a3fb5", "#0f7a8a", "#b5487a", "#3f6b2a", "#c2611f", "#2f4f8f"];
function components(nodes, edges, t) {
  const parent = new Map(nodes.map((i) => [i, i]));
  const find = (x) => { while (parent.get(x) !== x) { parent.set(x, parent.get(parent.get(x))); x = parent.get(x); } return x; };
  for (const [i, j, s] of edges) if (s >= t) { const a = find(i), b = find(j); if (a !== b) parent.set(Math.max(a, b), Math.min(a, b)); }
  const groups = new Map();
  for (const [i, j, s] of edges) if (s >= t) for (const v of [i, j]) { const r = find(v); if (!groups.has(r)) groups.set(r, new Set()); groups.get(r).add(v); }
  return [...groups.values()].map((g) => [...g].sort((a, b) => a - b)).filter((g) => g.length >= 2);
}
const C_EDGES = (() => { const e = []; for (let i = 0; i < CASES.length; i++) for (let j = i + 1; j < CASES.length; j++) {
  if (Math.abs(CASES[i].day - CASES[j].day) > 180) continue; const s = pairScore(CASES[i], CASES[j]); if (s >= .3) e.push([i, j, s]); } return e; })();
function renderClusters() {
  const thr = +$("c-thr").value, split = $("c-split").checked, max = Math.max(3, +$("c-max").value || 8);
  $("v-cthr").textContent = thr.toFixed(2);
  let work = components(CASES.map((_, i) => i), C_EDGES, thr).map((g) => [g, thr]), done = [], dropped = 0;
  while (work.length) {
    const [g, t] = work.pop();
    if (!split || g.length <= max) { done.push([g, t]); continue; }
    if (t + .05 > .995) { dropped += g.length; continue; }
    const set = new Set(g); const sub = C_EDGES.filter(([i, j]) => set.has(i) && set.has(j));
    for (const h of components(g, sub, t + .05)) work.push([h, t + .05]);
  }
  done.sort((a, b) => a[0][0] - b[0][0]);
  const color = new Map(); done.forEach(([g], k) => g.forEach((i) => color.set(i, PALETTE[k % PALETTE.length])));
  const svg = $("c-svg"); svg.replaceChildren();
  const X = (c) => 20 + (c.x / 20) * 1060, Y = (c) => 15 + (c.y / 14) * 350;
  for (const [i, j, s] of C_EDGES) if (s >= thr) {
    const same = color.get(i) && color.get(i) === color.get(j);
    svg.append(el("line", { x1: X(CASES[i]), y1: Y(CASES[i]), x2: X(CASES[j]), y2: Y(CASES[j]), stroke: same ? color.get(i) : "#c3cad4",
      "stroke-width": same ? 1.6 : 1, opacity: same ? .75 : .5, "stroke-dasharray": same ? "" : "4 3" }));
  }
  CASES.forEach((c, i) => {
    const node = el("circle", { cx: X(c), cy: Y(c), r: color.has(i) ? 6 : 3.5, fill: color.get(i) || "#c3cad4", stroke: c.series >= 0 ? "#182534" : "none", "stroke-width": 1.2 });
    node.append(el("title", {}, `${c.id}${c.series >= 0 ? " (planted series " + c.series + ")" : " (background)"} · day ${c.day.toFixed(0)}`));
    svg.append(node);
  });
  const leg = $("c-legend"); leg.replaceChildren();
  done.forEach(([g, t], k) => { const s = document.createElement("span");
    const pure = new Set(g.map((i) => CASES[i].series)); s.innerHTML = `<i style="background:${PALETTE[k % PALETTE.length]}"></i>S-${k + 1}: ${g.length} cases at ≥ ${t.toFixed(2)}${pure.size === 1 && !pure.has(-1) ? " · one planted series" : ""}`; leg.append(s); });
  const note = document.createElement("span"); note.textContent = `Dashed grey: edges cut by a re-split. Black ring: planted series member. ${dropped ? dropped + " cases dropped as unsplittable." : ""}`; leg.append(note);
}
["c-thr", "c-split", "c-max"].forEach((id) => $(id).addEventListener("input", renderClusters));

// ---- 5. top-k heap ------------------------------------------------------------------------------
const TK = { target: CASES[0], cands: CASES.slice(1), i: 0, heap: [], skipped: 0, playing: false };
function tkReset() { TK.i = 0; TK.heap = []; TK.skipped = 0; $("tk-log").replaceChildren(); $("v-k").textContent = $("tk-k").value; tkDraw(); }
function tkLog(cls, text) { const d = document.createElement("div"); d.className = cls; d.textContent = text; const l = $("tk-log"); l.prepend(d); }
function tkStep() {
  const k = +$("tk-k").value;
  if (TK.i >= TK.cands.length) { TK.playing = false; $("tk-play").textContent = "Play"; return; }
  const c = TK.cands[TK.i++], bound = pairBound(TK.target, c);
  TK.heap.sort((a, b) => a.s - b.s);
  if (TK.heap.length === k && bound < TK.heap[0].s) { TK.skipped++; tkLog("skip", `${c.id}: bound ${bound.toFixed(3)} < k-th best ${TK.heap[0].s.toFixed(3)} → skipped, no MO work`); }
  else {
    const s = pairScore(TK.target, c);
    if (TK.heap.length < k) { TK.heap.push({ id: c.id, s }); tkLog("push", `${c.id}: score ${s.toFixed(3)} → pushed (heap filling)`); }
    else if (s > TK.heap[0].s) { const out = TK.heap.shift(); TK.heap.push({ id: c.id, s }); tkLog("push", `${c.id}: score ${s.toFixed(3)} → replaces ${out.id} (${out.s.toFixed(3)})`); }
    else tkLog("drop", `${c.id}: bound ${bound.toFixed(3)} passed, exact score ${s.toFixed(3)} too low`);
  }
  tkDraw();
}
function tkDraw() {
  TK.heap.sort((a, b) => a.s - b.s);
  $("tk-heap").replaceChildren(...TK.heap.map((h) => { const d = document.createElement("div"); d.textContent = `${h.id} · ${h.s.toFixed(3)}`; return d; }));
  $("tk-scan").textContent = `${TK.i} / ${TK.cands.length}`; $("tk-skip").textContent = `${TK.skipped} (${TK.i ? Math.round(100 * TK.skipped / TK.i) : 0}%)`;
}
function tkLoop() { if (!TK.playing) return; tkStep(); setTimeout(tkLoop, 70); }
$("tk-play").onclick = () => { if (TK.i >= TK.cands.length) tkReset(); TK.playing = !TK.playing; $("tk-play").textContent = TK.playing ? "Pause" : "Play"; tkLoop(); };
$("tk-step").onclick = tkStep; $("tk-reset").onclick = () => { TK.playing = false; $("tk-play").textContent = "Play"; tkReset(); };
$("tk-k").addEventListener("input", tkReset);

// ---- 6. pawn index -----------------------------------------------------------------------------
const PAWN = (() => { const r = rng(3), cats = ["laptop", "phone", "jewelry", "camera", "bicycle", "tools"];
  const serials = ["C02X1", "SN0123", "SNO123", "AB-123", null, null, null];
  const marks = ["sticker fox", "engraved J", null, null, null];
  return Array.from({ length: 30 }, (_, i) => ({ id: `P-${100 + i}`, cat: cats[Math.floor(r() * 6)], serial: serials[Math.floor(r() * 7)], mark: marks[Math.floor(r() * 5)] })); })();
const ITEMS = [{ id: "I-1 laptop, serial SN0123", cat: "laptop", serial: "SN0123", mark: null },
  { id: "I-2 jewelry, marking 'engraved J'", cat: "jewelry", serial: null, mark: "engraved J" },
  { id: "I-3 camera, serial AB 123", cat: "camera", serial: "AB123", mark: "sticker fox" }];
const PW = { item: 0, mode: "index" };
const norm = (s) => s && s.replace(/[^a-z0-9]/gi, "").toUpperCase();
function pwMatch(it, p) {
  const a = norm(it.serial), b = norm(p.serial), same = it.cat === p.cat;
  if (a && b) return a === b ? "exact serial lead" : same ? "conflicting serial (rejected)" : null;
  if (it.mark && p.mark && it.mark.toLowerCase() === p.mark.toLowerCase()) return "shared marking (review)";
  return same ? "same category (description check)" : null;
}
function renderPawn() {
  const it = ITEMS[PW.item]; $("pw-item").textContent = it.id;
  $("pw-brute").classList.toggle("on", PW.mode === "brute"); $("pw-index").classList.toggle("on", PW.mode === "index");
  let cmp = 0, hit = 0;
  $("pw-cards").replaceChildren(...PAWN.map((p) => {
    const inBucket = p.cat === it.cat || (norm(p.serial) && norm(p.serial) === norm(it.serial)) || (p.mark && it.mark && p.mark.toLowerCase() === it.mark.toLowerCase());
    const compared = PW.mode === "brute" || inBucket, m = compared ? pwMatch(it, p) : null;
    if (compared) cmp++; if (m) hit++;
    const d = document.createElement("div"); d.className = "card " + (m ? "hit" : compared ? "cand" : "off");
    d.innerHTML = `<b>${p.id}</b> · ${p.cat}<br><span class="mono">${p.serial || "no serial"}${p.mark ? " · " + p.mark : ""}</span>${m ? `<br>${m}` : ""}`;
    return d;
  }));
  $("pw-cmp").textContent = `${cmp} / ${PAWN.length}`; $("pw-hit").textContent = hit;
}
$("pw-brute").onclick = () => { PW.mode = "brute"; renderPawn(); };
$("pw-index").onclick = () => { PW.mode = "index"; renderPawn(); };
$("pw-next").onclick = () => { PW.item = (PW.item + 1) % ITEMS.length; renderPawn(); };

// ---- 7. benchmark bars (log scale) ----------------------------------------------------------------
function renderBench() {
  const svg = $("bench-svg"); svg.replaceChildren();
  if (!BENCH.length) { svg.append(el("text", { x: 20, y: 40, fill: "#657080" }, "Run casefile bench to populate.")); return; }
  const lo = Math.log10(5), hi = Math.log10(Math.max(...BENCH.map((b) => b.ref)) * 1.3), X = (ms) => 260 + ((Math.log10(ms) - lo) / (hi - lo)) * 800;
  [10, 100, 1000, 10000, 100000].forEach((t) => { if (Math.log10(t) > hi) return; svg.append(el("line", { x1: X(t), x2: X(t), y1: 10, y2: 300, stroke: "#e3e6eb" }));
    svg.append(el("text", { x: X(t) - 14, y: 320, "font-size": 11, fill: "#657080" }, t >= 1000 ? `${t / 1000}s` : `${t}ms`)); });
  BENCH.forEach((b, i) => {
    const y = 14 + i * 32;
    svg.append(el("text", { x: 8, y: y + 17, "font-size": 12, fill: "#202936" }, `${b.step} · ${fmt(b.cases)} cases`));
    for (const [v, fill, dy] of [[b.ref, "#c3cad4", 0], [b.opt, "#245ab5", 13]]) {
      const r = el("rect", { x: 260, y: y + dy, height: 11, rx: 2, fill, width: 0 }); svg.append(r);
      requestAnimationFrame(() => { r.style.transition = `width 1.1s ${i * 60}ms cubic-bezier(.2,.8,.2,1)`; r.setAttribute("width", Math.max(2, X(v) - 260)); });
    }
    svg.append(el("text", { x: Math.max(X(b.ref), X(b.opt)) + 8, y: y + 17, "font-size": 12, "font-weight": 600, fill: "#276345" }, `${(b.ref / b.opt).toFixed(1)}×`));
  });
}

renderScore(); swDraw(); renderBound(); renderClusters(); tkReset(); renderPawn(); renderBench();
