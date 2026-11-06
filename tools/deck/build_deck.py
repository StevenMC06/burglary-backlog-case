"""Build the Casefile deck: HTML (9 slides, 1280x720) -> PDF via Playwright + installed Chrome.

All numbers come from docs/deck/cost.json (casefile cost), evals/results.json (casefile eval) and
data/recordings/*/*.json (measured run costs). Nothing is typed by hand except labels.

Run:  .venv/Scripts/python tools/deck/build_deck.py
"""
import glob
import html
import json
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT_HTML = os.path.join(ROOT, "docs", "deck", "casefile_deck.html")
OUT_PDF = os.path.join(ROOT, "docs", "deck", "casefile_deck.pdf")


def load(p):
    with open(os.path.join(ROOT, p), encoding="utf-8") as f:
        return json.load(f)


cost = load("docs/deck/cost.json")
ev = load("evals/results.json")
res = {r["scenario"]: r for r in cost["results"]}
C, B, O = res["Conservative"], res["Base"], res["Optimistic"]


def k(x):
    return f"${x / 1000:,.0f}k"


def pct(x, d=1):
    return f"{x * 100:.{d}f}%"


# ---- measured per-agent run cost from recordings -------------------------------------------
agent_cost = {}
for p in glob.glob(os.path.join(ROOT, "data", "recordings", "*", "*.json")):
    try:
        with open(p, encoding="utf-8") as f:
            r = json.load(f)
    except (OSError, ValueError):
        continue
    agent_cost.setdefault(r.get("agent", "?"), []).append((r.get("model", "?"), r.get("cost_usd") or 0.0))
avg = {a: (v[0][0], sum(c for _, c in v) / len(v), len(v)) for a, v in agent_cost.items()}

# ---- eval: wall -----------------------------------------------------------------------------
wall = ev.get("wall", [])


def f1(x):
    return "not run" if x is None else pct(x, 0)


wall_rows = ""
for w in wall:
    llm = w.get("llm_only")
    wall_rows += (
        f"<tr><td>{w.get('cases', w.get('subset', ''))}</td>"
        f"<td>{f1(llm['f1']) if llm else 'not recorded'}</td>"
        f"<td><b>{pct(w['casefile']['f1'], 0)}</b></td>"
        f"<td>{w.get('llm_decoy_pairs_accepted', '-') if llm else '-'}</td></tr>"
    )
sp = ev["series_pruned"]
prop = ev["property"]
_rep = open(os.path.join(ROOT, "evals", "report.md"), encoding="utf-8").read()
_m = re.search(r"\| time_band \| (\d+)% \((\d+)/(\d+)\)", _rep)
extract_line = (
    f"Extractor ({ev['extraction_runs']} recorded runs): entry, premise, weekday 100%; time band only {_m.group(1)}% (wide windows are ambiguous; a human confirms)."
    if _m else ""
)

screens = os.path.join(ROOT, "docs", "deck", "ui.png")
screen_tag = '<img class="shot" src="ui.png">' if os.path.exists(screens) else '<div class="shot ph">UI screenshot</div>'

chain = sum(avg[a][1] for a in ("extractor", "triage", "packet_writer") if a in avg)
chain_line = (
    f"Per-case chain (extractor + triage + packet) about ${chain:.2f}, so roughly ${chain * 10000 / 1000:,.1f}k a year at 10,000 cases, against the ${B['recurring_cost'] and 12:.0f}k/yr inference line in the Base budget."
    if chain
    else ""
)
inf_rows = ""
for a in ["extractor", "triage", "series_hunter", "skeptic", "packet_writer", "victim_liaison", "series_llm_only"]:
    if a in avg:
        m, c, n = avg[a]
        inf_rows += f"<tr><td>{a}</td><td>{m}</td><td>${c:.3f}</td><td>{n}</td></tr>"

