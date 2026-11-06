# Economics: fund a gated pilot, do not promise 15% budget savings

All department baseline figures come from the fictional case. Intervention assumptions are planning estimates, not measured efficacy, vendor quotes, or claims about real police departments.

## Recommendation
Base video triage produces $106,500/year net economic benefit at full adoption (3.49% of baseline), and a $168,700 first-year loss after build costs and ramp-up. Even the optimistic scenario reaches only 14.15% steady-state savings. No scenario meets the 15% joint cost target when incremental follow-up is funded. Pitch this shortfall honestly.

Monetized hours are capacity, not demonstrated cash savings. To claim budget reduction, identify overtime, contractor expense, or another avoidable expenditure. If salaries remain unchanged, report redeployed capacity and incremental expense. Finance must validate realizable savings before expansion.

## Formulas and assumptions
Treated cases = 10,000 × eligible fraction × adoption among eligible cases.
Net minutes saved = baseline 120 video-review minutes minus all post-tool processing, verification, correction, and documentation. An additional fraction of saved officer time is reserved for extra evidence handling; exclude this from financial benefits.
Gross labor value = treated cases × net minutes saved / 60 × $50.
Releasable value = gross value × (1 − officer reserve).
Steady net economic benefit = releasable value − annual recurring costs INCLUDING new detective follow-up.
Year-one net = releasable value × first-year ramp − full annual recurring costs − one-time implementation.

| Scenario | Eligible | Adoption | Net minutes saved | Treated cases | Officer reserve | Year-one ramp |
|---|---:|---:|---:|---:|---:|---:|
| Conservative | 60% | 70% | 40 | 4,200 | 20% | 50% |
| Base | 80% | 85% | 60 | 6,800 | 20% | 65% |
| Optimistic | 90% | 95% | 90 | 8,550 | 10% | 75% |

One-time build/training = $180,000 in each scenario. Ramp is relative to each scenario's steady adoption, avoiding double application. Full recurring costs in year one are conservative. Scenario brackets are not confidence intervals. Optimistic combines strong performance, adoption, and low costs and must not be called expected.

| Scenario | Gross labor value | Reserved officer hours | Annual recurring | Steady net | Year-one net | Year-one economic ROI | Minutes needed for steady 15% target |
|---|---:|---:|---:|---:|---:|---:|---:|
| Conservative | $140,000 | 560.0 | $195,500 | $-83,500 (-2.74%) | $-319,500 (-10.48%) | -85.09% | 233.21 |
| Base | $340,000 | 1,360.0 | $165,500 | $106,500 (3.49%) | $-168,700 (-5.53%) | -48.83% | 137.43 |
| Optimistic | $641,250 | 1,282.5 | $145,500 | $431,625 (14.15%) | $107,344 (3.52%) | 32.98% | 94.04 |

## Estimated costs
One-time $180,000: full-source missed-event audit $30,000 plus integration/workflow $80,000; security/privacy setup $30,000; evaluation setup $20,000; training/change management $20,000. This is a production pilot allowance, not the much smaller competition prototype. Actual procurement quotes remain necessary.

Base recurring $165,500: compute/storage/model services $30,000; integration support $20,000; security/audit support $15,000; ongoing evaluations/training $9,500; incremental detective follow-up $91,000.
Conservative other operations $104,500 plus follow-up $91,000 = $195,500. Optimistic other operations $54,500 plus follow-up $91,000 = $145,500. All estimates include an allowance, not proof of CJIS compliance. Check retention, footage volume, licensing, labor overhead, and procurement in quotes; case hourly rates may omit real-world overhead.

## Target thresholds, ROI, payback
At base coverage/adoption, reserve, and costs, the 15% target requires 137.43 minutes saved per treated case, exceeding the 120-minute video baseline. Video alone cannot meet the target with these inputs, even at zero residual review time.
Optimistic requires 94.04 minutes saved, compared with its assumed 90. Equivalently, at optimistic benefit, recurring costs must be no more than $119,625 to hit the target; modeled $145,500 is $25,875 too high.

Year-one ROI = (ramped releasable labor value − recurring − build) / (recurring + build). This is economic ROI, not cash return.
Base payback: year one ends $168,700 unrecovered; at $106,500 full-adoption annual net benefit thereafter, payback = 1 + 168,700/106,500 = 2.58 years (~31.0 months from launch). Conservative never pays back under fixed inputs. Optimistic repays within year one; do not name an exact month without a monthly expense/adoption schedule.

## Conditional clearance hurdle, not a forecast
13% ×1.25 =16.25%; 1,625 total annual clearances, 325 additional versus 1,300 baseline.

1,000 net-new verified lead cases ×65% completed follow-up ×50% incremental clearance conversion =325 additional clearances. These are unproven assumptions. Leads must be case-level and deduplicated, exclude leads that would arise anyway, and incremental clearance must exclude cases that would clear under standard practice. A controlled comparison is necessary.

1,000 lead cases represent 10% of all cases or 14.7% of the 6,800 base treated cases. Completed follow-ups =650. At 2 detective hours per completed follow-up INCLUDING all extra work through closure, workload =1,300 hours ×$70 =$91,000, included above. Two hours is uncertain and may be too low. At 4 hours, detective expense doubles to $182,000: subtract another $91,000 from every annual net scenario; base becomes $15,500 (0.51%) and optimistic $340,625 (11.17%).

The base reserves 1,360 officer hours for incremental evidence handling/verification (81.6 minutes per new lead case at 1,000 leads). These hours are NOT savings and do NOT substitute for detective hours. Work on the 350 lead cases that do not complete follow-up must fit within the modeled reserve/cost allowances; extra detective triage beyond the two-hour bundle is an additional cost sensitivity, not free labor.

At 25% conversion rather than50%, the same650 completed follow-ups yield162.5 expected incremental clearances (14.625% overall clearance), not325. Clearance, arrest, prosecution, and conviction are distinct.

## Presentation sentence
“We recommend a measured video-triage pilot. Our base case releases $340,000 in annual officer labor capacity. We reserve $68,000 of that capacity for evidence handling, and budget $165,500 in recurring costs including extra detective work, leaving $106,500 in net economic benefit. We do not claim the15% target is established. The clearance target requires325 additional outcomes that must be demonstrated in a controlled pilot.”

## Repeatability
Run `python3 economics.py` to regenerate scenarios.csv and arithmetic assertions. CSV values are unrounded. Baseline arithmetic uses supplied fictional case data. No external pricing or efficacy claim is made.

## Additional independent-review audit allowance
The added one-time $30,000 covers 300 packages × assumed 120 minutes of footage/package × $50/hour for exhaustive missed-event review. Footage duration is an explicit assumption: the case provides labor-review time, not raw footage duration. This adds to, rather than replaces, the original $20,000 evaluation setup. At eight incremental detective hours per completed follow-up, cost is $364,000/year: base steady net becomes −$166,500.
