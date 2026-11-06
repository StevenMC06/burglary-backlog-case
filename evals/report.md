# Casefile evaluation (synthetic ground truth)

Dataset: 2000 cases, 1500 pawn records, 40 planted series, 10 decoys. Engine: casefile-core 0.1.0.

## 1. Series recovery (full dataset)

| stage | precision | recall | F1 | series recovered |
|---|---|---|---|---|
| engine clusters (threshold 0.9) | 63% | 100% | 77% | 38/40 |
| + verify_series pruning | 66% | 100% | 79% | 38/40 |

Runtime: 1.92 s for 2000 cases.

Operating points (engine clusters + verify pruning):

| threshold | precision | recall | F1 | series recovered |
|---|---|---|---|---|
| 0.5 | 29% | 100% | 45% | 32/40 |
| 0.6 | 30% | 100% | 46% | 32/40 |
| 0.7 | 31% | 100% | 47% | 32/40 |
| 0.8 | 39% | 100% | 56% | 33/40 |
| 0.9 | 66% | 100% | 79% | 38/40 |
| 0.95 | 83% | 96% | 89% | 35/40 |

## 2. Decoy hypotheses

10/10 decoys rejected by `verify_series` (each violates a hard constraint).

## 3. Stolen-property matching

Exact-serial leads: 100 (100 correct, precision 100%). Resurfaced items found as lead or review candidate: 113/116 (recall 97%). Conflicting-serial look-alikes rejected: 83588. Marking candidates for human review: 2825.

## 4. Extraction (Extractor agent, recorded runs: 12)

| field | accuracy |
|---|---|
| entry_method | 100% (12/12) |
| entry_point | 100% (12/12) |
| premise | 100% (12/12) |
| time_band | 58% (7/12) |
| weekday | 100% (12/12) |
| property_types (mean Jaccard) | 92% |

## 5. Single LLM vs Casefile (same cases)

| cases | single LLM (Opus, all reports in context) F1 | Casefile (engine + verify) F1 | decoy pairs accepted by LLM |
|---|---|---|---|
| 20 | 100% | 80% | 0 |
| 60 | 67% | 69% | 6 |
| 150 | 86% | 86% | 9 |

All data synthetic; planted ground truth. These numbers show the method works on data built to resemble the problem, not production accuracy.
