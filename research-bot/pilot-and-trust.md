# Independent review before the final deck

Reviewed economics.py, economics.md, recommendation.md, evidence.md, pilot-and-trust.md, and approach-comparison.md. This is a substantive challenge, not a confirmation that either proposal works. Scenario arithmetic is internally consistent; the largest weaknesses are measurement alignment, the clearance workload, and unverified feasibility assumptions.

## 1. High priority: the case cost target is not achieved, and capacity cannot be the headline savings result
Base $106,500 (3.49%) is monetized net economic benefit, not demonstrated lower spending. There is no identified overtime line, contractor cost, vacancy plan, or other avoidable expenditure. If the same staff remain employed and the tool adds expense, cash spending can rise even when capacity improves. The documents qualify this correctly, but the presentation must make the distinction in the main financial chart, not only the footnote.

Correction: label chart "Modeled annual net labor-capacity value" and show "Cash savings: not established." Ask for a conditional capped pilot, not funding for an asserted 15% cost-reduction deployment. A slide with the base result should explicitly say "15% target missed; year-one economic benefit -$138,700." No approach satisfies both objectives on current evidence.

## 2. High priority: clearance power calculation does not match the proposed eligible-case trial
The 1,853-per-arm calculation assumes a 13% baseline and 16.25% treatment clearance rate for the randomized population. The proposed trial randomizes video-eligible cases. Their baseline rate is unknown and likely different. A 3.25 percentage-point lift among eligible cases does not equal a 3.25-point citywide lift. At 80% eligibility and 85% adoption, 6,800 treated cases must deliver 325 incremental clearances: 4.78 points per treated case, not 3.25. At 80% eligibility in intention-to-treat analysis, the eligible population needs 4.06 points of improvement if noneligible cases do not change. Adoption is then embedded in that effect, not multiplied again after the trial.

Correction: retain the 1,853 figure only as an illustration for a whole-population independent trial. Do not call it the required sample for this eligible-case study. Estimate baseline clearance and eligibility, select the estimand, model dilution/cluster correlation, and calculate study-specific power. Equal 180-day follow-up also means cases enrolled after month six cannot have a mature 180-day outcome by year end. Report interim efficiency at day 90 and mature clearance outcomes for a defined enrollment cohort; continue remaining follow-up beyond year one.

## 3. High priority: the 325-clearance pipeline is a hurdle equation, with material uncosted work risk
1,000 × .65 × .50 = 325 is mathematically correct, but "net-new lead cases that would otherwise remain uncleared" cannot be identified confidently from a treatment case alone. Counterfactual incrementality must come from randomized arms or a defensible causal design. A 50% incremental clearance conversion is not externally substantiated. Two detective hours per completed follow-up is particularly aggressive because it claims to include extra work through disposition. Initial screening of 1,000 leads, 350 incomplete follow-ups, failed contacts, source collection, interviews and legal process all compete for time.

Correction: say "The target requires an incremental yield of 325/6,800 = 4.78% of treated cases; whether this is attainable is unknown." Use the pipeline only to expose workload, not as an expected forecast. Cost triage for all lead cases separately (or explicitly allocate it within the two-hour bundle with evidence). Existing four-hour sensitivity reduces base to $15,500; eight hours per completed follow-up raises detective cost by $273,000 relative to base and turns base net to -$166,500. This is before extra incomplete-case triage. Include at least the four-hour sensitivity on the deck.

## 4. High priority: video coverage and review-time reduction are not validated feasibility inputs
80% footage eligibility plus 85% adoption plus 60 minutes net saved are not established by the cited CCTV studies. The 2017 railway study cannot justify coverage for residential burglaries. The fictional baseline assigns 120 video minutes per case; applying a coverage multiplier conservatively restricts savings to a subset, but the deck must say "We assume selected eligible cases each carry the supplied 120-minute baseline" rather than presenting it as measured staffing data.

