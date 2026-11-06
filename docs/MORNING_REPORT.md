# Morning report (2026-11-06)

Deck due Fri 1:00 PM to ddolev@uw.edu. Pitch 3:03 PM, DEM 004. **Nothing has been pushed, sent or submitted.**

## Deliverables
| item | path |
|---|---|
| Deck (9 slides, PDF) | `docs/deck/casefile_deck.pdf` (HTML source `casefile_deck.html`; rebuild: `.venv/Scripts/python tools/deck/build_deck.py`) |
| Demo video, 63 s, Kokoro voiceover | `media/demo.mp4` (script `tools/media/demo_script.json`, recorder `tools/media/record_demo.py`) |
| Web console | `cargo run -- serve` then http://localhost:8080 |
| Eval report | `evals/report.md`, `evals/results.json` |
| Cost model | `cargo run -- cost` -> `docs/deck/cost.json` |

Checks: `cargo build`, `cargo test` (38 + 49 pass), `cargo clippy --all-targets -- -D warnings` all pass.
Recording spend: $1.34 (hero runs only, via `claude -p`).

## What changed per EVIDENCE_AND_REVIEW.md
- `cost.rs`: leads now scale with adoption; false leads cost detective time; property/linkage overlap removed. 5 tests.
- Result (Base): **9.4% net, +262 clearances (15.6%)**. Misses both targets. Conservative 2.1% / +45. Optimistic 15.4% / +458 (only one that meets them).
- The old "knife edge at +316" is gone: after the fixes Base is 63 clearances short, not 9. The deck says we probably miss 15%.
- Monte Carlo numbers in the review were from the pre-fix model, so they are NOT on any slide.

## Honest eval findings (all synthetic; planted ground truth)
- Series pair precision 66% / recall 100% at threshold 0.9 (38/40 series). Default 0.6 gave only 29% precision; 0.9 was picked from a sweep on the same data, so it is optimistic. Sweep is in the report.
- Decoys: 10/10 rejected. Fixed a datagen flaw first: the original spatiotemporal decoys used 6-10 h windows, which one offender can legitimately cover, so the engine was right to pass them.
- **Single Opus pass matches Casefile on F1** (100/67/86% vs 80/69/86% at n=20/60/150, one run each). The real differences: it accepted 6 and 9 decoy pairs, and 2,000 cases would be ~0.7M tokens. The slide says this plainly; do not claim an F1 win.
- Pawn: 100/100 exact-serial leads correct; 113/116 resurfaced items found as lead or candidate.
- Extractor (12 runs): 100% on entry/premise/weekday, **58% on time band**.
- Hero decoy hard-fails with a precise reason (15.9 km apart, 30 min window, 34 min needed).

## Rubric gaps still open (say so if asked)
- No patrol hands-free channel (victim message draft is the one built channel). Slide says "not built".
- `/api/replay` returns a JSON timeline, not SSE (SSE needs one new crate: tokio-stream or futures-core; your call).
- Cost model assumptions `lead_precision` (80%), `dismiss_min` (20), `overlap_share` (20%) are stated assumptions; our eval measured 66% pair precision on synthetic data.
- 50% conversion remains unbenchmarked; sensitivity shown (Conservative uses 35%).
- Real-data near-repeat check needs a re-pull with offense dates from data.seattle.gov: **needs your OK**, not done.
- Series are ~10% of cases, not the ~35% in the spec (40 series of 2-12 cases and 35% conflict); sizes kept.
- Workshop claims on Washington burglary law/DNA are unverified and kept off slides.
- Slide 6 reconciles Steven's bot (3.5%) in one line; its figures come from the Discord ledger, not recomputed here.

## Needs Kevin
1. Review the deck PDF and video (watch for any claim you do not want to defend).
2. OK to push to `github.com/StevenMC06/burglary-backlog-case`? (not pushed)
3. OK to re-pull Seattle data with dates for a near-repeat check? (not pulled)
4. Submit the deck to ddolev@uw.edu yourself before 1:00 PM.
