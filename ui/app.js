"use strict";
// Casefile console. Vanilla JS, no network beyond the local /api. All data is fictional.
// Text is always inserted with textContent (never innerHTML), so record content cannot inject markup.

const ACTOR = "det.demo";
const REASONS = ["Not lawful", "Not available", "Too resource-intensive", "Other"];
const $ = (id) => document.getElementById(id);

// Stored audit sequences remain zero-based; display ordinals start at one.
function auditNumber(seq) { return Number.isSafeInteger(seq) && seq >= 0 ? seq + 1 : "unknown"; }

function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === false || v == null) continue;
    if (k === "class") el.className = v;
    else if (k === "text") el.textContent = v;
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const kid of kids.flat()) {
    if (kid == null || kid === false) continue;
    el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
  }
  return el;
}
function svg(tag, attrs, ...kids) {
  const el = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [k, v] of Object.entries(attrs || {})) el.setAttribute(k, v);
  for (const kid of kids.flat()) if (kid != null) el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
  return el;
}
async function api(path, opts) {
  const r = await fetch(path, opts);
  const body = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(body.error || `HTTP ${r.status}`);
  return body;
}
const fmtTime = (s) => (s == null ? "" : new Date(s * 1000).toISOString().slice(5, 16).replace("T", " ") + "Z");
const pct = (x) => (x * 100).toFixed(1) + "%";
const shortId = (id) => id.replace(/^CC-/, "");
const human = (s) => s.toLowerCase().replace(/\b\w/g, (c) => c.toUpperCase()).replace(/xx/gi, "XX")
  .replace(/\b(Nw|Ne|Sw|Se|Slu|Id)\b/g, (w) => w.toUpperCase());
const prov = (kind, label) => h("span", { class: "prov " + kind }, label);
const BAND = { strong: "pass", moderate: "weak", weak: "fail" };
const bandLabel = (b) => (b ? b[0].toUpperCase() + b.slice(1) : "");
function meter(score, band) {
  return h("span", { class: "meter " + (BAND[band] || ""), role: "img", "aria-label": `Solvability ${Math.round(score)} of 100, ${band}` },
    h("i", { style: `width:${Math.max(2, Math.min(100, score)).toFixed(0)}%` }));
}
function toast(msg) {
  const t = $("toast");
  t.textContent = msg;
  t.hidden = false;
  clearTimeout(toast.t);
  toast.t = setTimeout(() => (t.hidden = true), 2600);
}

function fill(el, ...kids) {
  el.replaceChildren(...kids.flat(2).filter((k) => k != null && k !== false).map((k) => (k.nodeType ? k : document.createTextNode(String(k)))));
}

const S = { queue: null, current: null, series: null, kpi: null, team: null, seriesPick: null, decided: {} };

// ---------------------------------------------------------------- KPI strip
function renderKpi() {
  const box = $("kpi");
  box.replaceChildren();
  const k = S.kpi;
  if (!k) return;
  const tgtPct = k.baseline.cost_target / k.baseline.annual_labor;
  k.results.forEach((r) => {
    const base = r.scenario === "Base";
    box.append(
      h("div", { class: "tile" + (base ? " base" : ""), title: k.note },
        h("b", {}, r.scenario),
        h("div", { class: "row" }, "net ", h("span", { class: "v" }, pct(r.steady_net_pct)), ` / ${pct(tgtPct)} target `,
          h("span", { class: "chip " + (r.meets_cost_target ? "pass" : "fail") }, r.meets_cost_target ? "met" : "missed")),
        h("div", { class: "row" }, "+", h("span", { class: "v" }, Math.round(r.additional_clearances)), ` clearances / +${k.baseline.clearance_target_additional} target `,
          h("span", { class: "chip " + (r.meets_clearance_target ? "pass" : "fail") }, r.meets_clearance_target ? "met" : "missed")))
    );
  });
}