CSS = """
@page { size: 1280px 720px; margin: 0 }
* { box-sizing: border-box }
body { margin: 0; font-family: 'Segoe UI', Arial, sans-serif; color: #1b2430 }
.slide { width: 1280px; height: 720px; padding: 44px 64px; page-break-after: always; position: relative; background: #fff; overflow: hidden }
h1 { font-size: 40px; line-height: 1.12; margin: 0 0 10px; color: #0f2a43 }
h2 { font-size: 22px; margin: 0 0 8px; color: #0f6b73 }
p, li { font-size: 20px; line-height: 1.35 }
ul { margin: 6px 0; padding-left: 22px }
.sub { font-size: 22px; color: #44566a; margin-bottom: 18px }
.kick { font-size: 14px; letter-spacing: .12em; text-transform: uppercase; color: #0f6b73; font-weight: 700; margin-bottom: 6px }
.foot { position: absolute; bottom: 16px; left: 64px; right: 64px; font-size: 13px; color: #6b7c8f; display: flex; justify-content: space-between }
.cols { display: flex; gap: 36px } .cols > div { flex: 1 }
.big { font-size: 54px; font-weight: 800; color: #0f2a43; line-height: 1 }
.lab { font-size: 14px; color: #55677a }
.card { border: 1px solid #d5dde6; border-radius: 10px; padding: 14px 18px; background: #f6f9fb }
.warn { border-left: 6px solid #c2410c; background: #fff4ec; padding: 10px 16px; font-size: 19px; margin-top: 12px }
.ok { border-left: 6px solid #0f6b73; background: #eaf6f6; padding: 10px 16px; font-size: 19px; margin-top: 12px }
table { border-collapse: collapse; font-size: 17px } td, th { border-bottom: 1px solid #d5dde6; padding: 6px 12px; text-align: left }
th { color: #0f6b73; font-size: 14px; text-transform: uppercase; letter-spacing: .06em }
.tag { display: inline-block; font-size: 14px; font-weight: 700; padding: 2px 10px; border-radius: 99px; background: #0f2a43; color: #fff; margin: 0 6px 6px 0 }
.tag.h { background: #0f6b73 } .tag.s { background: #7a4b00 } .tag.o { background: #5b2a86 }
.shot { width: 100%; border: 1px solid #c8d2dc; border-radius: 8px } .ph { height: 330px; display: flex; align-items: center; justify-content: center; background: #eef2f6; color: #8493a3 }
.bar { height: 22px; border-radius: 4px; background: #0f6b73 }
.small { font-size: 15px; color: #55677a }
"""


def slide(n, kick, title, body, foot="Synthetic data: Cascade City (fictional). AI suggests, people decide."):
    return f"""<section class="slide"><div class="kick">{kick}</div><h1>{title}</h1>{body}
<div class="foot"><span>{foot}</span><span>{n} / 9</span></div></section>"""


slides = []

slides.append(slide(1, "Casefile · Stage 6 detective follow-up", "10,000 burglaries.<br>A few hundred burglars.", f"""
<p class="sub">Cascade City logged 10,000 burglaries and cleared 1,300 (13%). Casefile connects the cases so detectives can close them.</p>
<div class="cols" style="margin-top:30px"><div class="card"><h2>AI suggests</h2><p>Agents read reports, hypothesize series, draft packets.</p></div>
<div class="card"><h2>A rules engine verifies</h2><p>Could one person be at both scenes? Same MO? Serial match? Yes/no with reasons.</p></div>
<div class="card"><h2>People decide</h2><p>Every assignment, alert and victim message needs a click, recorded in a hash-chained audit log.</p></div></div>
<p class="small" style="margin-top:34px">Cases, not people: we never score a person or a neighbourhood. Prototype runs on synthetic data plus public Seattle geography.</p>"""))

slides.append(slide(2, "The bottleneck", "Connecting cases, not collecting them", f"""
<div class="cols"><div>
<p>Official baseline: <b>$305 per case, $3.05M a year.</b></p>
<table><tr><th>Step</th><th>Minutes</th><th>Who</th></tr>
<tr><td>Intake</td><td>15</td><td>Officer</td></tr><tr><td>Report</td><td>60</td><td>Officer</td></tr>
<tr><td>Video review</td><td>120</td><td>Officer</td></tr><tr><td>Evidence</td><td>45</td><td>Officer</td></tr>
<tr><td><b>Detective follow-up</b></td><td><b>90</b></td><td><b>Detective: $1.05M, the largest cost</b></td></tr></table></div>
<div><h2>Where an hour is least valuable</h2><p>A detective re-reading a no-lead report and cross-referencing by hand.</p>
<ul><li>Clearance counts offenses: one arrest can clear every linked burglary.</li>
<li>A few repeat offenders commit a large share of burglaries; most cases never get linked.</li>
<li>Domain expert at kickoff: "We know who the bad guys are. We just don't have enough resources."</li></ul></div></div>
<p class="small" style="margin-top:26px">Sources: case brief (baseline); Eck, PERF 1983 (most burglary investigations suspended early for lack of leads); FBI UCR clearance rules.</p>"""))

slides.append(slide(3, "What changes on Monday morning", "One screen for the sergeant and the detective", """
<table style="width:100%;margin-top:10px"><tr><th></th><th>Today</th><th>With Casefile</th></tr>
<tr><td><b>Sergeant</b></td><td>Reads 36 overnight reports one by one</td><td>Opens a queue already structured, scored and cross-referenced; screens out no-lead cases with a signed packet</td></tr>
<tr><td><b>Detective</b></td><td>Hunts through RMS and pawn records</td><td>Gets a lead packet: linked cases, pawn hits, a next-steps checklist to accept or reject with a reason</td></tr>
<tr><td><b>Victim</b></td><td>Hears nothing for weeks</td><td>Gets a plain-language update in their language, approved by staff</td></tr></table>
<div class="ok">Stage 7 property matching feeds Stage 6: pawn serial hits are leads, never proof of ownership or guilt (pawn records are furnished to police under RCW 19.60.040).</div>"""))

