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

const S = { queue: null, current: null, series: null, kpi: null, team: null, seriesPick: null, queueFilter: "all", decided: {} };
const wideInbox = typeof matchMedia === "function" ? matchMedia("(min-width: 1180px)") : null;
wideInbox?.addEventListener("change", e => { $("case-switcher").open = e.matches; });

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
  const matchesFilter = c => S.queueFilter === "pawn" ? c.flags.pawn_hit === "exact_serial_lead" : S.queueFilter === "related" ? !!c.flags.series_id : true;
  const cases = q.cases.filter(c => matchesFilter(c) && `${c.id} ${c.neighborhood} ${c.block}`.toLowerCase().includes(query));
  ["all", "pawn", "related"].forEach(filter => $("queue-filter-" + filter).setAttribute("aria-pressed", String(S.queueFilter === filter)));
  $("queue-count").textContent = query || S.queueFilter !== "all" ? `${cases.length}/${q.cases.length}` : q.cases.length;
  $("queue-empty").hidden = cases.length > 0;
  const ul = $("queue-list");
  ul.replaceChildren(
    ...cases.map((c) => {
      const sv = c.solvability;
      const btn = h("button", { type: "button", class: "qbtn", "data-id": c.id, "aria-current": String(S.current?.case.id === c.id), onclick: () => selectCase(c.id) },
        h("span", { class: "l1" }, h("span", { class: "qid" }, c.id),
          sv ? h("span", { class: "qscore", title: `Solvability ${sv.score.toFixed(0)}/100 · ${sv.band}` }, sv.score.toFixed(0)) : null),
        sv ? h("span", { class: "queue-meter " + sv.band, role: "img", "aria-label": `Solvability ${sv.score.toFixed(0)} out of 100` },
          h("i", { style: `width:${Math.max(0, Math.min(100, sv.score))}%` })) : null,
        h("span", { class: "l2" }, human(c.block)),
        h("span", { class: "l3" }, human(c.neighborhood)));
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
["all", "pawn", "related"].forEach(filter => {
  $("queue-filter-" + filter).onclick = () => { S.queueFilter = filter; if (S.queue) renderQueue(); };
});
function markCurrent() {
  document.querySelectorAll(".qbtn").forEach((b) => b.setAttribute("aria-current", String(b.dataset.id === S.current?.case.id)));
  globalThis.CasefileMotion?.select(S.current?.case.id);
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
    h("div", { class: "row-flex" }, h("span", { class: "mono" }, m.pawn_id),
      h("span", { class: "kv" }, `+${m.days_after_offense.toFixed(0)} ${Math.round(m.days_after_offense) === 1 ? "day" : "days"}`)),
    h("div", {}, p.item?.description || m.item_id),
    h("div", { class: "serials" },
      h("div", {}, h("span", { class: "kv" }, "Reported serial"), h("span", { class: "mono" }, p.item?.serial || "Not recorded")),
      h("div", {}, h("span", { class: "kv" }, "Pawn serial"), h("span", { class: "mono" }, p.pawn?.serial || "Not recorded"))),
    p.pawn ? h("div", { class: "kv" }, p.pawn.dealer) : null,
    h("details", { class: "step-details" }, h("summary", {}, "Match details"), h("p", {}, title[1]), h("p", {}, m.reason),
      st === "exact_serial_lead" ? h("p", {}, h("strong", {}, "Ownership unverified. "), "Seller is not presumed to be the offender.") : null));
}
function readableReport(narrative) {
  const terms = [[/\bRP\b/g, "Reporting person"], [/\bUNK SUSP\(s\)/gi, "Unknown suspect(s)"], [/\bUNK SUSP\b/gi, "Unknown suspect"], [/\bUNK\b/g, "unknown"], [/\bPOE\b/g, "Entry point"], [/\bFD\b/g, "front door"], [/\bRESD\b/g, "Residential"], [/\bSTOR UNIT B&E\b/g, "Storage unit break-in"], [/\brpt'd\b/g, "reported"], [/\bS\/N\b/g, "serial"], [/\bEst\./g, "Estimated"], [/\bSusp\b/g, "Suspect"], [/\bveh\b/g, "vehicle"], [/\bdk\b/g, "dark"]];
  return terms.reduce((text, [pattern, replacement]) => text.replace(pattern, replacement), narrative);
}
function reportOverview(c) {
  return h("div", { class: "report-overview" },
    h("div", { class: "location-preview" },
      h("div", { class: "globe-stage" },
        h("canvas", { class: "globe-canvas", width: "800", height: "800", role: "img", tabindex: "0", "aria-label": "Dotted globe illustration. Drag or use arrow keys to rotate." }),
        h("canvas", { class: "scene-canvas", width: "640", height: "320", "aria-hidden": "true" })),
      h("div", { class: "preview-caption" }, h("span", {}, "Illustration · not evidence"), h("button", { type: "button", class: "globe-replay" }, "Replay zoom ↗"))),
    h("div", { class: "section-head" }, h("h2", {}, "Report")),
    h("p", { class: "narr source-record" }, c.narrative),
    h("p", { class: "narr report-readable" }, readableReport(c.narrative)));
}

function propertyContents(d) {
  const visible = d.pawn.filter(p => p.match.status !== "rejected_conflicting_serial");
  const excluded = d.pawn.filter(p => p.match.status === "rejected_conflicting_serial");
  return [
    h("div", { class: "section-head" }, h("h2", {}, "Property")),
    ...visible.map(pawnCard),
    visible.length ? null : h("p", { class: "empty" }, "No pawn leads."),
    excluded.length ? h("details", { class: "disclosure" }, h("summary", {}, `${excluded.length} excluded records`), excluded.map(pawnCard)) : null,
  ];
}

function renderCase() {
  const d = S.current, c = d.case, s = d.solvability;
  const box = $("case-card");
  const lead = d.pawn.find(p => p.match.status === "exact_serial_lead");
  const hours = c.window_end != null && c.window_start != null ? Math.max(0, (c.window_end - c.window_start) / 3600) : null;
  fill($("case-heading"),
    h("div", { class: "heading-main" },
      h("div", { class: "breadcrumb" }, "Inbox / Burglary"),
      h("h2", {}, c.id),
      h("div", { class: "address" }, `${human(c.block)} · ${human(c.neighborhood)} · Beat ${c.beat}`)),
    h("dl", { class: "case-summary" },
      h("div", { class: "sum" }, h("dt", {}, "Solvability"),
        h("dd", {}, h("b", { class: "summary-score" }, s.score.toFixed(0)), h("span", { class: "of" }, "/100"))),
      h("div", { class: "sum" }, h("dt", {}, "Property"), h("dd", { class: lead ? "mono" : "" }, lead ? lead.match.pawn_id : "No serial lead")),
      h("div", { class: "sum" }, h("dt", {}, "Series"), h("dd", {}, d.series_id || "Not linked")),
      h("div", { class: "sum" }, h("dt", {}, "Offense window"), h("dd", {}, hours == null ? "Not recorded" : `${hours < 1 ? hours.toFixed(1) : hours.toFixed(0)} h`))));
  fill(box, 
    h("section", { class: "case-section", id: "evidence-report", role: "tabpanel", "aria-labelledby": "evidence-tab-report", tabindex: "0" },
      reportOverview(c),
      h("p", { class: "report-time" }, `${fmtTime(c.window_start)} – ${fmtTime(c.window_end)}`),
      h("section", { class: "property-overview", "aria-label": "Property leads" }, propertyContents(d))),
    h("section", { class: "case-section", id: "evidence-property", role: "tabpanel", "aria-labelledby": "evidence-tab-property", tabindex: "0", hidden: true },
      propertyContents(d)),
    h("section", { class: "case-section", id: "evidence-related", role: "tabpanel", "aria-labelledby": "evidence-tab-related", tabindex: "0", hidden: true },
    d.similar.length ? d.similar.map((x) => {
      const o = x.other, p = x.pair;
      return h("div", { class: "linked" },
        h("span", {}, h("button", { type: "button", class: "linkbtn", onclick: () => selectCase(o.id) }, o.id), h("details", { class: "location-details" }, h("summary", {}, "Location"), h("span", { class: "kv" }, human(o.neighborhood)))),
        h("span", { class: "kv", title: `Similarity ${p.score.toFixed(2)}; ${p.components.shared_signatures} shared signatures` }, `${p.components.distance_km.toFixed(1)} km · ${p.components.days_apart.toFixed(0)} d`));
    }) : h("p", { class: "empty" }, "No related cases."))
  );
  fill($("review-factors"),
    h("div", { class: "section-head" }, h("h2", {}, "Review factors"), h("span", { class: "factor-score" }, s.score.toFixed(0) + " / 100")),
    bars(s.contributions));
  showEvidence("report");
}

// ---------------------------------------------------------------- right: actions
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
  const li = h("li", { class: "step " + (done ? done.kind : ""), "data-step": String(i + 1).padStart(2, "0") });
  const body = h("div", { class: "step-title" }, st.step);
  li.append(body);
  const btns = h("div", { class: "btns" });
  if (done) {
    btns.append(h("span", { class: "chip " + (done.kind === "accepted" ? "pass" : "fail") }, done.kind === "accepted" ? "Accepted" : "Rejected: " + done.reason));
  } else {
    let reason = REASONS[0];
    const reasonLabel = h("span", {}, reason);
    const picker = h("details", { class: "reason-picker" });
    const trigger = h("summary", { "aria-label": "Rejection reason" }, reasonLabel, h("span", { "aria-hidden": "true" }, "⌄"));
    const options = REASONS.map(value => h("button", { type: "button", class: "reason-option", "aria-pressed": String(value === reason), onclick: () => {
      reason = value; reasonLabel.textContent = value; picker.open = false;
      options.forEach(button => button.setAttribute("aria-pressed", String(button.textContent === value)));
      trigger.focus();
    } }, value));
    const menu = h("div", { class: "reason-menu", role: "group", "aria-label": "Rejection reasons" }, options);
    picker.append(trigger, menu);
    picker.ontoggle = () => { if (picker.open) globalThis.CasefileMotion?.reveal(menu, -5); };
    menu.onkeydown = e => {
      if (e.key === "Escape") { picker.open = false; trigger.focus(); e.preventDefault(); return; }
      const index = options.indexOf(document.activeElement);
      const next = e.key === "ArrowDown" ? (index + 1) % options.length : e.key === "ArrowUp" ? (index - 1 + options.length) % options.length : e.key === "Home" ? 0 : e.key === "End" ? options.length - 1 : null;
      if (next != null) { e.preventDefault(); options[next].focus(); }
    };
    const note = h("textarea", { "aria-label": "Rejection notes", placeholder: "Add context for the team…", maxlength: "200", rows: "3" });
    const confirm = h("button", { type: "button", class: "btn sm danger" }, "Confirm rejection");
    const accept = h("button", { type: "button", class: "btn sm primary" }, "Accept");
    const reject = h("button", { type: "button", class: "btn sm" }, "Reject");
    const cancel = h("button", { type: "button", class: "btn sm" }, "Cancel");
    const form = h("fieldset", { class: "rejection-form", hidden: true },
      h("legend", {}, "Reject this action"), h("div", { class: "field-label" }, "Reason"), picker,
      h("label", {}, h("span", { class: "field-label" }, "Notes ", h("span", { class: "muted" }, "(optional · 200 characters)")), note),
      h("div", { class: "rejection-controls" }, cancel, confirm));
    cancel.onclick = () => { form.hidden = true; picker.open = false; reject.hidden = false; reject.focus(); };

    accept.onclick = async () => {
      accept.disabled = reject.disabled = form.disabled = true;
      try {
        await decide("accept_step", d.case.id, { step: st.step, source: st.source });
        S.decided[key] = { kind: "accepted" }; renderPacket();
      } catch (e) { toast(e.message); accept.disabled = reject.disabled = form.disabled = false; }
    };
    reject.onclick = () => { form.hidden = false; reject.hidden = true; globalThis.CasefileMotion?.reveal(form); trigger.focus(); };
    confirm.onclick = async () => {
      form.disabled = confirm.disabled = accept.disabled = true;
      try {
        await decide("reject_step", d.case.id, { step: st.step, reason, note: note.value.slice(0, 200), source: st.source });
        S.decided[key] = { kind: "rejected", reason }; renderPacket();
      } catch (e) { toast(e.message); form.disabled = confirm.disabled = accept.disabled = false; }
    };
    btns.append(accept, reject, form);
  }
  li.append(btns);
  return li;
}
function pendingSteps(d) {
  return d.next_steps.map((step, index) => ({ step, index })).filter(({ index }) => !S.decided[`${d.case.id}#${index}`]);
}
function renderPacket() {
  const d = S.current, pane = $("pane-packet");
  const pending = pendingSteps(d);
  const completed = d.next_steps.map((step, index) => ({ step, index })).filter(({ index }) => S.decided[`${d.case.id}#${index}`]);
  fill(pane, 
    h("div", { class: "section-head" }, h("h2", {}, "Suggested actions")),
    pending.length ? h("ul", { class: "steps", "aria-live": "polite" }, pending.map(({ step, index }) => stepItem(step, index, d))) : h("p", { class: "complete-status", role: "status" }, "All actions reviewed"),
    completed.length ? h("details", { class: "more-actions" }, h("summary", {}, "Reviewed"),
      h("ul", { class: "steps" }, completed.map(({ step, index }) => stepItem(step, index, d)))) : null
  );
}

// ---------------------------------------------------------------- right: series
function pickSeries() {
  const list = S.series?.series || [];
  const id = S.current?.case.id;
  return list.find((s) => s.id === S.seriesPick) || list.find((s) => s.case_ids.includes(id));
}
function mapSvg(ser) {
  const W = 440, H = 180, P = 24;
  const pts = ser.cases.map((c) => ({ id: c.id, x: c.location.lon * Math.cos((c.location.lat * Math.PI) / 180), y: -c.location.lat, c }));
  const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
  const x0 = Math.min(...xs), x1 = Math.max(...xs), y0 = Math.min(...ys), y1 = Math.max(...ys);
  const sc = Math.min((W - 2 * P) / Math.max(1e-9, x1 - x0), (H - 2 * P) / Math.max(1e-9, y1 - y0));
  const px = (p) => P + (p.x - x0) * sc + (W - 2 * P - (x1 - x0) * sc) / 2;
  const py = (p) => P + (p.y - y0) * sc + (H - 2 * P - (y1 - y0) * sc) / 2;
  const off = new Set(ser.verify.offending_cases);
  const el = svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "series-map", role: "img", "aria-label": `Schematic locations of ${pts.length} cases in ${ser.id}. Nearby points are grouped. No map tiles.` });
  const groups = [];
  for (const p of pts) {
    const x = px(p), y = py(p);
    const group = groups.find(g => Math.hypot(g.x - x, g.y - y) < 28);
    if (group) group.points.push(p); else groups.push({ x, y, points: [p] });
  }
  for (const g of groups) {
    const bad = g.points.some(p => off.has(p.id));
    const queued = g.points.some(p => ser.in_queue.includes(p.id));
    el.append(svg("g", {}, svg("title", {}, g.points.map(p => p.id).join(", ")),
      svg("circle", { cx: g.x, cy: g.y, r: g.points.length > 1 ? 13 : 8, fill: bad ? "#8b2435" : queued ? "#173b63" : "#6b7d89", stroke: "#fff", "stroke-width": 3 }),
      g.points.length > 1 ? svg("text", { x: g.x, y: g.y + 4, "text-anchor": "middle", style: "fill:white;font-size:12px;font-weight:600" }, g.points.length) : null));
  }
  return el;
}
function renderSeries() {
  const pane = $("pane-series");
  const list = S.series?.series || [];
  if (!list.length) { pane.replaceChildren(h("p", { class: "empty" }, "No linked series.")); return; }
  const ser = pickSeries();
  if (!ser) {
    fill(pane, h("p", { class: "empty" }, "No linked series for this case."),
      h("div", { class: "series-choices" }, list.map(s => h("button", { type: "button", class: "series-choice", onclick: () => { S.seriesPick = s.id; renderSeries(); } },
        h("span", { class: "mono" }, s.id), h("span", {}, human(s.cases[0]?.neighborhood || "Other cases"))))));
    return;
  }
  const v = ser.verify;
  const index = list.indexOf(ser);
  const move = offset => { S.seriesPick = list[(index + offset + list.length) % list.length].id; renderSeries(); };
  const cases = [...ser.cases].sort((a, b) => a.window_start - b.window_start);
  const t0 = Math.min(...cases.map(c => c.window_start)), t1 = Math.max(...cases.map(c => c.window_end));
  const span = Math.max(1, t1 - t0);
  const offending = new Set(v.offending_cases);
  const labels = { spatiotemporal_feasibility: "Time and distance", conflicting_clearance: "Cleared cases", mo_consistency: "Entry pattern", property_consistency: "Stolen items", geographic_spread: "Locations" };
  const key = "series:" + ser.id;
  const done = S.decided[key];
  const approve = h("button", { type: "button", class: "btn primary", disabled: v.verdict === "fail" || !!done }, done ? "Approved" : "Approve series alert");
  approve.onclick = async () => {
    if (v.verdict === "fail" || S.decided[key]) return;
    approve.disabled = true;
    try {
      await decide("approve_series_alert", ser.id, { case_ids: ser.case_ids, verdict: v.verdict });
      S.decided[key] = { kind: "accepted" }; renderSeries();
    } catch (e) { toast(e.message); approve.disabled = false; }
  };
  fill(pane, 
    h("div", { class: "series-heading" }, h("h2", {}, "Connected cases"),
      h("div", { class: "series-nav", "aria-label": "Browse series" },
        list.length > 1 ? h("button", { type: "button", "aria-label": "Previous series", onclick: () => move(-1) }, "‹") : null,
        h("span", { class: "mono" }, ser.id),
        list.length > 1 ? h("button", { type: "button", "aria-label": "Next series", onclick: () => move(1) }, "›") : null)),
    cases.length ? mapSvg(ser) : null,
    h("ul", { class: "series-case-list" }, cases.map(c => {
      const start = Math.max(0, Math.min(100, (c.window_start - t0) / span * 100));
      const width = Math.max(0, Math.min(100 - start, (c.window_end - c.window_start) / span * 100));
      const cleared = ser.cleared?.case_id === c.id || c.status?.kind === "cleared_by_arrest";
      return h("li", {}, h("button", { type: "button", class: "series-case" + (offending.has(c.id) ? " conflict" : ""), "aria-current": String(c.id === S.current?.case.id), onclick: async () => {
        await selectCase(c.id);
        if (S.current?.case.id === c.id) { S.seriesPick = ser.id; renderSeries(); showTab("series"); }
      } },
        h("span", { class: "series-case-title" }, h("span", { class: "mono" }, c.id), h("span", { class: "series-case-arrow", "aria-hidden": "true" }, "↗")),
        h("span", { class: "series-case-address" }, human(c.block)),
        h("span", { class: "series-case-time" }, `${fmtTime(c.window_start)} – ${fmtTime(c.window_end)}`),
        h("span", { class: "case-range", role: "img", "aria-label": `${c.id}: ${fmtTime(c.window_start)} to ${fmtTime(c.window_end)}` }, h("i", { style: `left:${start}%;width:${width}%;` })),
        cleared ? h("span", { class: "series-case-note" }, "Cleared by arrest") : null));
    })),
    h("ul", { class: "series-checks", "aria-label": "Series checks" }, v.checks.map(c => h("li", { class: "series-check " + c.verdict },
      h("span", { class: "check-symbol", "aria-label": c.verdict === "pass" ? "Passed" : c.verdict === "weak" ? "Needs review" : "Conflict" }, c.verdict === "pass" ? "✓" : "!"),
      h("div", {}, h("span", {}, labels[c.name] || human(c.name.replace(/_/g, " "))), c.verdict === "pass" ? null : h("p", {}, c.reason))))),
    h("div", { class: "series-approval" }, approve,
      v.verdict === "fail" ? h("p", { class: "series-blocked" }, "Resolve the conflicts before approving.") : null)
  );
}

// ---------------------------------------------------------------- right: victim
function renderVictim() {
  const d = S.current, pane = $("pane-victim");
  const saved = d.recorded.victim_liaison;
  const rec = saved?.output.language?.toLowerCase() === "en" ? saved : null;
  const msg = rec ? rec.output.message : d.victim_template.message;
  const lang = "en";
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
    h("div", { class: "section-head" }, h("h2", {}, "Victim update"), h("span", {}, `Draft · ${lang.toUpperCase()}`)),
    rec ? null : h("p", { class: "kv" }, "Standard template · No recorded draft"),
    h("div", { class: "packet-text", lang: lang }, msg),
    rec?.output.plain_english_summary ? h("div", { class: "kv" }, "English summary: " + rec.output.plain_english_summary) : null,
    h("div", { class: "kv" }, h("strong", {}, "Approval only. "), "Delivery is not connected."),
    h("div", { class: "row-flex" }, btn)
  );
}