// ---------------------------------------------------------------- queue
function renderQueue() {
  const q = S.queue;
  const query = $("queue-search").value.trim().toLowerCase();
  const cases = q.cases.filter((c) => `${c.id} ${c.neighborhood} ${c.block}`.toLowerCase().includes(query));
  $("queue-count").textContent = query ? `${cases.length}/${q.cases.length}` : q.cases.length;
  $("queue-empty").hidden = cases.length > 0;
  const ul = $("queue-list");
  ul.replaceChildren(
    ...cases.map((c) => {
      const sv = c.solvability;
      const btn = h("button", { type: "button", class: "qbtn", "data-id": c.id, "aria-current": String(S.current?.case.id === c.id), onclick: () => selectCase(c.id) },
        h("span", { class: "l1" }, h("span", { class: "qid" }, c.id),
          sv ? h("span", { class: "qscore", title: `Solvability ${sv.score.toFixed(0)}/100 · ${sv.band}` }, sv.score.toFixed(0)) : null),
        sv ? meter(sv.score, sv.band) : null,
        h("span", { class: "l2" }, human(c.block)),
        h("span", { class: "l3" }, human(c.neighborhood)),
        (c.flags.series_id || c.flags.pawn_hit === "exact_serial_lead") ? h("span", { class: "flags" },
          c.flags.pawn_hit === "exact_serial_lead" ? h("span", { class: "flag lead" }, "Serial lead") : null,
          c.flags.series_id ? h("span", { class: "flag series" }, "Possible series") : null) : null);
      return h("li", {}, btn);
    })
  );
  ul.onkeydown = (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const all = [...ul.querySelectorAll("button")];
    const i = all.indexOf(document.activeElement);
    const n = all[i + (e.key === "ArrowDown" ? 1 : -1)];
    if (n) { e.preventDefault(); n.focus(); }
  };
}
$("queue-search").oninput = () => { if (S.queue) renderQueue(); };
function markCurrent() {
  document.querySelectorAll(".qbtn").forEach((b) => b.setAttribute("aria-current", String(b.dataset.id === S.current?.case.id)));
}