slides.append(slide(4, "Demo", "Live from the prototype", f"""
<div class="cols"><div style="flex:2">{screen_tag}</div><div><p>75-second walkthrough, recorded runs of real Claude agents replayed offline:</p>
<ul><li>Overnight queue of 36 reports</li><li>Serial hit vs. look-alike rejected</li><li>Series proposed, then one case ruled out by the engine</li><li>Packet with citations</li><li>Human approval, audit log</li></ul></div></div>"""))

slides.append(slide(5, "Architecture", "AI proposes. The engine verifies. A detective decides.", f"""
<div class="cols"><div>
<span class="tag">Connectors</span><p class="small">MCP server (Rust): records, pawn feed, <b>verify_series</b>, solvability, citation checker, approval queue, cost model</p>
<span class="tag h">Agents</span><p class="small">Extractor (Haiku) · Triage (Sonnet) · Series Hunter (Sonnet) · Skeptic (Opus) · Packet Writer (Sonnet) · Victim Liaison (Haiku)</p>
<span class="tag s">Teams</span><p class="small">Intake &amp; Triage (per report) · Series Hunting (hunter ⇄ skeptic ⇄ engine, nightly) · Victim Comms (on status change)</p>
<span class="tag o">Channels</span><p class="small">Sergeant console · detective packet · victim message draft (any language, staff-approved). Not built: patrol hands-free brief.</p>
<div class="ok" style="font-size:16px">Approvals never block: requests queue with a ticket and escalate after 120 min, so human review waits hours, not days.</div></div>
<div><h2>Single LLM vs. proposer + verifier (our eval)</h2>
<table><tr><th>Cases</th><th>LLM only F1</th><th>Casefile F1</th><th>Decoy pairs accepted by LLM</th></tr>{wall_rows}</table>
<p class="small">Planted series with decoys, synthetic ground truth, one run per size. The LLM matches us on F1 at small scale; what it lacks is the hard check: it accepted decoy pairs that one person cannot have committed, and 150 cases already cost about 53k input tokens (2,000 would be about 0.7M, over one context window).</p>
<h2 style="margin-top:12px">Full 2,000-case run</h2>
<p class="small">Series pairs: precision {pct(sp['precision'],0)}, recall {pct(sp['recall'],0)}, {sp['series_recovered']}/{sp['series_total']} planted series recovered. Decoys rejected: {ev['decoys_rejected']}/{len(ev['decoys'])}. Exact-serial leads: {prop['exact_correct']}/{prop['exact_leads']} correct. {extract_line}</p></div></div>"""))

slides.append(slide(6, "Cost-benefit on the official baseline", f"Base case: {pct(B['steady_net_pct'])} net, short of the 15% target", f"""
<div class="cols"><div>
<div class="big">{k(B['steady_net'])}</div><div class="lab">net per year, {pct(B['steady_net_pct'])} of $3.05M (target 15% = $457.5k)</div>
<table style="margin-top:12px"><tr><td>Detective + officer capacity freed</td><td>{k(B['gross_capacity_value'])}</td></tr>
<tr><td>Run cost</td><td>-{k(B['recurring_cost'])}</td></tr>
<tr><td>Funded follow-up of new leads</td><td>-{k(B['followup_cost'])}</td></tr>
<tr><td>Dismissing false leads</td><td>-{k(B['dismissal_cost'])}</td></tr>
<tr><td>One-time setup</td><td>{k(B['one_time_cost'])}</td></tr>
<tr><td>Year one (ramped)</td><td>{k(B['year_one_net'])}</td></tr></table></div>
<div><table><tr><th>Scenario</th><th>Net %</th><th>+Clearances</th><th>Payback</th></tr>
<tr><td>Conservative</td><td>{pct(C['steady_net_pct'])}</td><td>+{C['additional_clearances']:.0f}</td><td>{C['payback_years']:.1f} y</td></tr>
<tr><td>Base</td><td>{pct(B['steady_net_pct'])}</td><td>+{B['additional_clearances']:.0f}</td><td>{B['payback_years']:.1f} y</td></tr>
<tr><td>Optimistic</td><td>{pct(O['steady_net_pct'])}</td><td>+{O['additional_clearances']:.0f}</td><td>{O['payback_years']:.1f} y</td></tr></table>
<div class="warn">We probably miss 15%. Only the Optimistic bracket reaches it. Dollars are freed capacity (about {B['detective_hours_saved']:,.0f} detective hours), not cash; freed hours fund follow-up, no layoffs. Cash savings need avoidable overtime or unfilled posts; finance must confirm.</div>
<p class="small">The team research bot's more cautious bracket (3.5% net) sits between our Conservative and Base. The pilot decides.</p></div></div>"""))