// ---------------------------------------------------------------- drawers
function activityItem(e) {
  const labels = { accept_step: "Accepted next step", reject_step: "Rejected next step", approve_series_alert: "Approved series alert", approve_victim_update: "Approved victim update", request_approval: "Requested approval" };
  const d = e.detail || {};
  const title = labels[e.action] || human(e.action.replace(/_/g, " "));
  return h("li", { class: "activity-entry" },
    h("div", { class: "activity-title" }, h("b", {}, title), h("span", { class: "kv" }, e.subject)),
    d.step || d.recommendation?.step ? h("p", { class: "activity-description" }, d.step || d.recommendation.step) : null,
    d.reason ? h("p", { class: "kv" }, "Reason: " + d.reason) : null,
    d.note ? h("p", { class: "kv" }, d.note) : null,
    h("div", { class: "kv" }, fmtTime(Number(e.ts)) + " · " + (e.actor === "det.demo" ? "Demo detective" : e.actor)),
    h("details", { class: "activity-verification" }, h("summary", {}, "Verification"),
      h("p", { class: "kv" }, "Entry #" + auditNumber(e.seq)), h("p", { class: "mono" }, "Hash: " + e.hash)));
}
async function loadAudit() {
  const a = await api("/api/audit");
  const ok = a.chain.intact;
  const chip = $("audit-chip");
  chip.className = "activity-count";
  chip.textContent = ok ? a.chain.entries : "!";
  chip.title = ok ? "Audit chain intact" : `Audit chain broken at #${auditNumber(a.chain.first_bad_seq)}`;
  $("audit-status").textContent = ok ? `Chain intact ✓ (${a.chain.entries} entries)` : `Chain broken at entry #${auditNumber(a.chain.first_bad_seq)}`;
  $("audit-status").className = "status " + (ok ? "ok" : "bad");
  $("audit-list").replaceChildren(...[...a.entries].reverse().map(activityItem));
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
  globalThis.CasefileMotion?.drawers();
  const d = $(id);
  if (!d.hidden) d.querySelector("[data-close]").focus();
}
document.querySelectorAll("[data-close]").forEach((b) => (b.onclick = () => { b.closest(".drawer").hidden = true; }));
$("btn-audit").onclick = async () => { await loadAudit().catch(() => {}); openDrawer("drawer-audit"); };
$("btn-team").onclick = () => openDrawer("drawer-team");
document.addEventListener("keydown", (e) => { if (e.key === "Escape") document.querySelectorAll(".drawer").forEach((d) => (d.hidden = true)); });

