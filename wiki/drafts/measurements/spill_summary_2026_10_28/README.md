# Spill-candidate event storage: inconclusive, parked

2026-10-28, Windows x86_64. **Inconclusive:** keeping candidate events in one
contiguous buffer preserved every observed native image, but did not establish
a whole-compile improvement. The compiler changes and new tests were removed
from the working source; the exact [candidate patch](candidate.patch) preserves
both for replay. No new optimization is promoted by this result commit.

## Hypothesis and scope

The [preceding investigation](../spill_loop_2026_10_28/README.md) attributed
95.797 ms to 58,212 slot-summary queries on the stack32 witness. The candidate
reused the scan's event buffer, stably sorted events by block/instruction, and
indexed block ranges instead of allocating and filling separate vectors. It
also removed an unused candidate load bitmap. No slot-reuse policy, validator,
work budget, source fixture or public interface changed. Sorting larger tuples
globally could offset saved allocations. [Research and acceptance](research.md).

The fixed base is local documentation commit
`4253e7254728b9189485337aa55db0d901be3d8d`. Its compiler, bundled sources, lockfile
and toolchain are unchanged from published
`2d87f18a195ec8523aeb4cdccc4ca0f793ba0e1b`; their diff was checked before reusing
the retained producer. All work stayed in `.agents/worktree/pass-ablation`.

| Producer | SHA256 | Source |
| --- | --- | --- |
| A | `75dcfeb5061db9f92709ad3d9a6f8246c7fb789a2222b910d662bc3f91f74b1d` | Published compiler base |
| B | `468778c2f73375d43c0cc5e35b735a34eae8ef04cfb22fc6750bb8d0c84079ac` | Fixed base plus candidate patch |

Both report optimized dev `opt-level=1`. Toolchain, CPU, exact input hashes and
producer profiles are in [metadata.json](metadata.json). Executables remain in
`build/spill-loop/baseline/omega.exe` and `build/spill-summary/candidate/omega.exe`.
The warm `build/pass-producer` target still contains the parked binary; rebuild
after restoration before treating it as the current source's compiler.

## Unprofiled comparison

Reused the existing `checking_simplification_2026_10_28/compare.py` unchanged.
Each subject has an A/B warmup, eight measured observations in ABBAABBA order,
then a separate A/B diagnostic profile. Every observation uses a new project,
build directory and process-local Windows source cache; ordinary package review
is settled before timing. No valid observation was discarded.

| Subject | A wall median [min, max], ms | B wall median [min, max], ms |
| --- | ---: | ---: |
| stack32 / other_machine | 5687.1 [5449.7, 6604.8] | 5678.9 [5488.3, 6293.8] |
| length / local | 2658.1 [2503.9, 2792.9] | 2814.9 [2662.7, 2895.6] |
| dynamic dispatch / match_join | 2868.5 [2702.1, 3034.5] | 2648.5 [2444.8, 2690.5] |

Stack32 changes by only -0.15% at the median; the controls move in opposite
directions. These data do not establish either a useful overall speedup or a
causal regression. Peak compiler job memory medians for stack32 were
1,343,574,016 bytes for A and 1,352,196,096 bytes for B, also not a demonstrated
memory saving. [All 36 observations](runs.json).

## Bounded attribution follow-up

The initial stack32 profile pair showed derivation at 320.681 ms versus
264.243 ms. Since one pair is weak evidence, a single additional six-run ABBAAB
profile comparison tested whether that result held. It reused
`spill_loop_2026_10_28/ablate.py` as an unchanged two-producer driver: despite its
historical name, B here is the contiguous-summary candidate, not private-slot
spilling. [All six follow-up observations](phase-runs.json).

| Profile scope | A median [min, max], ms | B median [min, max], ms |
| --- | ---: | ---: |
| Spill derivation | 416.209 [410.656, 476.434] | 331.118 [301.757, 359.566] |
| Register allocation, including derivation | 1946.766 [1901.173, 2095.123] | 1648.285 [1580.232, 1771.271] |

The faster candidate profiles are real observations, but attribution remains
uncertain: unchanged spill mutation also falls from 257.915 to 211.486 ms at
the median, reanalysis from 527.438 to 449.954 ms, and home probing from 330.512
to 284.829 ms. Checking also varies. Cache/allocator effects are possible, but
this experiment does not distinguish them from shared-host variation. Other
project tests were present in the [process snapshot](processes.json); no owner
process was interrupted. [Memory snapshot](memory.json). Nested scope times
overlap and must not be added as disjoint costs.

**Decision:** park the candidate, preserve its patch and restore the compiler.
The small targeted opportunity and uncertain whole-compile benefit do not
justify retaining this representation change now. This is not proof the idea
cannot help on a different workload, and the profile percentage is not a
shipping speedup claim. Do not repeat this trial unchanged just to obtain a
favorable sample.

## Correctness and execution evidence

- Two focused Rust tests passed ([receipt](tests.txt)): summary construction
  agrees with a last-event map across empty, sparse and duplicate positions,
  terminal loads and invalidated candidates; slot sharing accepts disjoint
  windows and refuses a destructive overlap.
- All 42 compiler invocations and native executions passed. Within each
  subject, baseline and candidate images are byte-identical, preserving the
  encoded stack layout as well as the authored exit. The six follow-up images
  also equal the stack32 baseline. This is observed equivalence on these
  witnesses, not a proof for every program.
- The first Rust test build failed with [LNK1181](linker-retry.txt). The named
  object existed at a 268-character path. A local `build/p` junction to
  `build/pass-producer` shortened the test target path; the retry passed in
  12.80 seconds. No source workaround or global system setting was used.
- Candidate build, successful test retry, 36-run comparison and six-run
  follow-up all returned exit 0. Rustfmt and scoped whitespace checks passed
  on the candidate. The saved patch applies to the fixed base, and restoration
  left `git diff --exit-code -- omega-rust` clean.
- No full corpus, other-host tests, Terminal/native differential qualification
  or promotion review was run after the performance decision. This is a parked
  experiment, not a validated compiler release.

The shared claim registration failed on push timeout, then temporary-file
locking errors. Read-back after retries and on completion confirmed ticket
`e63a8b414650400a804c7d61e67a0093` absent ([status](claim-status.json)); no active
claim was left to release. Failure receipts remain under `build/claims-pending`.

## Next question

Return to a larger measured cost. In this run, stack32 source checking alone
costs roughly 2.5 seconds. Its baseline diagnostic records 1,562.108 ms across
1,154 `validate_statement_node` scopes and 536.289 ms across 1,152
`value_position_calls` scopes. These are inclusive leads, not additive savings.
The next experiment should identify repeated work within statement validation
before proposing another representation or cache. This iteration stops after
its local result commit; no new push is authorized.
