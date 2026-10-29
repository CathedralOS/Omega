# Reuse the completed difference-constraint decision

ITERATION-PERFORMANCE, fixed base
`6c09718ef1eb2ce913ea4b4385a5df5e574b2cda`, detached worktree
`.agents/worktree/pass-ablation`. Continue the existing optimization loop;
this is a performance repair on unchanged programs, not a new fact-row closure.

**Retain locally; continue to the next measured cost.** The representative's
unprofiled whole-compile median falls 7.7%, below this run's 15% substantial-win
criterion. No new push, merge, release qualification or broad speedup is claimed.

## Mechanism

Integer satisfiability already installs and closes unary/difference constraints
and checks contradictory cycles. When every retained bound has that exact shape
and no opaque operation can subsequently strengthen intervals, the general
pairwise polynomial contradiction search is redundant. Skip only that search.
Strict integer admission, substitutions, constant contradictions, equality
residues, graph closure and disequality handling remain first. General polynomial
and opaque-operation queries retain the original search. The numerical tests
compare 576 bounded systems with exhaustive integer models and exercise both
fallbacks. No new cache, configuration, dependency or source rewrite is needed.

[Research](research.md) records the hypothesis and primary-source argument.
The first [diagnostic](preparation-profile.json) ruled out judge setup caching:
setup was 17.72 ms against 471.91 ms in judgments. The finer
[integer diagnostic](integer-profile.json) found 311.52 ms in the redundant
search. Diagnostic scopes are inclusive and cannot be summed as wall time.
Diagnostic patches are separate zero-context patches against the fixed base;
replay with `git apply --unidiff-zero`. None remains in production.

## Measurements

Four unprofiled measured runs per producer and subject after warmups, interleaved
ABBAABBA. Fresh source caches, identical frozen source/build inputs, verified
opt-level 1 producers. Milliseconds, median [minimum, maximum]:

| Workload | Baseline compile | Candidate compile | Baseline checking | Candidate checking |
| --- | ---: | ---: | ---: | ---: |
| Stack32 other_machine | 4414 [4259, 4611] | 4073 [4047, 4274] | 1802 [1729, 1862] | 1512 [1499, 1614] |
| Length local | 2294 [2241, 2486] | 2326 [2134, 2379] | 1121 [1073, 1145] | 1108 [1066, 1200] |
| Dispatch match_join | 2255 [2221, 3533] | 2236 [2220, 2269] | 1119 [1082, 1146] | 1092 [1088, 1099] |

Length's 1.4% higher median and dispatch's 0.8% lower median are within overlapping
ranges; these cases show no established material change. All valid observations,
including the slow dispatch baseline, remain in [runs](runs.json). The earlier
six-run [screen](screen-runs.json) reduced the call-scanning median 539.9 to
212.6 ms, but its whole-compile ranges overlap. It supports attribution, not a
separate broad speed claim. See [summary](summary.json) and [metadata](metadata.json).
The host is shared; [competing build](competing-builds.json) and
[memory](host-memory.json) snapshots document that limitation.

All 36 comparison and six screen compiles and native executions succeeded.
Each subject's native image is byte-identical between producers. Stack32 remains
114176 bytes, SHA256
`55adb749405f33a9dcefcadb5a0c3f165452d1a8f05993f11f30ce8114aeb8b8`.
Peak commitment medians were 1.355 versus 1.337 GB on stack32; no memory win is claimed.

## Validation and replay

- Optimized build succeeded. Two focused numerical tests passed, 23 skipped:
  `mbx nextest run -p validation --target-dir build/p difference_constraint_tests`.
  Nextest run `7e41db95-f608-4607-bab2-990f868947c9`; test execution 0.820 s.
- [All 64 behavior observations](validation.json) passed: nine positive stack32
  places through Terminal Psi and native execution, nine reject twins, and 14
  additional formation, exact-actual, stale-storage, contract and cast controls,
  each on both producers. Reject exits and diagnostics match A/B, including
  authored expected fragments. [Validator](validate.py), [output](validation.txt),
  [producer digests](validation-producers.json), [input digests](validation-inputs.json).
- Rustfmt and scoped whitespace checks pass. No whole-corpus/workspace sweep,
  Clippy, promotion review or other-host native execution was performed.

The comparison uses the existing
`../checking_simplification_2026_10_28/compare.py A B NEW_OUTPUT_DIRECTORY`;
behavior checks use `validate.py A B NEW_OUTPUT_DIRECTORY`. Use fresh output
directories and one `tools/build_slot.py` hold per local chain. Producer B was
built after the production patch and first test, before the second test-only
addition; the latter is under `cfg(test)` and does not affect the executable.
Exact production patch: [candidate.patch](candidate.patch); the committed test
module supplies the unit-test source. Next: profile remaining statement checking
on this candidate, without attributing older measurements to the new baseline.