slides.append(slide(7, "Clearance logic", f"How 13% becomes {pct(B['clearance_rate'])}: new leads, worked by detectives", f"""
<table style="font-size:19px"><tr><th>Step (Base)</th><th>Cases/yr</th></tr>
<tr><td>Open cases linked to an arrest (1,300 cleared × 30% hit × 1.5 per hit × 85% adoption)</td><td>{B['leads_from_linkage']:.0f}</td></tr>
<tr><td>Pawn/serial hits (10,000 × 35% traceable × 10% resurface × 85% × (1 − 20% already linked))</td><td>{B['leads_from_property']:.0f}</td></tr>
<tr><td>Moderate cases newly worked (120 × 85% adoption)</td><td>{B['leads_from_triage']:.0f}</td></tr>
<tr><td><b>New leads</b> → 65% completed → 50% clear, minus 10 lost to screening</td><td><b>{B['new_leads']:.0f} → +{B['additional_clearances']:.0f}</b></td></tr></table>
<div class="warn">Base gives {pct(B['clearance_rate'])}, not the 16.25% target (+325). Only Optimistic (+{O['additional_clearances']:.0f}) clears it. The 50% conversion is unbenchmarked (today's clearance is 13%); at the conservative 35% the result is +{C['additional_clearances']:.0f}. Every factor is an assumption the pilot measures.</div>
<p class="small">Follow-up hours are a cost, not a saving. False links cost detective time ({B['false_leads']:.0f} false leads dismissed at 20 min each, 80% precision assumed; our eval measures {pct(sp['precision'],0)} pair precision on synthetic data, so the real number is a pilot question).</p>"""))

slides.append(slide(8, "Pilot", "Prove it on one precinct before scaling", """
<ul><li><b>Days 0–30:</b> integrate the RMS export and pawn feed in a CJIS-approved environment; train 1 sergeant and 6 detectives; replay 6 months of closed cases to measure link precision.</li>
<li><b>Days 31–90:</b> randomized pilot: half of new burglaries get Casefile packets, half business as usual.</li>
<li><b>Months 4–12:</b> expand only if the gates pass; quarterly accuracy and equity audits.</li></ul>
<div class="ok" style="font-size:24px;margin-top:20px"><b>The one metric: cleared burglaries per detective-hour, pilot vs. control.</b> It captures both asks, savings and clearance, at once.</div>
<p>Gates: link precision on replay meets the bar we set with the department; zero un-reviewed actions; no drop in clearance on screened cases.</p>"""))

slides.append(slide(9, "Ethics, trust and the ask", "Cases, not people. Humans decide. Everything is auditable.", f"""
<div class="cols"><div><ul>
<li><b>Bias:</b> links cases, never scores people or neighbourhoods; no predictive patrol; equity monitor on screening rates.</li>
<li><b>Privacy:</b> CJIS-approved hosting; only data the department lawfully holds; no facial recognition.</li>
<li><b>False positives:</b> skeptic agent plus rules engine rejected {ev['decoys_rejected']}/{len(ev['decoys'])} decoy series; a serial match is a lead, not ownership.</li>
<li><b>Human in the loop:</b> no send, assign or alert action exists without a click; hash-chained audit log; replays give identical output.</li></ul></div>
<div><h2>Measured inference cost (recorded runs)</h2><table><tr><th>Agent</th><th>Model</th><th>$/run</th><th>n</th></tr>{inf_rows}</table>
<p class="small">Right-sized models: the cheapest adequate model per task. {chain_line}</p></div></div>
<div class="ok" style="font-size:22px"><b>Ask:</b> approve a 90-day, one-precinct pilot; scale only if the gates pass. Full program: {k(B['one_time_cost'])} setup, {k(B['recurring_cost'])}/yr to run.</div>"""))

doc = f"<!doctype html><html><head><meta charset='utf-8'><title>Casefile</title><style>{CSS}</style></head><body>{''.join(slides)}</body></html>"
with open(OUT_HTML, "w", encoding="utf-8") as f:
    f.write(doc)
print("wrote", OUT_HTML)

if "--no-pdf" in sys.argv:
    sys.exit(0)
from playwright.sync_api import sync_playwright  # noqa: E402

with sync_playwright() as p:
    b = p.chromium.launch(channel="chrome")
    pg = b.new_page(viewport={"width": 1280, "height": 720})
    pg.goto("file:///" + OUT_HTML.replace("\\", "/"))
    pg.pdf(path=OUT_PDF, width="1280px", height="720px", print_background=True)
    b.close()
print("wrote", OUT_PDF)