// ---------------------------------------------------------------- tabs
const EVIDENCE_TABS = ["report", "property", "related"];
function showEvidence(name) {
  EVIDENCE_TABS.forEach(t => {
    const on = t === name;
    $("evidence-tab-" + t).setAttribute("aria-selected", String(on));
    $("evidence-tab-" + t).tabIndex = on ? 0 : -1;
    $("evidence-" + t).hidden = !on;
  });
}
EVIDENCE_TABS.forEach((t, i) => {
  $("evidence-tab-" + t).onclick = () => showEvidence(t);
  $("evidence-tab-" + t).onkeydown = e => {
    const n = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? 2 : null;
    if (n == null) return;
    e.preventDefault();
    const target = EVIDENCE_TABS[(n + EVIDENCE_TABS.length) % EVIDENCE_TABS.length];
    showEvidence(target); $("evidence-tab-" + target).focus();
  };
});

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
    const n = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? TABS.length - 1 : null;
    if (n == null) return;
    e.preventDefault();
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
  $("selected-case-label").textContent = id;
  // The desktop inbox stays visible; narrow screens use the native disclosure.
  $("case-switcher").open = !!wideInbox?.matches;
  markCurrent(); renderCase(); renderPacket(); renderSeries(); renderVictim();
  globalThis.CasefileMotion?.caseTransition();
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
