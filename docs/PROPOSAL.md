# Cascade City PD: Burglary Backlog, Proposal & Pre-Implementation Spec

Status: **DRAFT v0.1, pre-implementation** (2026-11-05, Thursday evening)
Deadline: deck due **Fri 2026-11-06, 1:00 PM**; pitch Fri 2–5 PM, Dempsey 004 (downstairs). 7 min pitch + 3 min Q&A.
Working title: **"Casefile"**. Codename is fine; rename freely.

---

## 0. TL;DR

- **Thesis:** Cascade City doesn't have 10,000 burglaries. It has a few hundred burglars. Most cases get shelved within two days for "no leads" because nobody connects case #4,812 to case #4,907. This is a **connection problem**, not a detective-effort problem.
- **Bottleneck:** *Case screening & lead development*, meaning the step after the patrol report where a sergeant decides "assign or suspend." Every downstream stage inherits its mistakes.
- **Solution:** a small team of specialised AI agents that structures every report, scores solvability, links the report to other cases and to pawn/property records, and hands the sergeant a **verified lead packet**. A human approves every assignment, alert and message.
- **The hook for Guy:** in his talk he said *"I don't know what the game engine is in criminal investigations."* Our answer is a **deterministic constraint engine over time, space, MO and property**. The LLM proposes a series ("these 6 burglaries are one person"). The engine checks it: can one person physically be at both scenes? Do the MO and property match? Is any case already cleared to someone else? It answers yes or no with reasons. This mirrors his puzzle/grid demo exactly: *LLM proposes, engine verifies, human decides.*
- **Stack:** Rust workspace, built as one binary with `cargo run`. It has a pure-logic engine crate, an MCP server (official `rmcp` SDK), an axum web app with a no-build-step UI, SQLite, and raw-HTTP Claude calls. It supports **offline replay mode** so the live demo can't die on venue Wi-Fi.
- **Math (provisional, to be replaced by case-packet numbers):** clearance +330 cases (target +325) via series linkage, property matching and triage reinvestment. Labor −14.5% to −15% net of solution cost under the whole-pipeline definition, and about −25% if "labor" means investigations labor. **Clarify the denominator with organizers tonight** (§9).

---

## 1. Reading the case

| Item | Value | Note |
|---|---|---|
| Burglaries / yr | 10,000 | fictional city |
| Cleared | 13% = 1,300 | US 2024 actual is ~15.2% (FBI); 13% is realistic |
| Ask #1 | 13% → 16% = **+325 cleared/yr** | ≈ +25% relative (the presenter said 20.5%; 325/1,300 = 25%) |
| Ask #2 | **−15% labor cost, net of our solution's cost** | denominator ambiguous, see §9 |
| Rules | no facial recognition; public or made-up data only; **a human makes every decision** | design these in, not around |
| Your job | pick **ONE** of 7 stages; show time, money, clearance; list assumptions | |
| Judging (Guy, tech) | connectors, agent definitions, teams of agents, channels | his exact words |
| Judging (others) | business: money; domain: "is it reasonable to solve crime with this?" | |

