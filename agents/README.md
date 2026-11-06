# Agents

Each folder holds one agent run by `casefile record <agent> <input-id>` through `claude -p`:

- `agent.json`: `model`, `allowed_tools` (bare MCP tool names from `casefile mcp`, exposed as `mcp__casefile__<name>`; empty means no MCP server and no tools), `input` (what is inlined into the prompt).
- `system.md`: system prompt. Cases not people; AI suggests, humans decide; citations as `[CASE_ID:field]`; no suspect names.
- `schema.json`: JSON schema for the structured output (`additionalProperties: false`, `Mo` enums match `crates/core/src/types.rs`).

Built-in Claude tools are always disabled (`--tools ""`).

| agent | model | tools | input-id | output |
|---|---|---|---|---|
| extractor | haiku | none | case id | `{mo, items, notes}` |
| triage | sonnet | score_solvability, similar_cases, pawn_lookup, get_case | case id | `{disposition, rationale_with_citations, next_steps}` |
| series_hunter | sonnet | propose_clusters, similar_cases, verify_series, get_case | seed case id | `{hypotheses}` |
| skeptic | opus | verify_series, get_case, similar_cases | same id as the series_hunter recording it reviews | `{verdicts}` |
| packet_writer | sonnet | get_case, pawn_lookup, verify_series, check_citations | case id | `{packet_markdown, citations_ok}` |
| victim_liaison | haiku | get_case, draft_victim_update | case id | `{language, message, plain_english_summary}` |
| series_llm_only | opus | none | subset name in `data/seed/eval_subsets.json` | `{series, hypotheses}` |

`input` kinds: `case_narrative`, `case_id`, `subset_narratives`, `recording:<agent>`.

`series_llm_only` is the LLM-only baseline for the eval ("the wall"): all narratives of a subset inlined, no tools. `eval.rs` reads its `series` field (array of case-id arrays).
