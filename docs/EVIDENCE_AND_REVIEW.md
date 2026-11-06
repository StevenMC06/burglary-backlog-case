# Evidence rules and model review (added 2026-11-06 by Kevin's request; read before any claim, slide or cost change)

## Standing rules from Kevin
- Always check work against Guy Ernest's rubric (`docs/GUY_NOTES.md`): connectors/tools, agents, teams of agents (many-to-many), channels. Say what is missing.
- Every claim needs research + math + eval backup. Compute all scenarios and edge cases, not just base.
- Benchmarks must be relevant to this task (burglary series linkage, pawn/serial matching, auditability/replay). No generic or made-up benchmarks, no puzzle-accuracy as crime evidence.
- Honest shortfalls are rewarded. Never invent numbers outside the cost model and evals.

## Cost model results (`casefile cost`, official $3.05M baseline; targets 15% = $457.5k, +325 clearances)
| Scenario | Net % | Year-1 net | +Clearances | Payback |
|---|---|---|---|---|
| Conservative | 1.6% | -$209k | +75 | 5.3y |
| Base | 9.0% | -$5k | +316 (16.16%) | 1.0y |
| Optimistic | 15.1% | +$188k | +499 | 0.4y |

Sensitivity (scratch script, base case):
- No single lever reaches 15% net. +325 needs link hit rate 31.4% (base 30%) or conversion 51.4% (base 50%) or adoption 93%: base is on a knife edge.
- Monte Carlo over plausible ranges (20k draws): P(>=15% net) ~0.1%, P(>=+325) ~6%, P(steady net <0) ~6%, P(year-1 <0) ~90%. Median 4.5% net, +150 clearances.
- Edges: adoption 30% -> 0.5% net; Adams-style null (no packet/evidence savings) -> -5.0% net; follow-up 4h/lead -> 6.0%; conversion at today's 13% -> +75 clearances; everything bad -> 2.3% net, +84.
- Deck line to use: we probably miss 15%; say why and name the one pilot metric.

## Known flaws in `crates/core/src/cost.rs` (fix, add tests, re-run `casefile cost`, update `docs/deck/cost.json` and the deck)
1. Clearances ignore adoption: at 0% adoption the model still shows +219; with Adams-null savings still +316. Tie linkage/triage leads to adoption.
2. False leads cost nothing: halving linkage precision RAISED net. Add detective hours to dismiss false links/matches.
3. Lead double-counting: a case can be both linked and pawn-matched. Add an overlap term (halving property leads drops +316 to +267).
4. 50% conversion is unbenchmarked (today's clearance is 13%). Justify with our eval or lower it and show the sensitivity.
5. Dollars are capacity value (about 3,800 net detective hours), not cash. The deck must say so. Freed hours go to funded follow-up; no layoffs.
6. Steven's research bot says base $106.5k (3.5%) vs ours $275k (9%). Reconcile in one line.

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
