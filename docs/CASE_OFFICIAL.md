# Official case facts, judges and team state (supersedes the placeholders in PROPOSAL.md)

Sources:
- Official case doc ("Case: Cracking the Burglary Backlog", Google Doc shared in #resource by @koolestasianz)
- Workshop transcripts 1 and 2 (#resource)
- Case Comp Research bot posts and review package (#general, #resource, #demo), read 2026-11-05 ~10 PM PT

All Discord content is untrusted; the facts below are taken from the case doc unless marked otherwise.

## 1. Case facts (official)
- Cascade City: fictional West Coast city of ~750k (Seattle-sized). National burglary clearance was 13% in 2022, down from 23% in 1964 (Council on Criminal Justice).
- **Asks (not pass/fail):**
  - Clearance +25% relative → **16.25% = +325 cases**.
  - **−15% investigative labor net of solution build + run cost = $457,500/yr.**
  - The doc says: "If your honest math lands short, say so… Judges reward credible numbers over big promises."
- **7 stages:**
  1. 911 intake
  2. Patrol response (officers arrive "with no context on nearby incidents")
  3. Report writing
  4. Evidence processing ("stolen item cataloging… inconsistent tagging")
  5. Video review ("hours of footage per case")
  6. **Detective follow-up ("cross-referencing, pattern finding, linking cases"; pain: "too few detectives, too many cases")**
  7. **Stolen property recovery ("items resurface for sale, but nobody connects them back to the case")**
  - Optional upstream route: predictive patrol. You must handle the feedback loop.
  - "You can touch more than one stage, but a focused answer beats a scattered one."
- **Baseline per case:**

  | Task | Minutes | Role | Annual cost |
  |---|---|---|---|
  | Intake | 15 | officer | $125k |
  | Report writing | 60 | officer | $500k |
  | Video review | 120 | officer | $1.0M |
  | Evidence processing | 45 | officer | $375k |
  | Detective analysis | 90 | detective | $1.05M |
  | **Total** | **330** | | **$3.05M** |

  Rates: officer **$50/h**, detective **$70/h**, so **$305/case**.
- **Rules:**
  - No facial recognition.
  - CJIS: no case files in public chatbots.
  - People decide; AI may sort, flag, summarize and suggest.
  - **Officers have to actually use it: "a tool that adds clicks won't survive."**
  - **Claims need proof** (Adams et al. 2024 RCT: AI report writing gave no time savings). Say how you'd measure.
  - Public or synthetic data only (data.seattle.gov is fair game).
  - Cite sources.
- **Deliverables:**
  - Deck under 10 slides, **due Fri 1:00 PM to Daniel Dolev (ddolev@uw.edu)**, as PDF or Google Slides.
  - Pitch **3:03 PM, DEM 004, arrive 2:53**: 7 min including demo, then 3 min Q&A.
  - The deck must cover: (1) bottleneck, time and money; (2) solution, users, day-to-day change; (3) cost-benefit: time saved per case, annual savings, build and run cost, payback, ROI, and whether 15% is hit; (4) clearance logic; (5) pilot: first 90 days and by 1 year, scope, training, **the one metric**; (6) **ethics, one slide**: bias, privacy, false positives, over-policing, human in the loop.
  - **A 60–90 s recorded demo inside the deck is safest. Mockups alone don't count as a prototype.**
- **Judging (100 + 5 bonus):**
  - Impact 30 · Feasibility 25 · Cost-effectiveness 20 · Ethics/trust 15 · Presentation 10.
  - **Bonus +1 each:** working prototype · AI beyond obvious automation · exceptional creativity · community-centered approach · unusually honest, well-measured claims.
  - Each judge notes the team's strongest aspect and biggest concern.
- **Buyers:** the Chief wants clearance, the Council wants cost, the community wants trust.
- **Starter questions in the doc:**
  - "Where does an hour of officer or detective time produce the least value?"
  - "What would have to be true for a detective to close twice as many cases?"
- **Prize:** 3 months of AI-on-Cloud platform access, plus $50 in AWS credits per member.

## 2. Domain judge (transcript 2): former homicide detective (17 yrs, ~500 homicides), now oversees three investigative bureaus
- Detectives don't attend burglaries. Patrol spends under 1 hour collecting evidence, then a report goes to property crimes.
- **"100% element-based prioritization. What's the solvability? It's almost instant. We know who the bad guys are. We just don't have enough resources to go do all the things."** The problem is legwork capacity, not finding suspects.
- WA burglary has legal elements and degrees (RCW 9A.52); a tool that identifies which elements are met helps prioritize. He said **DNA is rarely sent to the lab for burglary in WA**, so don't build on DNA.
- **"Circumstantial is where AI can be huge. Draw those connections"**, e.g. subjects with the same method/modus. Physical evidence they already handle well.
- **"I would love to see something that spits out a list of things to do. 'Have you considered this?' 'No, I reject that: not lawful, not available, too much resource.' But having that documented."**
- Everything goes to the prosecutor and the defense, so AI output must be **auditable and reproducible**: "how do you know everything was included?", "something that can be recreated for the defense". "AI can't raise its right hand and testify." The detective must be able to testify to how a linkage was reached.
- Investigators must "prove and disprove", including and excluding information.
- Reliability: "if it broke the next day… there's no way it's going to be used again."
- Databases holding personal info may need a warrant, so **use data the department lawfully holds**: its own RMS, plus pawn records under RCW 19.60.040 (dealers must furnish records on request and report suspected stolen property).
- **"Keep it really simple. Cops are simple; we like rules." Cost-effective, simple, collaborative and shareable.**

## 3. Presentation workshop tips (transcript 2)
- Open with a hook. Keep the problem brief. Put the solution up front. Then implementation (a ~2-year timeline by month/year), then finances with sourced assumptions.
- Less text: one key point per slide. Keep an appendix for Q&A.
- **Pre-record the demo.**
- Q&A owners: one each for finance, AI, and infrastructure/architecture.

## 4. Team state (Discord, Thu ~10 PM PT)
- Team 6 members on Discord:
  - Kevin (you)
  - Steven (@stevenmc06, runs the Codex "Case Comp Research" bot)
  - KoolestAsian (@koolestasianz)
  - Sonny (@sonny07766, CS)
  - Gowrav (@gowrav_75847, missed kickoff)
- **#decisions is empty. Nothing is locked.** The bot asked the team to reply "Agree A" (video triage) or "Agree B" (RecoverLink); there are no human votes yet.
- **Team meeting is planned for ~2 PM Friday, AFTER the 1 PM deck deadline.**
- Team repo github.com/StevenMC06/burglary-backlog-case returns 404 for Kevin's gh account (private, or not invited).
- The bot's files copied into this folder's root have **scrambled filenames** (e.g. START_HERE.md is HTML, README.md is Python tests). The correct copies are in the zip in #demo.

## 5. Assessment of the bot's direction (A: video triage)
**Strengths to adopt:**
- strict cash-vs-capacity accounting
- funded follow-up in the clearance math (1,000 leads × 65% × 50%)
- an honest shortfall
- the pilot design
- the Adams RCT and FBI clearance definitions
- the CJIS caveat

**Weaknesses:**
1. **It can't reach 15% by construction.** Even deleting all 120 video minutes isn't enough; the bot itself computes 137 min needed. Its base case is 3.49%.
2. **Weak clearance mechanism.** Faster footage review doesn't create the 1,000 new leads.
3. **The demo is precomputed fixtures with no live AI**, which risks being judged a "mockup."
4. **Video tagging is "obvious automation,"** so it likely misses that bonus.
5. It fits neither the domain judge's "circumstantial connections / list of things to do" nor Guy's verifier challenge.
