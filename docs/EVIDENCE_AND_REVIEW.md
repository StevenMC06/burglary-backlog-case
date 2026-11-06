# Evidence rules and model review (added 2026-11-06 by Kevin's request; read before any claim, slide or cost change)

## Standing rules from Kevin
- Always check work against Guy Ernest's rubric (`docs/GUY_NOTES.md`): connectors/tools, agents, teams of agents (many-to-many), channels. Say what is missing.
- Every claim needs research + math + eval backup. Compute all scenarios and edge cases, not just base.
- Benchmarks must be relevant to this task (burglary series linkage, pawn/serial matching, auditability/replay). No generic or made-up benchmarks, no puzzle-accuracy as crime evidence.
- Honest shortfalls are rewarded. Never invent numbers outside the cost model and evals.

## Current cost model (independently reproduced)

Executable output matches docs/deck/cost.json and deliverables/cost.json. Illustrative steady-state scenarios; assumptions are not measured effectiveness or cash savings.

| Scenario | Net capacity/year | Net % | Year-one net | Additional clearances | Total rate |
|---|---:|---:|---:|---:|---:|
| Conservative | $63,737 | 2.09% | -$194,555 | 44.5 | 13.45% |
| Base | $285,355 | 9.36% | +$5,613 | 262.1 | 15.62% |
| Optimistic | $469,529 | 15.39% | +$196,842 | 457.9 | 17.58% |

Base misses by $172,145 and 62.9 clearances. Conversion must be 61.56% to reach +325 holding other assumptions fixed. At four hours/completed follow-up, base net is $209,165 (6.86%). These replace the superseded 9.0%/+316 table and old sensitivities.

Adoption scaling, false-lead dismissal costs and property/linkage overlap are implemented. Remaining limitations: triage/linkage uniqueness is assumed; repeated links to the same open case need case-level deduplication; incomplete follow-up time may be omitted. Base savings rely on screening 3,400 cases from 90 to 30 minutes ($238k gross), packet preparation ($119k), and officer cataloging ($70,833). Base costs: $62k operations, $76,190 completed follow-up, $4,289 dismissals; setup $130k. All are planning estimates. Year-one benefit ramps but lead-work costs remain full-year; clearance estimates are steady-state.

Synthetic eval: 66% pairwise precision/100% pairwise recall at threshold .9; 38/40 series recovered uses separate best-group Jaccard >=.5 criterion. These do not validate assumed 80% case-level lead precision. Constructed hard-constraint decoys, generator feature overlap and small extraction samples limit generalization. No broad superiority over a single LLM is established.

## Gaps vs Guy's rubric
- Show a many-to-many agent team and at least one channel (victim SMS/voice, patrol hands-free).
- `verify_series` is the "game engine" answer: show single-LLM "wall" vs proposer + verifier on OUR eval.
- Per-case inference cost by model tier (cheapest adequate model); inference is currently an unexplained $12k/yr.
- Non-blocking approval (waits hours, not days) needs a line.

## Relevant benchmarks only
- Series linkage P/R/F1 on held-out planted series with decoys at n=20/60/150 (eval.rs). Avoid circularity: hold out features the generator used, add adversarial decoys.
- Real-data near-repeat check: the Seattle CSV has no dates. A Knox-style test needs a re-pull with offense_date from data.seattle.gov (id tazs-3rd5). Ask Kevin before pulling; verify any near-repeat citation before it goes on a slide.
- Property matching: exact-serial precision, conflicting-serial look-alikes rejected (eval.rs).
- Auditability: replay a case, get identical linkage, hash chain verifies.