**Assumed 7 stages** (replace with the packet's list when the email arrives):
1. Report / call intake → 2. Patrol response & preliminary investigation → 3. Evidence collection (CSI) → **4. Case screening & assignment** → 5. Follow-up investigation → 6. Suspect ID & arrest → 7. Charging / closure / victim notification.

---

## 2. Why stage 4 (screening & lead development)

Evidence, in the order to use it on slides:

1. **Cases die early.** PERF/Eck's study of 3,000+ burglaries: about **¾ of investigations were suspended within 2 days for lack of leads**, and follow-up rarely exceeded 4 hours. When follow-up did happen, it materially affected arrests. ([Eck 1983 abstract](https://www.ncjrs.gov/App/Publications/abstract.aspx?ID=155079))
2. **Effort pays off in "moderate" cases.** A replication found more detective time significantly raised arrest probability in cases with *moderate* suspect information ([1994 study](https://ojp.gov/ncjrs/virtual-library/abstracts/relationship-between-evidence-detective-effort-and-disposition)). Triage that finds the moderate cases is the lever.
3. **Screening saves real hours.** Cambridge burglary solvability work: a screening model could cut investigative workload by **up to 42%** while retaining most detections. Average investigation is about 3.7–4.5 h. Caveat: some low-solvability cases still get solved, so triage must not be a hard cut. ([Cambridge repo](https://www.repository.cam.ac.uk/items/06bc10d8-81ac-4f52-b160-6c580b2d6b1e), [Paine thesis](https://www.crim.cam.ac.uk/system/files/documents/colin-paine.pdf))
4. **Linking burglaries works and is machine-friendly.** Burglary linkage models reach **AUC > 0.90** (logistic regression on MO, geography and time), and they outperformed police professionals in one study. ([Tonkin/Woodhams](https://research.utwente.nl/en/publications/linkage-analysis-in-cases-of-serial-burglary-comparing-the-perfor/), [Leicester comparison](https://leicester.figshare.com/articles/Linking_Property_Crime_Using_Offender_Crime_Scene_Behaviour_A_Comparison_of_Methods/10198388/files/18379169.pdf))
5. **One arrest can clear many offenses.** Clearance counts offenses, not offenders ([BOCSAR](https://www.bocsar.nsw.gov.au:443/Publications/CJB/cjb130.pdf)). A small share of offenders commits most crime. Linkage is a **clearance multiplier**.
6. **Property is a lead source.** Maryland's pawn/secondhand program: 20,000+ investigations closed cumulatively. Raleigh credited 163→212 solved cases/yr to pawn data. These figures are vendor/agency-reported, so treat them as directional. ([MD](https://mdsp.prod.maryland.gov/media/537), [Raleigh](https://www.police1.com/police-products/software/data-information-sharing-software/articles/case-study-how-one-pd-used-an-online-tracking-system-to-recover-313k-in-stolen-merch-LqJS3ubMLY5fiak9))

**Why NOT the obvious alternatives** (use this in Q&A; it shows domain depth):
- *AI report writing (stage 2):* the only RCT (Adams et al., Axon Draft One) found **no time savings**, and Anchorage dropped its trial because review time ate the gains ([EFF](https://www.eff.org/deeplinks/2025/03/anchorage-police-department-ai-generated-police-reports-dont-save-time), [KQED](https://www.kqed.org/news/12007520)). Vendor claims of 40–65% savings are unverified.
- *Online reporting / diverting patrol (stage 1):* large hours savings (Dallas: about 51k patrol hrs in 2022), but it skips scene evidence. DNA at burglary scenes **more than doubled suspect identification** in the NIJ field experiment ([NIJ](https://nij.ojp.gov/topics/articles/using-dna-solve-property-crimes)). It trades clearance for cost, and we need both.
- *Predictive patrol / near-repeat hotspots:* the evidence is mixed and it carries reputational risk. We use near-repeat only as a **linkage feature**, never to target people or neighbourhoods.

---

## 3. Solution design: Guy's four layers

### 3.1 Connectors (MCP servers): data plus symbolic computation

| Server | Tools (examples) | Type | Why it's a tool, not an LLM |
|---|---|---|---|
| `records` | `get_case`, `search_cases(filters)`, `case_status` | data | RMS is the source of truth |
| `evidence` | `evidence_status(case)` (prints/DNA/footwear queue) | data | lab state is exact |
| `property` | `pawn_lookup(serial|description)`, `property_hits(since)` | data | exact match on serials |
| **`linkage-engine`** | `similar_cases(case, k)`, `propose_clusters(window)`, **`verify_series(case_ids)`** | **symbolic** | Guy's "grid": deterministic yes/no plus reasons |
| `solvability` | `score(case)` → 0–100, band, per-factor contributions | symbolic/model | trained logistic model, explainable |
| `citations` | `check_citations(packet)` | symbolic | every claim must resolve to a real record field |
| `approval` ("approval MCP", Guy's term) | `request_approval(kind, payload, deadline)` → ticket; `approval_status(ticket)` | human channel | **non-blocking**: the pipeline keeps doing background work; ~2 h timeout → escalate to the next approver (Guy: "a couple hours, not a couple days"); maps to MCP **Tasks** extension polling |
| `notify` | `draft_victim_update(case, lang)` (draft only) | channel | sending needs an approval ticket |
| `cost-model` | `compute(params)` → hours ledger, net $, clearance delta, sensitivity | symbolic | **slide numbers come from code, not the LLM** |

`verify_series` hard constraints (the "game engine"):
- **Spatiotemporal feasibility:** for any two cases with overlapping offense windows, can one person travel between them? (haversine distance ÷ plausible urban speed)
- **Conflicting clearance:** a case already cleared to a different arrestee → conflict.
- **MO consistency:** entry point/method, target type, time-of-day/day-of-week bands, property types. Soft score with a threshold, not hard.
- **Physical-evidence contradictions:** e.g., footwear class/size mismatch.
- Returns `{verdict: pass|fail|weak, checks: [{name, pass, reason, cited_fields}]}`.

### 3.2 Agents: each has instructions, model, tools and a test

Model tiering per Guy ("don't always use the most expensive model"). Prices are per 1M tokens, input/output:

| Agent | Role (Guy's "brain / muscle / face") | Model | Tools | Output contract | Tested by |
|---|---|---|---|---|---|
| Intake Extractor | the clerk | **Haiku 5.5** ($0.10/$0.50), effort low | `get_case` | strict JSON MO schema | field accuracy vs gold |
| Triage Analyst | the brain | **Sonnet 5.5** ($2/$10), effort medium | `score`, `similar_cases`, `pawn_lookup`, `evidence_status` | disposition recommendation plus rationale → approval ticket | agreement with gold disposition |
| Series Hunter | the bloodhound | **Sonnet 5.5** via **Batch API (−50%)**, nightly | `propose_clusters`, `verify_series` | candidate series | series recall |
| Skeptic | the devil's advocate | **Opus 5.5** ($4/$20), effort high | `verify_series`, `get_case` | attack each series; keep only survivors | catch rate on planted **decoy** series |
| Packet Writer | the scribe | **Sonnet 5.5** | `get_case`, `check_citations` | detective lead packet, all claims cited | 0 uncited claims |
| Victim Liaison | the face | **Haiku 5.5** | `case_status`, `draft_victim_update` | calm, plain-language, any language; **draft only** | rubric plus human approval |

**Domain language (Guy: "learn the language of the case"):** the Extractor ships with a police-shorthand glossary (POE, UNK SUSP, RP, entry-method codes) plus few-shot examples. The roadmap item is a fine-tuned extractor on the department's own reports.

**Wiring is many-to-many, not one-to-one** (Guy's rubric): the architecture slide shows an agent × connector matrix.

**Orchestrator is code, not an LLM.** The workflow is known (intake → score → link → verify → packet → approve), so a deterministic Rust state machine runs it. LLMs are used only where judgment or language is needed. This is a deliberate point to make to Guy.

### 3.3 Teams
- **Team A, Intake & Triage** (per report, near-real-time): Extractor → Triage Analyst → Packet Writer.
- **Team B, Series Hunting** (nightly batch over all open cases): Series Hunter ⇄ Skeptic ⇄ `verify_series`. This is the proposer/grader loop from Guy's puzzle demo.
- **Team C, Victim Comms** (event-driven): Liaison drafts on status change → sergeant/clerk approves → send.

### 3.4 Channels (human ↔ AI)
| Human | Channel | What flows |
|---|---|---|
| Screening sergeant | web **triage console** (approval queue) | approve/override assignment, approve series alerts. **Every decision is a human click** |
| Detective | lead packet in console plus a morning digest; optional MCP access from Claude Desktop ("ask the case file") | verified leads with citations |
| Victim | SMS / email / web status page, any language | approved updates; inbound "any news?" answered from case status |
| Chief / analyst | KPI dashboard | clearance, hours ledger, equity monitor |
| Patrol (future) | hands-free voice on MDT | "what happened on this block in the last 14 days?" |

### 3.5 Guardrails (score these with the domain judges)
- No facial recognition, no biometrics. **We link cases, not people.** No individual risk scores and no neighbourhood "hotspot" targeting.
- A human approves every assignment, series alert, victim message and arrest decision. The system cannot act on its own: no send tool exists without an approval ticket.
- Append-only, **hash-chained audit log** of every agent action, tool call and human decision (tamper-evident, cheap to build).
- **Equity monitor:** suspension rate and time-to-assignment by precinct, so solvability scoring can't quietly abandon poorer areas.
- Deployment: AWS GovCloud via Bedrock (Claude is available there, FedRAMP-authorized). The CJIS Security Policy doesn't mandate a vendor, but the agency's CJIS Systems Officer must sign off ([AWS CJIS + Bedrock](https://aws.amazon.com/blogs/publicsector/building-a-rag-architecture-on-amazon-bedrock-to-support-cjis-compliance/)). No training on department data.

---

## 4. The math (provisional base case; the cost model recomputes everything)

> All of these are **assumptions**, listed explicitly as the case requires. If the case packet gives stage-level hours or costs, they replace these. The `cost-model` tool regenerates the slide charts in one command.

### 4.1 Labor baseline (burglary handling only)
Loaded hourly cost: officer $85, detective $95, sergeant $105, civilian $55. Basis: SPD 2020 avg detective cost ≈ $155k/yr ≈ $75/h ([SPD budget](https://api.sambuz.com/doc/seattle-police-department-budget-overview-ppt-presentation-94352)), plus about 25% for 2024–26 raises.

| Activity | Volume × hours × rate | $ / yr |
|---|---|---|
| Patrol response + prelim report | 10,000 × 2.0 h × $85 | 1,700,000 |
| Sergeant screening | 10,000 × 0.25 h × $105 | 262,500 |
| Detective follow-up (40% assigned) | 4,000 × 4.0 h × $95 (Eck ≤4 h; UK 3.7–4.5 h) | 1,520,000 |
| Suspended-case admin | 6,000 × 0.5 h × $95 | 285,000 |
| Victim contact / status calls | 10,000 × 0.5 h × $55 | 275,000 |
| **Total** | | **4,042,500** (investigations-only: 2,342,500) |

### 4.2 Hours ledger (stage 4 + what it feeds)
| Lever | Change | Hours saved | $ |
|---|---|---|---|
| Screening with AI packet | 15 → 5 min | 1,700 | 178,500 |
| Detective follow-up (triage + auto-assembled packet) | −25% (evidence says up to 42%) | 4,000 | 380,000 |
| Suspended-case admin (auto-drafted closure + coding) | 0.5 → 0.15 h | 2,100 | 199,500 |
| Victim status (proactive approved updates deflect calls) | 0.5 → 0.2 h | 3,000 | 165,000 |
| **Gross** | | **10,800** | **923,000** |
| Reinvested into moderate cases & series leads | | −1,200 | −114,000 |
| Solution cost (below) | | | −224,000 |
| **Net** | | | **≈ 585,000 = 14.5% of total / 25% of investigations labor** |

Solution cost/yr (lean): Claude tokens ≈ $5k (about $0.15/case × 10k × 3 safety factor; tokens are a rounding error, which is worth saying out loud), GovCloud hosting $24k, AI-on-Cloud platform $90k, implementation $120k amortised over 3 yrs = $40k, training $15k, 0.5 FTE analyst for QA/audit $50k.

Savings come from **overtime reduction and not back-filling attrition, not layoffs** (Guy: "we can't fire everybody").
**Gap to close if the denominator is the whole pipeline:** ~0.5 pt. Options: (a) steady-state view: training is one-time and amortisation ends after yr 3 → $640k ≈ 15.8%, (b) count detective overtime at 1.5× (needs a packet data point), (c) the packet's own numbers may differ. Decide once the case email arrives.

### 4.3 Clearance bridge (+325 needed)
| Lever | Logic | Base | Low | High |
|---|---|---|---|---|
| Series linkage after arrests | 25% of 1,300 cleared cases surface verified linked open cases; 45% convert to an additional cleared offense (property recovered, debrief, pawn) | **+150** | +80 | +220 |
| Property/serial auto-matching | serials and distinctive items extracted from every report, matched daily against pawn feed (Raleigh: 163–212 solved/yr) | **+80** | +50 | +120 |
| Triage reinvestment | 1,200 h → ~300 more moderate cases worked at ~33% incremental clearance | **+100** | +60 | +140 |
| **Total** | | **+330** | +190 | +480 |

Slides: waterfall (13% → 16%) plus a tornado sensitivity chart, both rendered from `cost-model`.

---

## 5. Prototype spec

### 5.1 Tech stack decision
**Rust, one Cargo workspace, one binary.** Why it fits this case and not just the hype:
- The engine *is* the product's credibility. Rust's type system plus `#[forbid(unsafe_code)]` and fast deterministic tests suit a "verifier" whose job is to be right.
- Single static binary, no runtime: `cargo run` locally, and a prebuilt `.exe` on the presenting laptop.
- The official MCP Rust SDK (`rmcp` 3.9, targets spec 2026-07-28) is mature.

Honest costs: slower iteration than Python, and **no official Anthropic Rust SDK**, so Claude is called over raw HTTP (documented Messages API; ~200 lines for the tool loop). **Escape hatch:** MCP is the seam. If the Rust agent loop slips, a 100-line Python orchestrator using the official `anthropic` SDK can drive the same Rust MCP server with no other change. Python 3.14 is already installed.

Verified installed: Rust 1.99.1, Python 3.14.7, git, gh. **Not installed: Node.** Hence the no-build-step UI.

Crates (deliberately few): `tokio`, `axum 0.8`, `reqwest` (rustls), `serde`/`serde_json`, `rusqlite 0.41` (bundled SQLite), `rmcp 3.9`, `tracing`, `thiserror`/`anyhow`, `sha2` (audit chain). Plus `toml` for config. No ML crates: the logistic model is ~60 lines of hand-written gradient descent.

### 5.2 Repo layout
```
esec-case-comp/
  Cargo.toml                 # workspace
  crates/
    engine/                  # PURE: no IO, no async. solvability, linkage, verify_series, citations, cost_model
    store/                   # SQLite schema + repo (cases, pawn, evidence, approvals, audit_log)
    mcp/                     # rmcp server exposing engine+store tools (stdio + streamable HTTP)
    agents/                  # Claude HTTP client, tool loop, agent defs, orchestrator state machine, replay recorder
    app/                     # axum: REST + SSE + static UI; main binary
  agents/*.toml              # agent definitions: instructions, model, effort, tools, output schema
  ui/                        # index.html, app.css, app.js: vanilla, no build step, offline-capable
  tools/datagen/             # synthetic data generator (planted series + decoys + pawn ledger)
  data/seed/                 # generated made-up data (committed)  |  data/replay/ recorded LLM runs
  evals/                     # gold sets + runner -> evals/report.md
  docs/                      # PROPOSAL.md, ARCHITECTURE.md, ASSUMPTIONS.md, DEMO.md, QA_PREP.md
```
Run modes: `cargo run -p app -- --live` (calls Claude), `--replay` (recorded responses, **no network**), and `cargo run -p engine --bin cost-model -- params.toml` (CSV/SVG for slides).

### 5.3 Data (public + made-up only)
- **Geography:** real public SPD burglary block-level coordinates from Seattle open data ([SPD Crime Data 2008–present](https://catalog.data.gov/dataset/spd-crime-data-2008-present)), relabelled "Cascade City". No names, no PII.
- **Synthetic reports:** ~2,000 burglaries over 12 months. **40 planted series** (2–12 cases each, shared MO and travel-feasible), **10 decoy series** (look similar but violate a constraint), a pawn ledger in which about 5% of stolen serials resurface, and evidence queue states. Narratives are generated once by Haiku from the structured truth and committed, so extraction can be scored against truth.

### 5.4 Evals: the slide that wins Guy
Replicate his own chart ("single LLM accuracy collapses as puzzles get harder") on *our* domain:

| Condition | Task | Metric |
|---|---|---|
| LLM-only (Opus 5.5, all cases in context) | find the series | series precision/recall, decoys accepted |
| Engine-only | same | same |
| **LLM + engine + skeptic (ours)** | same | same, plus **decoys that reached a human: target 0** |

Plus: extraction field accuracy, citation pass rate, and **measured** $/case from `usage` (which replaces the token estimate in §4). Run at 3 sizes (50 / 500 / 2,000 cases) to show where the single-LLM breakpoint is.

### 5.5 Demo script (90 s live, backup video recorded)
1. Sergeant console: *"Overnight: 38 new burglary reports."* Press **Run triage**; agents stream status.
2. A case card shows solvability **62 / moderate** with factor bars, plus a **pawn hit**: the stolen MacBook serial was pawned 3 days later.
3. Series panel: Hunter proposes **Series S-17 (7 cases)**. Skeptic plus engine **reject 1 case** ("offense windows overlap, 9.4 km apart in 20 min: infeasible"). The map/timeline shows survivors.
4. Sergeant clicks **Approve**. The lead packet is generated with every claim cited. The victim update is drafted in Spanish and waits in the approval queue.
5. KPI strip: hours saved and projected clearance, both from `cost-model`.
6. *(If time)* the same MCP server in Claude Desktop: "What links case 4812 to anything else?"

---

## 6. Deck (10 slides, 7 minutes)
1. **Hook:** "10,000 burglaries. A few hundred burglars." (0:30)
2. **The 7-stage map:** where hours and dollars go, with stage 4 highlighted. (0:45)
3. **Why stage 4:** ¾ shelved in 2 days, moderate cases respond to effort, linkage AUC > 0.9. (0:45)
4. **A sergeant's morning:** before vs after. (0:30)
5. **Architecture:** connectors, agents, teams, channels (Guy's framework, by name), with the agent × connector matrix. (0:45)
6. **"LLM proposes, engine verifies, human decides":** quote Guy's "I don't know what the game engine is in criminal investigations," then show our answer and the eval chart reproducing his "wall." (0:45)
7. **Live demo.** (1:30)
8. **Clearance bridge:** 13% → 16% waterfall. (0:30)
9. **Cost:** hours ledger, solution cost, net %, tornado. (0:30)
10. **Guardrails & 90-day pilot:** one precinct, KPIs, go/no-go. (0:30)

Appendix: assumptions table, sources, agent cards, Q&A backups.

## 7. Q&A prep (prepare answers in docs/QA_PREP.md)
- "Why not just use ChatGPT?" → the LLM-only eval row.
- "What if the AI links an innocent person?" → it links *cases*, not people. A skeptic agent plus the engine filter, and a human approves; the AI never names suspects from thin air.
- "Isn't this predictive policing?" → no person or neighbourhood scoring; plus the equity monitor.
- "Where do the savings actually come from if no one is fired?" → overtime plus attrition.
- "Why Haiku here and Opus there?" → cost/accuracy per task, measured.
- "AI report writing saves time, why not that?" → the RCT says it doesn't.
- "CJIS?" → GovCloud/Bedrock, CSO sign-off, no training on data, audit chain.
- "What if the pawn data doesn't exist?" → lever sensitivity: clearance still +250 without it.

## 8. Timeline (≈ 17 h to deck deadline)
| When | Tech (me + you) | Business teammates |
|---|---|---|
| Thu 7–9 PM | workspace scaffold, schema, datagen, engine skeleton + tests | read packet, pick/confirm stage, fill assumptions sheet |
| Thu 9 PM–12 AM | engine complete (scoring, linkage, verify, cost model), MCP server | slide skeleton, sources |
| Fri 7–10 AM | agent loop + orchestrator, UI, record replay run | finance slides from cost-model CSVs |
| Fri 10–11:30 AM | evals, screenshots, **record backup demo video**, release build | full rehearsal ×2 at 7:00 |
| Fri 11:30–12:45 | freeze code, fix only demo-blocking bugs | **submit deck by 12:45** |

Parallelism: I can split engine / datagen / UI into separate git worktrees with sub-agents once you say go.

## 9. Open questions (blocking first)
1. **Case email:** forward it the moment it arrives, especially the 7 stages and any stage-level hours/costs.
2. **"−15% labor cost": total burglary labor or the chosen stage's labor?** Decides whether we're at 14.5% or 25%. Email the organizers tonight (I'll draft it).
3. **Claude API access:** do you have an API key with credits, or does AI-on-Cloud provide one? Live build plus evals is roughly $10–30 total.
4. **Team:** size, names/roles, who owns finance and slides, who presents the demo.
5. **Rust comfort:** OK with me writing most of the Rust while you review and drive? Or Python orchestrator from the start?
6. **Pitch logistics:** whose laptop presents, OS, and whether the room has reliable Wi-Fi (replay mode covers us either way).

## 10. Risks
| Risk | Mitigation |
|---|---|
| Venue Wi-Fi / API outage | `--replay` mode plus recorded video |
| Rust iteration speed | engine first (pure, testable); Python orchestrator fallback over MCP |
| Judges distrust "AI policing" | guardrails slide, cases-not-people, human-approval clicks visible in demo |
| Synthetic data looks fake | real public Seattle geography; planted structure stated openly |
| Our numbers ≠ packet numbers | everything parameterised in `cost-model`; swap in under 5 min |
| Scope creep | demo path in §5.5 is the only must-have; everything else is stretch |

---

## Sources
- [FBI 2024 Reported Crimes in the Nation](https://cde.ucr.cjis.gov/LATEST/resources/reports/Reported%20Crimes%20in%20the%20Nation%20Quick%20Stats.pdf)
- [Eck 1983, *Solving Crimes* (PERF)](https://www.ncjrs.gov/App/Publications/abstract.aspx?ID=155079), [1994 follow-up test of triage](https://ojp.gov/ncjrs/virtual-library/abstracts/relationship-between-evidence-detective-effort-and-disposition), [Greater Manchester replication](https://cris.huji.ac.il/en/publications/assessing-solvability-factors-in-greater-manchester-england-the-c/)
- [Cambridge: resources & solvability in burglary detection](https://www.repository.cam.ac.uk/items/06bc10d8-81ac-4f52-b160-6c580b2d6b1e), [Paine](https://www.crim.cam.ac.uk/system/files/documents/colin-paine.pdf), [Olphin](https://www.crim.cam.ac.uk/system/files/documents/tom-olphin.pdf)
- [Linkage analysis in serial burglary](https://research.utwente.nl/en/publications/linkage-analysis-in-cases-of-serial-burglary-comparing-the-perfor/), [Leicester comparison of methods](https://leicester.figshare.com/articles/Linking_Property_Crime_Using_Offender_Crime_Scene_Behaviour_A_Comparison_of_Methods/10198388/files/18379169.pdf), [ML crime linkage review (arXiv 2411.00864)](https://arxiv.org/pdf/2411.00864)
- [NIJ: DNA solves property crimes](https://nij.ojp.gov/topics/articles/using-dna-solve-property-crimes), [Roman et al. 2009](https://link.springer.com/doi/10.1007/s11292-009-9086-4)
- [EFF on Anchorage / Draft One](https://www.eff.org/deeplinks/2025/03/anchorage-police-department-ai-generated-police-reports-dont-save-time), [KQED on the RCT](https://www.kqed.org/news/12007520)
- [Maryland RAPID/LeadsOnline](https://mdsp.prod.maryland.gov/media/537), [Raleigh case study](https://www.police1.com/police-products/software/data-information-sharing-software/articles/case-study-how-one-pd-used-an-online-tracking-system-to-recover-313k-in-stolen-merch-LqJS3ubMLY5fiak9)
- [Dallas online reporting memo](https://dallascityhall.com/government/citymanager/Documents/FY%2022-23%20Memos/3.%20Dallas%20Online%20Reporting%20System%20and%20Phone%20Reports%20Update.pdf), [Near-repeat burglary (Groff 2018)](https://popcenter.asu.edu/sites/default/files/near_repeat_burglary_groff_2018.pdf)
- [BOCSAR on clearance counting](https://www.bocsar.nsw.gov.au:443/Publications/CJB/cjb130.pdf)
- [SPD Crime Data (open data)](https://catalog.data.gov/dataset/spd-crime-data-2008-present), [SPD salary/benefits](https://seattle.gov/police/police-jobs/about-the-job/salary-and-benefits)
- [MCP 2026-07-28 spec](https://blog.modelcontextprotocol.io/posts/2026-07-28/), [MCP Apps extension](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx), [rmcp on docs.rs](https://docs.rs/crate/rmcp)
- [AWS: CJIS + Bedrock RAG](https://aws.amazon.com/blogs/publicsector/building-a-rag-architecture-on-amazon-bedrock-to-support-cjis-compliance/)