// ---------------------------------------------------------------- case card
function bars(contribs) {
  const max = Math.max(1, ...contribs.map((c) => Math.abs(c[1])));
  const kids = [];
  for (const [name, v] of contribs) {
    const w = (Math.abs(v) / max) * 50;
    const fill = h("i", { class: v >= 0 ? "pos" : "neg", style: v >= 0 ? `left:50%;width:${w}%` : `left:${50 - w}%;width:${w}%` });
    kids.push(h("span", {}, name.replace(/_/g, " ")), h("span", { class: "bar", role: "img", "aria-label": `${name} ${v.toFixed(1)} points` }, h("span", { class: "zero" }), fill),
      h("span", { class: "num" }, (v > 0 ? "+" : "") + v.toFixed(1)));
  }
  return h("div", { class: "bars" }, kids);
}
function pawnCard(p) {
  const m = p.match, st = m.status;
  const cls = st === "exact_serial_lead" ? "lead" : st === "rejected_conflicting_serial" ? "trap" : "";
  const title = { exact_serial_lead: ["pass", "Exact serial lead"], rejected_conflicting_serial: ["fail", "Rejected: conflicting serial"],
    review_candidate_marking: ["weak", "Shared marking"], unresolved_similar_description: ["info", "Description only"] }[st];
  return h("div", { class: "card " + cls },
    h("div", { class: "row-flex" }, h("span", { class: "chip " + title[0] }, title[1]), h("span", { class: "mono" }, m.pawn_id),
      h("span", { class: "kv" }, `+${m.days_after_offense.toFixed(0)} ${Math.round(m.days_after_offense) === 1 ? "day" : "days"}`)),
    h("div", {}, p.item?.description || m.item_id),
    h("div", { class: "serials" },
      h("div", {}, h("span", { class: "kv" }, "Reported serial"), h("span", { class: "mono" }, p.item?.serial || "Not recorded")),
      h("div", {}, h("span", { class: "kv" }, "Pawn serial"), h("span", { class: "mono" }, p.pawn?.serial || "Not recorded"))),
    p.pawn ? h("div", { class: "kv" }, p.pawn.dealer) : null,
    h("details", { class: "step-details" }, h("summary", {}, "Match details"), h("p", {}, m.reason),
      st === "exact_serial_lead" ? h("p", {}, "Ownership unverified. Seller is not presumed to be the offender.") : null));
}
function renderCase() {
  const d = S.current, c = d.case, s = d.solvability;
  const box = $("case-card");
  const lead = d.pawn.find((p) => p.match.status === "exact_serial_lead");
  const winH = c.window_end != null && c.window_start != null ? (c.window_end - c.window_start) / 3600 : null;
  fill($("case-heading"),
    h("div", { class: "heading-main" },
      h("div", { class: "breadcrumb" }, "Inbox / Burglary"),
      h("h2", {}, c.id, h("span", { class: "case-status" }, human(c.status.kind))),
      h("div", { class: "address" }, `${human(c.block)} · ${human(c.neighborhood)} · Beat ${c.beat}`)),
    h("dl", { class: "summary" },
      h("div", { class: "sum score " + (BAND[s.band] || "") },
        h("dt", {}, "Solvability"),
        h("dd", {}, h("b", {}, s.score.toFixed(0)), h("span", { class: "of" }, "/100 "), h("span", { class: "band" }, bandLabel(s.band)))),
      h("div", { class: "sum" + (lead ? " good" : "") },
        h("dt", {}, "Property"),
        h("dd", {}, lead ? h("span", { class: "mono" }, lead.match.pawn_id) : "No serial lead")),
      h("div", { class: "sum" + (d.series_id ? " good" : "") },
        h("dt", {}, "Series"),
        h("dd", {}, d.series_id || "Not linked")),
      winH != null ? h("div", { class: "sum" },
        h("dt", {}, "Offense window"),
        h("dd", {}, `${winH.toFixed(0)} h`)) : null));
  fill(box, 
    h("section", { class: "case-section" }, h("div", { class: "section-head" }, h("h2", {}, "Report"), prov("src", "Source record")),
      h("p", { class: "narr" }, c.narrative),
      h("p", { class: "report-time" }, `Offense window · ${fmtTime(c.window_start)} – ${fmtTime(c.window_end)}`)),
    h("section", { class: "case-section" }, h("div", { class: "section-head" }, h("h2", {}, "Property"), prov("rule", "Rules engine")),
      d.pawn.filter((p) => p.match.status !== "rejected_conflicting_serial").map(pawnCard),
      d.pawn.some((p) => p.match.status !== "rejected_conflicting_serial") ? null : h("p", { class: "empty" }, "No pawn leads."),
      d.pawn.some((p) => p.match.status === "rejected_conflicting_serial") ?
        h("details", { class: "disclosure" }, h("summary", {}, `${d.pawn.filter((p) => p.match.status === "rejected_conflicting_serial").length} excluded records`),
          d.pawn.filter((p) => p.match.status === "rejected_conflicting_serial").map(pawnCard)) : null),
    h("section", { class: "case-section" }, h("h2", {}, "Related cases"),
    d.similar.length ? d.similar.map((x) => {
      const o = x.other, p = x.pair;
      return h("div", { class: "linked" },
        h("span", {}, h("button", { type: "button", class: "linkbtn", onclick: () => selectCase(o.id) }, o.id), h("span", { class: "kv" }, human(o.neighborhood))),
        h("span", { class: "kv", title: `Similarity ${p.score.toFixed(2)}; ${p.components.shared_signatures} shared signatures` }, `${p.components.distance_km.toFixed(1)} km · ${p.components.days_apart.toFixed(0)} d`));
    }) : h("p", { class: "empty" }, "No related cases.")),
    h("details", { class: "disclosure" }, h("summary", {}, "Review factors"),
      h("div", { class: "score-line" }, h("b", {}, s.score.toFixed(0) + " / 100"), h("span", { class: "kv" }, "Solvability score")), bars(s.contributions),
      h("p", { class: "kv" }, `Reported ${fmtTime(c.reported_at)} · Extraction ${d.recorded.extractor ? "recorded" : "unavailable"}`))
  );
}