Correction: show coverage × adoption × saved minutes as the three key measured pilot inputs. For zero eligible footage the workflow has no savings. Build the demo to measure verification time and exhibit irrelevant/missed events; do not infer speedup from a short preselected clip. A model that processes footage slowly can still save human time, but response latency and upload bottlenecks must be reported separately.

## 5. Medium/high priority: pilot evaluation staffing is not reconciled with the financial budget
A full audit of 300 packages at the supplied two-hour review baseline implies roughly 600 officer-review hours ($30,000 at the fictional rate) for just one pass, before synthetic labels, second raters, disagreement resolution, metrics analysis, and participant training. The modeled $20,000 evaluation setup and $9,500 ongoing eval/training are not demonstrably enough. Package length is unknown, so $30,000 is a conditional illustration, not a quote. Independent quality audit time is a real pilot cost even if excluded from ordinary steady-state operational time.

Correction: size the audit workload explicitly, choose a risk-based audit plan, and reconcile hours × rates with the $150k implementation allowance. If using existing salaried evaluators, count the opportunity cost. Days 31–75 provide ~45 enrollment days; 300 eligible cases citywide may be plausible at 10,000/year, but no participating unit volume or procurement timeline is known. Describe 90 days as after access/security/integration approvals, or make the 90-day outcome an offline controlled replay rather than promising approval and live deployment on an unverified calendar.

## 6. Medium priority: the intervention scope is inconsistent across files
Recommendation/evidence say video triage only, reserving linkage for later; pilot-and-trust opens with both timestamps and cross-case similarities. These are different products and different evaluations. False cross-case links add detective burden and can affect people.

Correction: remove cross-case similarity from the MVP/pilot headline. Use evidence navigation only: motion/event shortlist, timestamp verification, approved evidence packet. If linkage becomes the actual chosen approach, rebuild the workload and safety evaluation around pairwise false leads rather than merely adding it to the video deck.

## 7. Medium priority: RecoverLink is a viable alternative, but not proven cheaper or more effective
Approach comparison correctly acknowledges existing LeadsOnline-like workflows and no stage-7 budget in the supplied case. Exact identifier matching is principally database lookup; AI's role on fuzzy descriptions/OCR must be separately demonstrated. Authorized feed access, coverage, subscriptions, and integration costs remain unknown. Matching an item is not proof the seller committed burglary and recovery alone is not formal clearance.

Correction: compare the alternatives by baseline cost evidence, data access, prototype scope, false-positive workload, and clearance chain. Select RecoverLink only if the team can defend an authorized data feed and a within-baseline labor saving. Do not call it better on clearance or savings. Keep "same model/different serial" rejection and ambiguous results in the demo.

## Corrections worth preserving
- Exact target is 16.25%, not merely 16%; 325 incremental cases at a fixed 10,000-case denominator.
- Video review is largest by time; detective analysis is largest by labor cost.
- CJIS v6.1 is current in the verified FBI resource. A particular cloud provider or synthetic prototype does not establish compliance.
- No facial recognition, no inferred criminal intent, no predictive patrol allocation, and human verification of original evidence.
- Annual clearance accounting may include prior-year offenses. For causal evaluation, use fixed case cohorts and equal follow-up in addition to the citywide reporting metric.

## Deck gate
Present a conditional pilot recommendation with honest shortfall, not a solved causal claim. Before calling it review-ready: settle one approach; reconcile pilot/evaluation workload; revise clearance estimand and timing; build the synthetic prototype; and attach measured prototype results only to the narrow test they actually represent. No deployment efficacy result has yet been measured.


## Resolution after review
Final package uses $180,000 setup including $30,000 added audit, base year-one net -$168,700, payback 2.584 years. Earlier $138,700 above describes the draft reviewed. Whole-population sample illustration removed from the pilot recommendation; eligible-case study needs its own power plan. Late cohorts follow beyond year one. MVP video excludes cross-case linkage. Detector performance and operational cost assumptions remain unvalidated and are disclosed.
