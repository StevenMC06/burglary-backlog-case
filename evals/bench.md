# Engine benchmark (`casefile bench`)

Seed dataset tiled into k one-year copies. Median of 3 runs, release build, this machine; "same" = optimised output equals the brute-force reference exactly. Threshold 0.9.

| k | cases | pawn | step | reference ms | optimised ms | speed-up | same |
|---|---|---|---|---|---|---|---|
| 1 | 2000 | 1500 | propose_clusters | 221.8 | 13.5 | 16.5x | yes |
| 1 | 2000 | 1500 | all_hits (pawn matching) | 5260.0 | 98.2 | 53.6x | yes |
| 1 | 2000 | 1500 | similar_cases k=5 (50 probes) | 24.2 | 12.3 | 2.0x | yes |
| 2 | 4000 | 3000 | propose_clusters | 554.3 | 36.9 | 15.0x | yes |
| 2 | 4000 | 3000 | all_hits (pawn matching) | 21265.8 | 367.1 | 57.9x | yes |
| 2 | 4000 | 3000 | similar_cases k=5 (50 probes) | 42.5 | 20.2 | 2.1x | yes |
| 4 | 8000 | 6000 | propose_clusters | 1109.7 | 61.8 | 18.0x | yes |
| 4 | 8000 | 6000 | all_hits (pawn matching) | 86401.8 | 1795.5 | 48.1x | yes |
| 4 | 8000 | 6000 | similar_cases k=5 (50 probes) | 105.6 | 37.0 | 2.9x | yes |

Pair funnel on the seed dataset (k=1): how many pairs each stage of propose_clusters still has to look at.

| threshold | all pairs | in 180-day window | in 30 km box | pass upper bound | edges |
|---|---|---|---|---|---|
| 0.5 | 1999000 | 1461811 | 1461811 | 594758 | 6309 |
| 0.9 | 1999000 | 1461811 | 1461811 | 68781 | 761 |

Why it is faster, with no change to results:
- propose_clusters: time-sorted sweep (only pairs inside the 180-day window are visited), an exact score upper bound (full MO agreement) rejects pairs before MO work, MO features precomputed as a bitmask and interned signature ids (no per-pair allocation), work split across threads.
- all_hits: pawn records normalised once and indexed by category, serial and marking; an item is compared only with records that can match it under the rules.
- similar_cases: bounded min-heap top-k (O(n log k)) with the same upper bound used as a branch-and-bound cut against the current k-th best.