// ---------------------------------------------------------------- right: packet + steps
function citeChips(text, d) {
  const re = /\[([A-Z]{2}-\d{2}-\d{6}|facts|[A-Za-z0-9-]+):([a-z_.]+)(?:=[^\]]*)?\]/g;
  const out = [];
  let last = 0, m;
  while ((m = re.exec(text))) {
    out.push(text.slice(last, m.index));
    let val = "see record";
    if (m[1] === d.case.id) {
      const v = m[2].split(".").reduce((o, k) => (o == null ? o : o[k]), d.case);
      if (v != null) val = typeof v === "object" ? JSON.stringify(v) : String(v);
    }
    out.push(h("span", { class: "chip cite", tabindex: "0", title: `${m[1]}.${m[2]} = ${val}` }, m[0]));
    last = m.index + m[0].length;
  }
  out.push(text.slice(last));
  return out;
}
function missing(label) {
  return h("p", { class: "ai-missing" }, `No recorded ${label.replace(/^the /, "")}.`);
}
async function decide(action, subject, detail) {
  const r = await api("/api/decision", { method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ actor: ACTOR, action, subject, detail }) });
  await loadAudit().catch(() => toast("Decision saved. Activity could not refresh."));
  toast(`Decision saved · #${auditNumber(r.entry.seq)}`);
  return r;
}
function stepItem(st, i, d) {
  const key = `${d.case.id}#${i}`;
  const done = S.decided[key];
  const li = h("li", { class: "step " + (done ? done.kind : "") });
  const body = h("div", {}, h("div", { class: "step-title" }, st.step),
    h("div", { class: "step-meta" }, st.source === "engine" ? prov("rule", "Rules engine") : prov("ai", "AI suggested · " + st.source), h("span", {}, `~${st.est_minutes} min`)),
    h("details", { class: "step-details" }, h("summary", {}, "Why this step"),
      h("p", {}, st.why), h("p", {}, st.lawful_basis_note), h("p", {}, "Source: " + st.source)));
  li.append(body);
  const btns = h("div", { class: "btns" });
  if (done) {
    btns.append(h("span", { class: "chip " + (done.kind === "accepted" ? "pass" : "fail") }, done.kind === "accepted" ? "Accepted" : "Rejected: " + done.reason));
  } else {
    const sel = h("select", { "aria-label": "Reject reason", hidden: true }, REASONS.map((r) => h("option", { value: r }, r)));
    const note = h("input", { type: "text", "aria-label": "Reject note", placeholder: "note (optional)", hidden: true, size: "14" });
    const confirm = h("button", { type: "button", class: "btn sm danger", hidden: true }, "Confirm reject");
    const accept = h("button", { type: "button", class: "btn sm primary" }, "Accept");
    const reject = h("button", { type: "button", class: "btn sm" }, "Reject");
    accept.onclick = async () => {
      accept.disabled = reject.disabled = true;
      try {
        await decide("accept_step", d.case.id, { step: st.step, source: st.source });
        S.decided[key] = { kind: "accepted" }; renderPacket();
      } catch (e) { toast(e.message); accept.disabled = reject.disabled = false; }
    };
    reject.onclick = () => { sel.hidden = note.hidden = confirm.hidden = false; reject.hidden = true; sel.focus(); };
    confirm.onclick = async () => {
      confirm.disabled = accept.disabled = true;
      try {
        await decide("reject_step", d.case.id, { step: st.step, reason: sel.value, note: note.value.slice(0, 200), source: st.source });
        S.decided[key] = { kind: "rejected", reason: sel.value }; renderPacket();
      } catch (e) { toast(e.message); confirm.disabled = accept.disabled = false; }
    };
    btns.append(accept, reject, sel, note, confirm);
  }
  li.append(btns);
  return li;
}
function renderPacket() {
  const d = S.current, pane = $("pane-packet");
  const pk = d.recorded.packet_writer, tr = d.recorded.triage;
  fill(pane, 
    h("div", { class: "section-head" }, h("h2", {}, "Suggested actions"), h("span", {}, `${d.next_steps.length} actions · you decide each one`)),
    tr ? null : h("p", { class: "ai-missing" }, "From case rules · No recorded triage"),
    h("ul", { class: "steps" }, d.next_steps.map((s, i) => stepItem(s, i, d))),
    h("details", { class: "disclosure" }, h("summary", {}, "Lead packet"),
    pk ? [h("div", { class: "row-flex" }, pk.output.citations_ok ? null : h("span", { class: "chip weak" }, "Citations need review"),
        h("span", { class: "kv" }, `${pk.model} · $${pk.cost_usd.toFixed(3)} · ${(pk.duration_ms / 1000).toFixed(0)}s`),
        h("button", { type: "button", class: "btn sm", onclick: () => replay("packet_writer", d.case.id) }, "Replay")),
      h("div", { class: "packet-text" }, citeChips(pk.output.packet_markdown || "", d))]
      : missing("the lead packet"),
    h("div", { id: "replay-box" })),
    tr ? h("details", { class: "disclosure" }, h("summary", {}, "Triage rationale"), h("p", { class: "kv" }, tr.output.rationale_with_citations)) : null
  );
}
async function replay(agent, id) {
  const box = $("replay-box");
  try {
    const t = await api(`/api/replay/${agent}/${id}?speed=2`);
    box.replaceChildren(h("div", { class: "ai-missing", "aria-live": "polite" }, h("b", {}, `Replaying ${agent} (${t.model})`)));
    const wrap = box.firstChild;
    t.events.forEach((e) => setTimeout(() => wrap.append(h("div", { class: "kv" },
      e.kind === "tool_call" ? `call ${e.name} ${e.input}` : e.kind === "tool_result" ? `result ${e.text}` : e.text)), e.at_ms));
  } catch (e) { box.replaceChildren(missing(agent + " replay")); }
}

