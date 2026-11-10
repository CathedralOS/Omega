# Progress

Updated after the first launch wave.

Base: `8febfc42316bdcede60caa33ddd10a641f0dee8b`
Branch: `work/grok-mega-2026-11-10`
Head at list commit: `4ec72ea17fe6b7b4c053277c4c15b031a36ccad6`
Draft PR: https://github.com/CathedralOS/Omega/pull/2282
Worker model for every new worker: `composer-2.5` (fast mode off)
Peak workers running at once: 40
Launch refusals:

- `Async new-VM subagent limit of 20 reached` while starting B1-007 (`embedding.rs` lines 3001-3012). Twenty other launches in that wave were accepted.
- `Async new-VM subagent limit of 20 reached` on most refill launches after twenty more slice completions (0055, 0056, 0065, 0072, 0073, 0075–0081, 0083–0086, 0088). Three launches were accepted (0074, 0082, 0087). Recorded running count after that refill: 20. Peak recorded running count remains 40.

| Stream | Items | Done | Running | Failed |
| --- | --- | --- | --- | --- |
| A | 295 slices (1115 ACCEPT rows in 145 slices; 1076 other rows in 150 slices) | 3 | 20 | 0 |
| B | 81 file ranges (B1 26, B2 55); merge files not started | 15 | 0 | 0 |
| C | 117 chapter pieces from 103 files; lane merge not started | 0 | 0 | 0 |
| D | 412 batches, 17263 programs; D2 not started | 1 | 0 | 0 |
| E | 19 units, 591 files, 7 excluded | 0 | 0 | 0 |
| F | 28 site files, 83 occurrences | 2 | 0 | 0 |
| G | 120 shards, not started. G stays at most a quarter of the live worker count, and only when a slot is free | 0 | 0 | 0 |

Running agent ids are in `state/inflight.tsv`.

Returned and not yet copied onto the branch (text is in the coordinator transcript): `b1_001` through `b1_004`, `b1_006`, `b1_011` through `b1_014`, `b1_016` through `b1_026`, `b2_002` through `b2_005`, `b2_007` through `b2_012`, `b2_014` through `b2_017`, `b2_019`, `b2_024`, `b2_025`, `batch_0001`, `C0001`, `C0002`, `unit_001`, `unit_002`. Each ended with its completion line. Copied this wave: `b2_054`. Selected-instruction literal folds do not evaluate add, divide, or remainder. A divisor of 1 becomes a copy. A zero dividend, or a wrapping remainder by 1 or by `u64::MAX`, becomes the constant 0. There is no shift rule. Signed minimum divided by minus one has no dedicated rule. Also returned with `SLICE COMPLETE`, not yet copied: `slice_0004` through `slice_0010`. `slice_0004` closes the missing-borrow-state skip (`A0029` `CLOSED`) because each flow state is built with its borrow row. `slice_0010` marks the index-check gate and empty-machine termination rows `REACHABLE`. `slice_0016` is `SLICE COMPLETE`. Latest `SLICE COMPLETE` batch, not yet copied: `slice_0011` through `slice_0015`, `slice_0017` through `slice_0031` except slices still running. Notable `REACHABLE` rows: `A0173` folds an empty membership predicate list to `Constant(true)`; `A0225` treats signed literal `-1` as a safe structural divisor (`*value != 0`); `A0176` drops boundary scalar ensures on a structural result and later validation accepts an empty `scalar_ensures` list.
