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
| A | 295 slices (1115 ACCEPT rows in 145 slices; 1076 other rows in 150 slices) | 0 | 3 | 0 |
| B | 81 file ranges (B1 26, B2 55); merge files not started | 2 | 9 | 0 |
| C | 117 chapter pieces from 103 files; lane merge not started | 0 | 2 | 0 |
| D | 412 batches, 17263 programs; D2 not started | 0 | 2 | 0 |
| E | 19 units, 591 files, 7 excluded | 0 | 2 | 0 |
| F | 28 site files, 83 occurrences | 0 | 2 | 0 |
| G | 120 shards, not started. A–F already have workers. G stays at most 5 while the cap is 20, and only when a slot is free | 0 | 0 | 0 |

Running agent ids are in `state/inflight.tsv`.
