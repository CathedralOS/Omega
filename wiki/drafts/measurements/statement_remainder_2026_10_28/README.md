# Index equality endpoints once per closure

ITERATION-PERFORMANCE, fixed base
`cb77f31d5eff1d3dbde5a0a59b5b45162d6edf58`. Continue the
[difference-constraint experiment](../contract_preparation_2026_10_28/README.md)
in the same detached worktree. **Retain locally and continue.** All three
comparison medians improve about 6%, with overlapping ranges. This supports a
modest candidate, not the requested substantial win or a broad performance claim.

## Mechanism and scope

The [first profile](profile.json) finds subtraction down to 15.87 ms; it no
longer justifies the next optimization. The [detail profile](detail-profile.json)
finds equality seeding at 186.58 ms versus 5.81 ms collecting terms and 10.73 ms
propagating closure. Each endpoint pair looked up the same symbolic operands in
the immutable graph. Resolve every endpoint once and read the existing indexed
query in that pair loop. No new identity, equality, ordering edge, work budget,
closure algorithm, lifetime, source syntax or public API is introduced.
All pairs and comparison-budget debits remain in their original order.
The existing symbolic entry point retains its identical-value and absent-vertex
behavior. Numeric indices are private and belong to this exact borrowed graph.

The source trace and reused primary research are in [research](research.md).
Temporary diagnostic patches were removed before building the candidate;
[candidate.patch](candidate.patch) contains implementation and test changes.
Diagnostic patches are zero-context; replay with `git apply --unidiff-zero`.
Use LF patch files on Windows. Reverse application was checked after normalizing
line endings; no diagnostic scopes remain in production.

## Results

Four measured unprofiled observations per producer and subject, after warmups,
interleaved ABBAABBA. Median [minimum, maximum], milliseconds:

| Workload | Baseline compile | Candidate compile | Baseline checking | Candidate checking |
| --- | ---: | ---: | ---: | ---: |
| Stack32 other_machine | 6557 [5809, 7578] | 6175 [5463, 6942] | 2431 [2216, 2658] | 2181 [1858, 2461] |
| Length local | 3186 [3059, 3275] | 2996 [2880, 3078] | 1587 [1549, 1706] | 1564 [1516, 1602] |
| Dispatch match_join | 2539 [2387, 2840] | 2395 [2366, 2400] | 1300 [1208, 1451] | 1208 [1190, 1217] |

The stack32 whole-compile median is 5.8% lower and checking 10.3% lower. The
separate profile pair reduces statement-validation scopes 1099 to 653 ms, but
adjacent call checking also varies 344 to 255 ms. Those scopes are inclusive,
not disjoint wall costs; host variability prevents assigning all that delta to
endpoint indexing. Do not multiply this iteration's median change by an earlier
experiment and call it a measured cumulative gain. No valid timing was discarded.

[All 36 runs](runs.json), [summary](summary.json), and [exact metadata](metadata.json)
retain timings, source/build hashes and optimized producer profiles. All compiles
and native executions succeeded, with byte-identical images per subject. Stack32
remains 114176 bytes with SHA256
`55adb749405f33a9dcefcadb5a0c3f165452d1a8f05993f11f30ce8114aeb8b8`.
Its peak job commitment median increased 1.335 to 1.362 GB; no memory win is claimed.
The [host snapshot](host-memory.json) had about 25 GB free RAM; the simultaneous
[Rust process snapshot](competing-builds.json) was empty. These are snapshots,
not proof of exclusive CPU access throughout the run.

## Validation and replay

- Optimized candidate build succeeded, opt-level 1.
- Five focused graph tests passed, including comparison of indexed and symbolic
  mutual queries with an independent reference walk over cycles and overflow,
  constructor/generation identities, fresh environments and finite limits.
  [Full output](tests.txt), nextest run `e3b253b6-ae6b-4f4f-ba9a-96e1df52f215`.
- All [68 source observations](validation.json) passed: both producers on the
  nine positive stack32 places (`run --both`), nine reject twins, and sixteen
  arithmetic, effect, mutation and subtraction controls. A/B reject diagnostics
  agree and additional subtraction controls match their authored fragments.
  [Output](validation.txt), [producer hashes](validation-producers.json).
- Scoped rustfmt and whitespace checks pass. No full corpus/workspace sweep,
  Clippy, promotion review, other-host native execution or publication was run.

Replay with the existing `../statement_validation_2026_10_28/validate.py` and
`../checking_simplification_2026_10_28/compare.py`, passing baseline and candidate
executables followed by fresh output directories. Hold one build slot for each
local chain. Baseline A is the prior retained difference-constraint producer;
candidate B SHA256 is
`139ad7c5be5564fc9633e50e1d0a41744e7770c4bea909d476cd0845d7e8a558`.
Next: attribute the remaining arithmetic-walk cost on this new baseline.
