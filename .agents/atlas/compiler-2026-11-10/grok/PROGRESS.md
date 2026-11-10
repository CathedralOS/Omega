# Progress

Updated after the first launch wave.

Base: `8febfc42316bdcede60caa33ddd10a641f0dee8b`
Branch: `work/grok-mega-2026-11-10`
Head at list commit: `4ec72ea17fe6b7b4c053277c4c15b031a36ccad6`
Draft PR: https://github.com/CathedralOS/Omega/pull/2282
Worker model for every new worker: `composer-2.5` (fast mode off)
Peak workers running at once: 20
Launch refusals:

- `Async new-VM subagent limit of 20 reached` while starting B1-007 (`embedding.rs` lines 3001-3012). Twenty other launches in that wave were accepted. The working concurrency is 20. B1-007 was not started and stays queued.

| Stream | Items | Done | Running | Failed |
| --- | --- | --- | --- | --- |
| A | 295 slices (1115 ACCEPT rows in 145 slices; 1076 other rows in 150 slices) | 3 | 7 | 0 |
| B | 81 file ranges (B1 26, B2 55); merge files not started | 13 | 13 | 0 |
| C | 117 chapter pieces from 103 files; lane merge not started | 0 | 0 | 0 |
| D | 412 batches, 17263 programs; D2 not started | 1 | 0 | 0 |
| E | 19 units, 591 files, 7 excluded | 0 | 0 | 0 |
| F | 28 site files, 83 occurrences | 2 | 0 | 0 |
| G | 120 shards, not started. A–F already have workers. G stays at most 5 while the cap is 20, and only when a slot is free | 0 | 0 | 0 |

Running agent ids are in `state/inflight.tsv`.

Returned and not yet copied onto the branch (text is in the coordinator transcript): `b1_001` through `b1_004`, `b1_006`, `b1_011` through `b1_014`, `b1_016` through `b1_026`, `b2_002` through `b2_005`, `b2_007` through `b2_012`, `b2_014` through `b2_017`, `b2_019`, `b2_024`, `b2_025`, `batch_0001`, `C0001`, `C0002`, `unit_001`, `unit_002`. Each ended with its completion line. Copied this wave: `b2_038`. In `candidates.rs`, `evaluate_integer_operation` (9111–9140) turns a `None` from exact arithmetic, division, remainder, or an exact shift into `CandidateEvaluationMismatch`. Wrapping and saturating add, subtract, and multiply return a value with no `Option` at that call. Division by zero, an out-of-range shift, and the minimum value divided by minus one stay inside `IntegerType` and are undecided here. Also returned, not yet copied: `b2_032`, `b2_033`, `b2_035` through `b2_037`, `b2_039` through `b2_042`.
