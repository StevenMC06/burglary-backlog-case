# Review safeguards and actual architecture

## Flow
Synthetic seed records -> Store -> deterministic linkage/property/verifier tools -> web API -> console -> human demo decision -> backend validation -> tamper-evident audit file.

Separately, `casefile record` runs bounded Claude agents through MCP tools, saving structured outputs, raw events, model, tokens, duration and cost. Store overlays recorded extractor MOs. The web console combines engine results and saved agent outputs; replay pacing is illustrative, not a live end-to-end agent run. Ground truth is used for evaluation, not returned by console endpoints.

## Enforced boundaries
The decision endpoint allows only existing console actions, requires known case/series subjects, recomputes series verification using server-owned membership, and refuses failed-series approval even if the request claims pass. Rejections require a recognized reason; Other requires a nonblank note. Series audit membership/verdict are canonicalized from server results. Invalid requests return 400 without an audit append. Existing request and success response shapes remain unchanged.

Audit readers reject malformed lines and non-missing-file read errors. Verification checks contiguous sequences, previous hashes and entry hashes. Append verifies the existing chain before writing and refuses corruption rather than repairing or skipping it. The audit API reports failures; MCP approval lookup verifies hashes before trusting decisions. Ticket creation preflights audit integrity/capacity before persisting a ticket; cross-file writes remain nontransactional if a later I/O failure occurs. Unterminated final entries are readable but block appends to avoid concatenating JSON.

## Before / after regression checks
- Forged passing verdict for H-DECOY: previously accepted by the handler; now rejected without append.
- Unknown action/subject or missing rejection reason: previously accepted if nonblank; now rejected.
- Malformed final/interior audit line: previously skipped; now reported and append refused.
- Rehashed entry with discontinuous sequence: previously accepted; now rejected.
- Valid accept/reject/victim approval: still succeeds, one append per call.

## Limits and ownership
`actor` is a caller-supplied DEMO identity, not an authenticated officer. No external action or message is performed by these approvals. Hash chaining is tamper-evident, not immutable: tail deletion or full rewriting cannot be detected without an external trusted checkpoint. Cross-process audit writers are not coordinated; the web handler mutex only serializes decisions in that server process. Do not claim production audit storage or compliance.

Algorithm and UI work remain with their existing owners. The core verifier still needs distinct-member validation; its MCP wrapper already rejects fewer than two IDs. This branch does not change the algorithm, model, financial claims, or synthetic evaluation efficacy.

## Independent review and repeatable checks
Two independent reviewers inspected approval handling and audit/MCP integration. Their findings led to canonical series audit fields, hash verification before status lookup, safe refusal of unterminated logs, and ticket preflight.

Verified: cargo build --offline; cargo test --offline --quiet (41 binary +49 core tests); cargo clippy --offline --all-targets -- -D warnings; python3 tools/review/check_safeguards.py (isolated real HTTP server, synthetic data); git diff --check.

Step decisions now require an exact current step/source match and preserve the canonical recommendation snapshot. Duplicate matches fail closed. The API does not yet use versioned recommendation IDs. No production authentication, immutable storage, or cross-process transaction guarantee is claimed.
