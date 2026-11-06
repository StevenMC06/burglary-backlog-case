# Seven-minute pitch and judge questions

Provisional direction: video evidence triage. Team approval is pending. Spoken copy is a rehearsal draft, not a claim that a prototype or pilot has already run. Budget a 75-second recording inside the seven minutes. Target 650–750 spoken words plus the recording, and verify timing in rehearsal.

## 0:00–0:35 — Recommendation
In Cascade City's fictional baseline, 10,000 burglaries produce only 1,300 clearances. We recommend a focused pilot that helps officers find relevant moments in evidence footage. The system proposes timestamps. An officer checks the original footage, and a detective receives a verified packet. We will show the financial limits as well as the opportunity. We recommend buying evidence about whether this workflow works before buying a citywide promise.

## 0:35–1:15 — Bottleneck
Video review consumes two officer hours per case in the supplied baseline: 20,000 hours or one million dollars annually. Detective analysis costs slightly more, at $1.05 million. We chose video because it is a large, bounded task with original footage available for verification. Actual eligibility matters: we do not assume every real burglary has useful footage. A competing property-matching approach offers a clear recovery workflow, but the case provides no separate recovery labor baseline and access to commercial data still needs validation.

## 1:15–1:45 — Workflow
Evidence staff upload footage once. The tool suggests candidate events and preserves their source timestamps. Officers accept, correct, or reject each suggestion after viewing the source. All footage remains available, including moments the model misses. Nothing identifies a suspect or makes an arrest decision. The evidence packet records the source, model version, and reviewer actions.

## 1:45–3:00 — Recorded synthetic demonstration
Show the fixture disclosure. Open the source, jump to a candidate event, reject an ordinary activity flag, find a planted missed event through full-source review, then export the human-approved packet. If model suggestions are precomputed, say so on screen and aloud. This demonstrates the review workflow, not live model detection or production accuracy. Replace this segment only with a recording of the completed, checked demo.

## 3:00–4:00 — Economics
Our base scenario assumes 80 percent of cases are eligible, 85 percent adoption, and 60 net officer minutes saved per treated case. That produces $340,000 in annual labor capacity. We reserve $68,000 of those hours for extra evidence handling. Recurring costs are $165,500, including additional detective follow-up. The result is $106,500 of steady annual economic benefit, or 3.49 percent of the baseline. Setup costs $180,000 in this planning allowance. With a 65 percent first-year ramp, year one loses $168,700. These are estimates, not quotes. Even our optimistic scenario reaches only 14.15 percent. If finance cannot identify avoidable expenditure, these hours are capacity rather than cash savings.

## 4:00–4:50 — Clearance
A 25 percent relative increase means moving from 13 percent to 16.25 percent: 325 additional clearances. One hurdle is 1,000 genuinely new lead-bearing cases, 65 percent completed follow-up, and 50 percent incremental conversion. Multiplying those gives 325. None of those conversion rates is proven. The model funds 1,300 additional detective hours, but if each completed follow-up takes four hours rather than two, base economic benefit falls to $15,500. Finding evidence sooner does not by itself establish a clearance improvement.

## 4:50–5:50 — Pilot
First, define the workflow with officers, evidence staff, security, and community input. Test synthetic packages with independently labeled full footage. Then propose a 300-case randomized agency pilot, contingent on approval. Our main metric is total human review time, including preparation, checking, corrections, fallback, and handoff. We also measure missed evidence and false positives. At 90 days, expand only if the time savings and predeclared safety requirements hold. A small pilot cannot prove the clearance target. A longer controlled rollout needs equal follow-up windows and separate citywide and eligible-case reporting.

## 5:50–7:00 — Trust and decision
No facial recognition. No neighborhood risk scores or predictive patrol allocation. Cases without cameras still receive normal investigation access. An operational deployment needs actual CJIS controls, including access, encryption responsibilities, audit records, retention, and supplier controls. A cloud label alone does not establish compliance. Every person-affecting decision remains with a human who checks the original evidence. Our request is a gated pilot with a $180,000 setup allowance and a separately stated annual operating and follow-up estimate of $165,500. We are not promising a 15 percent cash cut. We are proposing a way to test whether verified evidence review can release time and produce additional useful leads without weakening evidence quality.

# Likely judge questions

**Why propose something below the target?** The case rewards credible numbers. We tested the target instead of choosing assumptions just to reach it. The pilot can reject this approach. At optimistic coverage/adoption, the model still needs either 94.04 net minutes saved instead of 90, or $25,875 lower recurring costs, to reach 15 percent in steady economic value.

**Why should we believe 60 minutes saved?** We should not treat it as established. It is the base planning hypothesis. The randomized comparison measures the entire human workflow and includes failures and non-users.

**Why not use report writing?** The cited randomized trial did not find significant time savings. That does not prove video triage succeeds; it explains why the design requires direct workflow measurement.

**What does the prototype prove?** Only that its demonstrated workflow and audit functions run on synthetic fixtures. Precomputed suggestions do not prove live AI analysis or any detection accuracy. Runtime/cost/accuracy need a separate model evaluation.

**Does a matched object prove burglary?** No. A candidate event or stolen-property match is a lead. Identity, intent, ownership and legal authority require human investigation. Property recovery does not by itself clear the burglary.

**Are all 325 additional clearances caused by the tool?** The target pathway explicitly requires incrementality. Observed clearances include cases that would have cleared anyway. A control group is needed.

**What about the 350 lead cases without completed follow-up?** They can still consume work. Any added detective triage outside the modeled cost allowance increases costs. The two-hour completed-case assumption is uncertain and must not conceal partial-case effort.

**Why not remove staff to save cash?** This model does not recommend staffing cuts. Finance must identify genuinely avoidable expenses before claiming budget reductions. Unchanged salaries mean released capacity, not cash savings.

**What if the AI misses the crucial event?** Preserve full footage, audit with full-source ground truth, support human additions and fallback review, and agree quality gates before looking at results. If safe time savings do not exist, stop the pilot.

**How do you avoid surveillance feedback loops?** This proposal reviews lawfully obtained evidence after an incident. It does not allocate patrols or score neighborhoods. Audit unequal camera coverage and preserve investigation access for cases without video.

**Is this CJIS compliant?** No production system has been implemented or certified by this work. The proposal identifies controls for agency approval. The competition uses only synthetic/public data.

**Why not property matching as the main option?** It may produce a cleaner demo and victim benefit. But data access, match precision, additional detective work and an absent Stage 7 labor baseline make its economics less supported by the supplied case. The independent comparison remains provisional until team feedback.