// ---------------------------------------------------------------- right: series
function pickSeries() {
  const list = S.series?.series || [];
  const id = S.current?.case.id;
  return list.find((s) => s.id === S.seriesPick) || list.find((s) => s.in_queue.includes(id)) || list[0];
}
function mapSvg(ser) {
  const W = 440, H = 190, P = 22;
  const pts = ser.cases.map((c) => ({ id: c.id, x: c.location.lon * Math.cos((c.location.lat * Math.PI) / 180), y: -c.location.lat, c }));
  const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
  const x0 = Math.min(...xs), x1 = Math.max(...xs), y0 = Math.min(...ys), y1 = Math.max(...ys);
  const sc = Math.min((W - 2 * P) / Math.max(1e-9, x1 - x0), (H - 2 * P) / Math.max(1e-9, y1 - y0));
  const px = (p) => P + (p.x - x0) * sc + (W - 2 * P - (x1 - x0) * sc) / 2;
  const py = (p) => P + (p.y - y0) * sc + (H - 2 * P - (y1 - y0) * sc) / 2;
  const off = new Set(ser.verify.offending_cases);
  const sorted = [...pts].sort((a, b) => a.c.window_start - b.c.window_start);
  const el = svg("svg", { viewBox: `0 0 ${W} ${H}`, role: "img", "aria-label": `Map of ${pts.length} cases in ${ser.id}, no map tiles` });
  el.append(svg("polyline", { points: sorted.map((p) => `${px(p)},${py(p)}`).join(" "), fill: "none", stroke: "#7b8696", "stroke-dasharray": "3 3" }));
  for (const p of pts) {
    const q = ser.in_queue.includes(p.id), bad = off.has(p.id), cl = ser.cleared?.case_id === p.id;
    const g = svg("g", {}, svg("title", {}, `${p.id} ${p.c.block}`),
      svg("circle", { cx: px(p), cy: py(p), r: q ? 6 : 4.5, fill: bad ? "#9c1c1c" : q ? "#1d4f91" : "#6b7f99", stroke: cl ? "#155c33" : "#fff", "stroke-width": cl ? 3 : 1.5 }),
      svg("text", { x: px(p) + (px(p) > W - 70 ? -8 : 8), y: py(p) + 3, "text-anchor": px(p) > W - 70 ? "end" : "start" }, shortId(p.id)));
    el.append(g);
  }
  return el;
}
function timelineSvg(ser) {
  const W = 440, H = 52, P = 16;
  const cs = ser.cases;
  const t0 = Math.min(...cs.map((c) => c.window_start)), t1 = Math.max(...cs.map((c) => c.window_end));
  const x = (t) => P + ((t - t0) / Math.max(1, t1 - t0)) * (W - 2 * P);
  const el = svg("svg", { viewBox: `0 0 ${W} ${H}`, role: "img", "aria-label": "Timeline of offense windows" });
  el.append(svg("line", { x1: P, x2: W - P, y1: 26, y2: 26, stroke: "#7b8696" }));
  const sorted = [...cs].sort((a, b) => a.window_start - b.window_start);
  sorted.forEach((c, i) => {
    const w = Math.max(3, x(c.window_end) - x(c.window_start));
    el.append(svg("rect", { x: x(c.window_start), y: 20, width: w, height: 12, fill: ser.in_queue.includes(c.id) ? "#1d4f91" : "#6b7f99" }, svg("title", {}, `${c.id} ${fmtTime(c.window_start)}`)));
    if (ser.in_queue.includes(c.id) || ser.cleared?.case_id === c.id)
      el.append(svg("text", { x: Math.min(Math.max(x(c.window_start), P), W - P), y: 14, "text-anchor": x(c.window_start) > W - 80 ? "end" : "start" }, shortId(c.id).slice(-4) + (ser.cleared?.case_id === c.id ? " cleared" : " queue")));
  });
  el.append(svg("text", { x: P, y: H - 3 }, fmtTime(t0)), svg("text", { x: W - P, y: H - 3, "text-anchor": "end" }, fmtTime(t1)));
  return el;
}
function renderSeries() {
  const pane = $("pane-series");
  const list = S.series?.series || [];
  if (!list.length) { pane.replaceChildren(h("p", { class: "empty" }, "No series found for the queue.")); return; }
  const ser = pickSeries();
  const v = ser.verify;
  const sel = h("select", { "aria-label": "Series", onchange: (e) => { S.seriesPick = e.target.value; renderSeries(); } },
    list.map((s) => h("option", { value: s.id, selected: s.id === ser.id }, `${s.id} · ${s.case_ids.length} cases · ${s.verify.verdict}`)));
  const sk = ser.skeptic?.output?.verdicts?.[0];
  const key = "series:" + ser.id;
  const done = S.decided[key];
  const approve = h("button", { type: "button", class: "btn primary", disabled: v.verdict === "fail" || !!done }, done ? "Series alert approved" : "Approve series alert");
  approve.onclick = async () => {
    approve.disabled = true;
    try {
      await decide("approve_series_alert", ser.id, { case_ids: ser.case_ids, verdict: v.verdict });
      S.decided[key] = { kind: "accepted" }; renderSeries();
    } catch (e) { toast(e.message); approve.disabled = false; }
  };
  fill(pane, 
    h("div", { class: "section-head" }, sel, prov("rule", "Rules engine")),
    h("div", { class: "kv" }, ser.rationale),
    ser.cleared ? h("div", { class: "kv" }, `Cleared by arrest: ${ser.cleared.case_id} · ${ser.cleared.arrestee_ref}`) : null,
    mapSvg(ser), h("p", { class: "map-key" }, "Blue: inbox · Red: conflict · Green ring: cleared"), timelineSvg(ser),
    h("h3", {}, "Link checks"),
    h("ul", { class: "checks" }, v.checks.map((c) => h("li", {}, h("span", { class: "chip " + (c.verdict === "pass" ? "pass" : c.verdict === "weak" ? "weak" : "fail") }, (c.verdict === "pass" ? "PASS " : c.verdict === "weak" ? "WEAK " : "FAIL ") + c.name.replace(/_/g, " ")), h("span", { class: "kv" }, c.reason)))),
    h("details", { class: "disclosure" }, h("summary", {}, "Independent review (AI skeptic)"),
      ser.skeptic && sk ? h("p", { class: "kv" }, sk.reasons_with_citations) : missing("the independent review")),
    h("div", { class: "row-flex" }, approve),
    v.verdict === "fail" ? h("p", { class: "kv" }, "Approval blocked by failed link checks.") : null
  );
}

