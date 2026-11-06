# Deck content v1 (9 slides, 7:00 incl. 75 s demo) — source of truth for the Slides artifact

Numbers: `cargo run -- cost` (official baseline) and `cargo run -- eval` (synthetic ground truth). {EVAL_*} = filled after evals run.
Rubric map: Impact 30 (S2, S6, S7) · Feasibility 25 (S3, S5, S8) · Cost-effectiveness 20 (S6) · Ethics 15 (S9) · Presentation 10 · Bonus: prototype (S4), AI beyond obvious automation (S5 verifier), creativity (S1/S5), community (S3 victim updates + property returned), honest claims (S6/S7 shortfall shown).

## S1 — Hook (0:00–0:30)
Title: **10,000 burglaries. A few hundred burglars.**
Sub: Casefile connects the cases so detectives can close them. AI suggests · a rules engine verifies · people decide.
Visual: 10,000 dots collapsing into ~40 clusters.
Notes: "Cascade City logged 10,000 burglaries and cleared 1,300. But burglary isn't 10,000 separate mysteries — research and every detective we know say a small number of repeat offenders commit a large share. The department's own words: too few detectives, too many cases. Our recommendation is to stop treating them as 10,000 cases."

## S2 — The bottleneck: Stage 6, detective follow-up (0:30–1:15)
Title: **The bottleneck is connecting cases, not collecting them**
Left: official per-case baseline bar (intake 15 · report 60 · video 120 · evidence 45 officer-min; detective 90 min = **$1.05M, the largest single cost**). $305/case, $3.05M/yr.
Right, 3 proof points:
- ¾ of burglary investigations are suspended within 2 days for "no leads" (Eck, PERF 1983, 3,000+ cases).
- Clearance counts **offenses**: one arrest can clear every linked burglary (FBI UCR clearance rules).
- "We know who the bad guys are. We just don't have enough resources." — the domain expert at our kickoff.
Answer to the case's question "where does an hour produce the least value?": a detective re-reading a no-lead report and manually cross-referencing other reports.

## S3 — What changes on Monday morning (1:15–1:45)
Title: **One screen for the sergeant and the detective**
Before → After (3 rows, icons):
- Sergeant: reads 36 overnight reports one by one → opens a queue already structured, scored and cross-referenced; screens out no-lead cases in minutes with a signed packet.
- Detective: hunts through RMS and pawn records → receives a verified lead packet: linked cases, pawn hits, a next-steps checklist to accept or reject.
- Victim: hears nothing for weeks → gets a plain-language update in their language, approved by staff; property comes home when a pawn hit is confirmed.
Footer: Stage 7 property matching feeds Stage 6 (pawn records are furnished to police under RCW 19.60.040).

## S4 — Demo (1:45–3:05) — 75 s recorded video
Title: **Live from the prototype** (synthetic data, real Rust engine, recorded Claude runs)
Video: media/demo.mp4 (voiceover by script below).

