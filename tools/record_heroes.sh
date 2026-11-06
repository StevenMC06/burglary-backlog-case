#!/usr/bin/env bash
# Records the hero set of agent runs, sequentially (cost plan: heroes only). Skips existing files.
# Usage: CLAUDE_BIN=... bash tools/record_heroes.sh
set -u
cd "$(dirname "$0")/.."
P=1996; W=1992; S=1999
HP=CC-26-00$P; HW=CC-26-00$W; HS=CC-26-00$S
run() { cargo run -q -- record "$@" 2>&1 | grep -E "^recorded|rror|fail" ; }

# extractor x12 (Haiku): heroes + first queue cases
IDS=$(python -I -c "
import json
d=json.load(open('data/seed/demo.json'))
h=[d['hero_pawn']['case_id'],d['hero_weak']['case_id'],d['hero_series']['queue_case']]
ids=h+[c for c in d['queue'] if c not in h]
print(' '.join(ids[:12]))")
for id in $IDS; do run extractor "$id"; done

# triage x4 (Sonnet)
for id in $HP $HS $HW CC-26-001965; do run triage "$id"; done
# series hunter x1 then skeptic x1 (Opus) on the hero series queue case
run series_hunter "$HS"
run skeptic "$HS"
# packet writer x2, victim liaison x1
run packet_writer "$HP"
run packet_writer "$HS"
run victim_liaison "$HP"
# LLM-only wall baseline x3 (Opus)
for n in n20 n60 n150; do run series_llm_only "$n"; done
echo ALL_DONE
