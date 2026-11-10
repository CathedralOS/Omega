# Progress

Returned worker files were copied from the coordinator transcript onto this branch. The five workers that were still marked running have finished and their files are on the branch. Refused slices stay unstarted until a launch is accepted.

Base: `8febfc42316bdcede60caa33ddd10a641f0dee8b`
Branch: `work/grok-mega-2026-11-10`
Draft PR: https://github.com/CathedralOS/Omega/pull/2282
Worker model for every new worker: `composer-2.5` (fast mode off)
Peak workers running at once: 40
Launch refusals:

- `Async new-VM subagent limit of 20 reached` while starting B1-007 (`embedding.rs` lines 3001-3012). Twenty other launches in that wave were accepted.
- `Async new-VM subagent limit of 20 reached` on most refill launches after twenty more slice completions (0055, 0056, 0065, 0072, 0073, 0075–0081, 0083–0086, 0088). Three launches were accepted (0074, 0082, 0087). Recorded running count after that refill: 20. Peak recorded running count remains 40.
- `Async new-VM subagent limit of 20 reached` on the next refill after the recovered notes landed. Twenty launches were accepted (0055, 0056, 0065, 0072, 0073, 0075–0081, 0083–0086, 0088–0091). The following twenty were refused (0092–0111). Running count after that wave: 20. Peak recorded running count remains 40.
- `Async new-VM subagent limit of 20 reached` while refilling after slices 0065, 0073, and 0077 finished. Eighteen launches were accepted (0092–0105, 0107–0109, 0111). Slices 0106 and 0110 were refused in that wave, and a retry of 0106, 0110, 0112, 0113, and 0114 was refused with the same text. Running count after that refill: 35. Peak recorded running count remains 40.
- `Async new-VM subagent limit of 20 reached` while refilling after seventeen slices finished (0055, 0056, 0072, 0075, 0076, 0078–0081, 0083–0086, 0088–0091). Eighteen later launches were accepted (0106, 0110, 0112–0127). Slice 0128 was refused with that text. Running count after that refill: 36. Peak recorded running count remains 40.
- `Async new-VM subagent limit of 20 reached` after the next wave of ACCEPT slices 0128–0145 and two spec chapters (C0003, C0004) were accepted. C0005–C0010 were refused with that text. Running count: 20. Peak recorded running count remains 40.
- `Async new-VM subagent limit of 20 reached` after ACCEPT slices 0128–0145 and chapters C0003–C0004 landed. Twenty spec chapters C0005–C0024 were accepted. C0025–C0030 were refused with that text. Running count: 20. Peak recorded running count remains 40.
- C0008 returned `CHAPTER COMPLETE` on a stub that pointed at a file the worker did not include. The stub was not kept. C0036 and C0044 were refused with `Async new-VM subagent limit of 20 reached` while later launches in the same wave were accepted. Peak recorded running count remains 40.
- `Async new-VM subagent limit of 20 reached` for C0062–C0066 after C0059–C0061 were accepted. A retry of C0062 was refused with the same text.
- `Async new-VM subagent limit of 20 reached` for C0081, C0082, and C0084–C0091 after C0062–C0080 and C0083 were accepted. A retry of C0082 was accepted. Retries of C0081 and C0084 were refused with the same text. Running count after that refill: 39. Peak recorded running count remains 40.
- C0008 again returned a pointer instead of the rules. C0076 returned a summary that pointed at a file. Both returns were discarded and the chapters were relaunched. `Async new-VM subagent limit of 20 reached` for C0100–C0103 and C0105–C0106 after C0097–C0099 and C0104 were accepted. A retry of C0100 was refused with the same text. Running count: 35. Peak recorded running count remains 40.

| Stream | Items | Done | Running | Failed |
| --- | --- | --- | --- | --- |
| A | 295 slices (1115 ACCEPT rows in 145 slices; 1076 other rows in 150 slices) | 145 | 0 | 0 |
| B | 81 file ranges (B1 26, B2 55); merge files not started | 81 | 0 | 0 |
| C | 117 chapter pieces from 103 files; lane merge not started | 65 | 35 | 0 |
| D | 412 batches, 17263 programs; D2 not started | 2 | 0 | 0 |
| E | 19 units, 591 files, 7 excluded | 2 | 0 | 0 |
| F | 28 site files, 83 occurrences | 2 | 0 | 0 |
| G | 120 shards, not started. G stays at most a quarter of the live worker count, and only when a slot is free | 0 | 0 | 0 |

ACCEPT slices 0001–0145 are on the branch. Sixty-five spec chapters are on the branch. C0008 and the first proof-contracts slice are being rerun after pointer returns and are not on the branch. Next unstarted spec chapter is C0100. Running count: 35. Peak recorded running count remains 40.

`B/notes/b2_031.md` was returned without `QUESTION COMPLETE`. The note ends in the identity-patch table. Do not treat that range as finished.

Stream B notes are on the branch, including the full worker text for `b2_038`, `b2_044`, and `b2_054`. Selected-instruction literal folds do not evaluate add, divide, or remainder. A divisor of 1 becomes a copy. A zero dividend, or a wrapping remainder by 1 or by `u64::MAX`, becomes the constant 0. There is no shift rule. Signed minimum divided by minus one has no dedicated rule.

Newest copied ACCEPT slices through `slice_0071`, except 0041, 0053, 0055, 0056, and 0065. `slice_0048` is entirely `UNDECIDED` (late `calls.rs` ranges, no acceptance witness). Contradictory hypotheses make a logical-not fact `Proven` without judging the operand (`A0411`) and make `premises_prove` return true (`A0415`). `validate` can return `Ok` while exact-overflow errors sit only in `deferred_overflow` (`A0517`). A longer static write path is forced non-overlapping before the general overlap walk (`A0507`). User-written `&` and `<<` skip the spelled-builtin check (`A0524`).