## S5 — Architecture: AI proposes, the engine verifies (3:05–4:05)
Title: **AI proposes. The engine verifies. A detective decides.**
Diagram (Guy's four layers):
- Connectors (MCP server, Rust): records · pawn feed · linkage engine `verify_series` · solvability · citation checker · approval queue · cost model.
- Agents (right-sized models): Extractor (Haiku) · Triage (Sonnet) · Series Hunter (Sonnet, nightly batch) · Skeptic (Opus) · Packet Writer (Sonnet) · Victim Liaison (Haiku).
- Teams: Intake & Triage (per report) · Series Hunting (nightly: hunter ⇄ skeptic ⇄ engine) · Victim Comms (on status change).
- Channels: sergeant console · detective packet · victim text/email (any language) · chief KPI dashboard.
Callout: "The game engine for investigations": could one person be at both scenes? same MO? serial matches? — deterministic yes/no with reasons, reproducible for prosecutor and defense.
Proof chart: series found correctly — single LLM vs engine + agents, at 20 / 60 / 150 cases. {EVAL_WALL}

## S6 — Cost-benefit on the official baseline (4:05–4:50)
Title: **Base case: 9% net labor savings, payback in about a year**
Big numbers (Base): $428k capacity freed/yr · −$62k run cost · −$91k funded follow-up of new leads · **$275k net = 9.0%** · setup $130k · year-one ≈ break-even (−$5k) · payback ≈ 1.0 yr.
Scenario strip: Conservative 1.6% · Base 9.0% · Optimistic 15.1% (meets 15%).
Honest line: "We hit 15% only in the optimistic case. Savings are freed detective capacity; cash savings come from overtime and not back-filling vacancies — finance must confirm."
Time saved per case (base): detective −36 min avg (screened cases 90→30; others −20), officer evidence cataloging −10 min.

## S7 — Clearance logic (4:50–5:30)
Title: **How 13% becomes ~16%: new leads, worked by real detectives**
Waterfall: 585 linked open cases surfaced after arrests (1,300 × 30% × 1.5) + 298 pawn/serial hits (10,000 × 35% traceable × 10% resurface × 85%) + 120 moderate cases newly worked = **1,003 new verified leads** × 65% completed × 50% clear − 10 lost to screening = **+316 → 16.16%** (target 16.25%).
Note: leads are case-level and deduplicated; follow-up hours are budgeted as cost (S6), not counted as savings. Every factor is an assumption the pilot measures.

## S8 — Pilot: 90 days, then 1 year (5:30–6:10)
Title: **Prove it on one precinct before scaling**
- Days 0–30: integrate RMS export + pawn feed in a CJIS-approved environment; 90-min training for 1 sergeant + 6 detectives; replay 6 months of closed cases (ground truth) to measure link precision.
- Days 31–90: randomized pilot — half of new burglaries get Casefile packets, half business-as-usual.
- Month 4–12: expand to all precincts if gates pass; quarterly equity and accuracy audits.
**The one metric: cleared burglaries per detective-hour (pilot vs control).** It captures both asks at once.
Gates: linked-case precision ≥ 90% on replay; zero un-reviewed actions; no drop in clearance on screened cases.

## S9 — Ethics & trust + the ask (6:10–7:00)
Title: **Cases, not people. Humans decide. Everything is auditable.**
- Bias & over-policing: links cases, never scores people or neighbourhoods; no predictive patrol; equity monitor tracks screening rates by neighbourhood.
- Privacy: CJIS-approved hosting; only data the department lawfully holds (RMS, RCW 19.60.040 pawn records); no facial recognition; synthetic data in this prototype.
- False positives: skeptic agent + rules engine reject {EVAL_DECOYS}; a pawn serial match is a lead, never ownership or guilt.
- Human in the loop: no send/assign/alert action exists without a person's click; hash-chained audit log; outputs reproducible for prosecutor and defense.
Ask: **Approve a 90-day, one-precinct pilot: $130k setup, $62k/yr to run.**

## Appendix (Q&A)
A1 assumptions table (all scenario inputs) · A2 sources · A3 agent cards (instructions/model/tools/test) · A4 why not video triage / AI report writing (Adams RCT) · A5 eval details.

---

# Demo voiceover script v1 (~185 words, ~75 s) — refine timing to the real hero data
1. It's 7 AM in Cascade City's property-crimes unit. Thirty-six burglaries came in overnight.
2. Casefile has already read every report. A small, fast model pulls out how each break-in happened, and what was taken, down to serial numbers.
3. This one has a pawn hit: the stolen laptop's serial number turned up at a pawn shop three days later. It's flagged as a lead, not proof. And a look-alike with a different serial is rejected.
4. Overnight, an agent proposed that seven burglaries are one offender: rear sliding doors, pried open midday, laptops and jewelry.
5. Then the rules engine checks it, like a referee. One case is ruled out: same time, nine kilometres away. Nobody can be in two places at once.
6. The detective gets a lead packet where every claim links back to a record, plus a to-do list they can accept, or reject with a reason.
7. Nothing happens without a person: assignments, alerts, even a Spanish update to the victim. Every click lands in a tamper-evident audit log.
8. AI suggests. The engine verifies. People decide.
