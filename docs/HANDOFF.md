# HANDOFF: resume here (paused 2026-11-05 ~10:15 PM PT, Kevin near usage limit)

**Deadline:** deck to ddolev@uw.edu by **Fri 1:00 PM**. Pitch 3:03 PM, DEM 004.

## Read first (in this order)
0. `docs/EVIDENCE_AND_REVIEW.md`: NEW 2026-11-06. Standing evidence rules, cost-model sensitivity, known model flaws to fix, gaps vs Guy's rubric. Fix the flaws before the deck numbers are final.
1. `docs/CASE_OFFICIAL.md`: official case numbers, rubric, judges, team state
2. `docs/BUILD_PLAN.md`: locked decisions and the W1–W4 worker specs
3. `docs/deck/DECK_CONTENT.md`: 9-slide content and demo voiceover script
4. `docs/GUY_NOTES.md`: technical judge profile

## Locked decisions (Kevin)
- **Direction:** C. Stage 6 linkage, fed by stage 7 pawn matching.
- **Build:** Rust.
- **Deck and voiceover:** built by Claude. Kokoro voice now (`tools/media/tts.py`; models in `media/models/`, gitignored, re-download per tts.py docstring); ElevenLabs swap optional.
- **AI outputs:** *was* "replay of real Claude runs recorded via `claude -p`, no API key". **If Kevin now has an API key, ask whether to also add live API calls** (the raw-HTTP Messages API from Rust); replay stays as the demo-safety mode either way.

## Done on `main`
- Workspace scaffold, core data contract (`crates/core/src/types.rs`), engine stubs with SPEC comments.
- Cost model on the official baseline (`casefile cost`; tests pass):
  - Base: 9.0% net savings, +316 clearances
  - Optimistic: 15.1% net savings
  - Conservative: 1.6% net savings
- Hash-chained audit log (`audit.rs`, test passes) and `store.rs`.
- Eval harness (`eval.rs`): series P/R/F1, decoys, property, extraction, and the LLM-only "wall" vs Casefile.
- Media tooling in `.venv`: kokoro-onnx, imageio-ffmpeg, playwright (uses installed Chrome), python-pptx, matplotlib. Voice samples are in `media/voice-samples/`.
- Research-bot files moved unchanged to `research-bot/`.

## Paused worker branches (WIP commits; they branched from 615af8a, so merge them, don't diff against main)

| branch | done | remaining |
|---|---|---|
| `worktree-agent-ad90695b9f8bb8e08` (W1 engine) | `linkage.rs` (~890 lines) | `verify.rs`, `solvability.rs`, `property.rs`, `citations.rs` + tests + clippy |
| `worktree-agent-a47251b990148b2fc` (W2 data) | `data/public/seattle_burglary_points.csv` (4,788 real public rows) + `SOURCE.md` | `datagen.rs` generator + tests; run it to produce `data/seed/*` |
| `worktree-agent-a02cb18e2b0417567` (W3 MCP) | `mcp.rs` + `mcp/tests.rs` (~1,350 lines) | `record.rs` (claude -p recorder), `agents/*` (6 agents + README), plus a 7th `series_llm_only` agent for the eval |
| W4 web console | nothing saved | everything in the BUILD_PLAN W4 section |

Worktrees live in `.claude/worktrees/agent-<id>`. To continue in place: `git merge <branch>` on main, or re-launch a worker on the branch.

## Remaining pipeline after the workers
1. Merge the branches.
2. `cargo run -- datagen`.
3. `cargo run -- eval --make-subsets`.
4. Record agent runs for the demo queue and heroes, plus the `series_llm_only` runs on subsets n20/n60/n150.
5. `cargo run -- eval`.
6. Playwright recording at 1280×720.
7. Kokoro voiceover and ffmpeg mux to `media/demo.mp4`.
8. Deck via the Artifact "Slides" type (type_url `https://claude.ai/artifact/8jTsAFQMFDb2oA8MsPJ2eL`), exported as PDF.
9. Optionally push to the team repo `github.com/StevenMC06/burglary-backlog-case`. It is public and Kevin has WRITE access. **Ask Kevin before pushing.**

## COST PLAN (Kevin: keep under $20, ~$10 ideal; API key billing)

**Models**
- Main session: **Sonnet 5.5**, set in `.claude/settings.local.json`.
- Workers: `build-implementer` = **Sonnet 5.5, effort medium** (project override in `.claude/agents/`).
- Use Opus only for: the skeptic recording (1–2 runs) and the 3 `series_llm_only` baseline runs.
- Never spawn `build-architect` or `build-security` (both Opus).

**Scope trims** (no loss in judged quality):
- **W1:** skip `solvability::fit()`; use hand-set documented coefficients.
- **W2:** slim generator: real Seattle points (already fetched) + templated narratives. **No Hugging Face dataset fits** (no public burglary narratives with offender ground truth, and the case bans real case data); checked 2026-11-05.
- **W3:** recorder with sequential runs (no thread pool) and one retry. The 6 agents plus `series_llm_only`.
- **W4:** build against the REAL merged engine and data (run it after W1 + W2). **No `--mock` mode.** One screen, at most 2 screenshot iterations.
- **Recordings, heroes only:**
  - extractor × 12 (Haiku)
  - triage × 4 (Sonnet)
  - series_hunter × 1 (Sonnet)
  - skeptic × 1 (Opus)
  - packet_writer × 2 (Sonnet)
  - victim_liaison × 1 (Haiku)
  - series_llm_only × 3 (Opus)
  - Always use `--system-prompt-file` (replaces Claude Code's 57k-token default prompt) and disable built-in tools.
- **Deck:** one build plus one visual QA pass.

**Order** (sequential where it avoids rework): W1 + W2 in parallel → merge → W3 + W4 in parallel → merge → datagen → recordings → eval → video → deck → `docs/MORNING_REPORT.md`.

**Budget rule:** after each phase, check spend (`/cost`). If the projected total exceeds $18, cut remaining recordings to the minimum hero set and skip the optional work. Stop and write the morning report if spend passes $20.

**Estimate:** about $14–20 expected (workers ~$9–14, main ~$4–6, recordings ~$1–3).

**"Web console"** = our product UI (the sergeant/detective dashboard), not web scraping. Scraping/browsing already works for free via the installed Playwright + Chrome; nothing to add.

## Paste this into the fresh session
> Resume from docs/HANDOFF.md and follow its COST PLAN exactly (main on Sonnet, workers = build-implementer on Sonnet medium, budget ≤ $20). Finish autonomously before 7 AM: complete W1–W4 from the paused branches, generate data, record the hero agent runs via `claude -p`, run evals, record the 75 s demo with the Kokoro voiceover, build the 9-slide deck as a Slides artifact and export a PDF, then write docs/MORNING_REPORT.md (results, spend, open questions, decisions for Kevin). Replay-only (no live API calls in the app). Don't push to GitHub or message anyone.

## Team / Discord
- Kevin was to post the "Agree C" proposal in #general, asking for votes by 9 AM. No replies yet as of 10:10 PM.
- Teammates: Steven (runs the research bot), KoolestAsian, Sonny, Gowrav.
- Read Discord with the `esec-catchup` skill (read-only MCP `discord-esec`).