// ---------------------------------------------------------------- right: victim
function renderVictim() {
  const d = S.current, pane = $("pane-victim");
  const rec = d.recorded.victim_liaison;
  const msg = rec ? rec.output.message : d.victim_template.message;
  const lang = rec ? rec.output.language : "es";
  const key = "victim:" + d.case.id;
  const done = S.decided[key];
  const btn = h("button", { type: "button", class: "btn primary", disabled: !!done }, done ? "Approved" : "Approve update");
  btn.onclick = async () => {
    btn.disabled = true;
    try {
      await decide("approve_victim_update", d.case.id, { language: lang, source: rec ? "victim_liaison" : "engine_template" });
      S.decided[key] = { kind: "accepted" }; renderVictim();
    } catch (e) { toast(e.message); btn.disabled = false; }
  };
  fill(pane, 
    h("div", { class: "section-head" }, h("h2", {}, "Victim update"), rec ? prov("ai", `AI draft · ${lang.toUpperCase()}`) : prov("rule", `Template · ${lang.toUpperCase()}`)),
    rec ? null : h("p", { class: "kv" }, "Standard template · No recorded draft"),
    h("div", { class: "packet-text draft", lang: lang }, msg),
    rec?.output.plain_english_summary ? h("div", { class: "kv" }, "English summary: " + rec.output.plain_english_summary) : null,
    h("div", { class: "kv" }, "Approval is recorded. Delivery is not connected."),
    h("div", { class: "row-flex" }, btn)
  );
}

// ---------------------------------------------------------------- drawers
async function loadAudit() {
  const a = await api("/api/audit");
  const ok = a.chain.intact;
  const chip = $("audit-chip");
  chip.className = "activity-count";
  chip.textContent = ok ? a.chain.entries : "!";
  chip.title = ok ? "Audit chain intact" : `Audit chain broken at #${auditNumber(a.chain.first_bad_seq)}`;
  $("audit-status").textContent = ok ? `Chain intact ✓ (${a.chain.entries} entries)` : `Chain broken at entry #${auditNumber(a.chain.first_bad_seq)}`;
  $("audit-status").className = "status " + (ok ? "ok" : "bad");
  $("audit-list").replaceChildren(...[...a.entries].reverse().map((e) => h("li", {},
    h("b", {}, e.action), ` on ${e.subject} by ${e.actor}`, h("div", { class: "kv" }, `#${auditNumber(e.seq)} · ${fmtTime(Number(e.ts))} · `, h("span", { class: "mono" }, e.hash.slice(0, 12)), " · ", JSON.stringify(e.detail)))));
  if (!a.entries.length) $("audit-list").replaceChildren(h("li", { class: "empty" }, "No decisions yet."));
}
function renderTeam() {
  const t = S.team;
  const tools = t.tools.map((x) => x.tool);
  const head = h("tr", {}, h("th", {}, "Agent (model)"), tools.map((x) => h("th", { class: "rot" }, h("span", {}, x))), t.channels.map((c) => h("th", { class: "rot" }, h("span", {}, c.label + " (draft)"))));
  const rows = t.agents.map((a) => h("tr", {}, h("td", {}, h("b", {}, a.name), ` (${a.model})`),
    tools.map((x) => (a.tools.includes(x) ? h("td", { class: "on", title: `${a.name} calls ${x}` }, "●") : h("td", {}, ""))),
    t.channels.map((c) => (c.drafted_by.includes(a.name) ? h("td", { class: "gate", title: "Draft needs human approval" }, "◐") : h("td", {}, "")))));
  fill($("team-body"), 
    h("h3", {}, "Agents & connectors"),
    h("p", { class: "kv" }, "● Tool access · ◐ Draft channel"),
    h("table", {}, h("thead", {}, head), h("tbody", {}, rows)),
    t.channels.map((c) => h("p", {}, h("b", {}, c.label), ` via ${c.via}: ${c.note}`)),
    h("p", { class: "kv" }, "series_llm_only is the evaluation baseline.")
  );
}
function openDrawer(id) {
  for (const d of document.querySelectorAll(".drawer")) d.hidden = d.id !== id ? true : !d.hidden;
  const d = $(id);
  if (!d.hidden) d.querySelector("[data-close]").focus();
}
document.querySelectorAll("[data-close]").forEach((b) => (b.onclick = () => { b.closest(".drawer").hidden = true; }));
$("btn-audit").onclick = async () => { await loadAudit().catch(() => {}); openDrawer("drawer-audit"); };
$("btn-team").onclick = () => openDrawer("drawer-team");
document.addEventListener("keydown", (e) => { if (e.key === "Escape") document.querySelectorAll(".drawer").forEach((d) => (d.hidden = true)); });

// ---------------------------------------------------------------- tabs
const TABS = ["packet", "series", "victim"];
function showTab(name) {
  TABS.forEach((t) => {
    const on = t === name;
    $("tab-" + t).setAttribute("aria-selected", String(on));
    $("tab-" + t).tabIndex = on ? 0 : -1;
    $("pane-" + t).hidden = !on;
  });
}
TABS.forEach((t, i) => {
  $("tab-" + t).onclick = () => showTab(t);
  $("tab-" + t).onkeydown = (e) => {
    const n = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : null;
    if (n == null) return;
    const to = TABS[(n + TABS.length) % TABS.length];
    showTab(to); $("tab-" + to).focus();
  };
});

// ---------------------------------------------------------------- boot
let selection = 0;
async function selectCase(id) {
  const request = ++selection;
  try {
    const data = await api("/api/case/" + encodeURIComponent(id));
    if (request !== selection) return;
    S.current = data;
  } catch (e) { if (request === selection) toast(e.message); return; }
  S.seriesPick = null;
  markCurrent(); renderCase(); renderPacket(); renderSeries(); renderVictim();
  $("case-card").scrollTop = 0;
  document.querySelectorAll(".pane").forEach((p) => { p.scrollTop = 0; });
}
async function main() {
  try {
    const [queue, series, kpi, team] = await Promise.all([api("/api/queue"), api("/api/series"), api("/api/kpi"), api("/api/team")]);
    Object.assign(S, { queue, series, kpi, team });
  } catch (e) {
    $("case-heading").replaceChildren(h("p", { class: "empty" }, "Could not load cases: " + e.message));
    return;
  }
  renderKpi(); renderQueue(); renderTeam();
  await loadAudit().catch(() => {});
  const first = new URLSearchParams(location.search).get("case") || S.queue.cases[0]?.id;
  if (first) await selectCase(first);
  const tab = new URLSearchParams(location.search).get("tab");
  if (TABS.includes(tab)) showTab(tab);
}
main();
