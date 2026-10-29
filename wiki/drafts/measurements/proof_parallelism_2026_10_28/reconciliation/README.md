# Split Dungeon's lowering reconciliation cost

Follow-up to the [checking and lowering measurement](../README.md), on the same
Windows x86_64 Ryzen 9 7950X host and unchanged Dungeon EOF workload. The prior
inclusive timer grouped three operations into roughly two seconds. This pass
identifies which owner consumes it and counts its work. No optimization is applied.

## Result

Three timing runs, fresh private source cache and native output directory each:

| Operation | Times, milliseconds | Median |
| --- | --- | --- |
| Place-declaration reconciliation | 0.2101, 0.1673, 0.1511 | **0.1673 ms** |
| Descriptor residences | 0.0697, 0.0615, 0.0596 | **0.0615 ms** |
| Payload memberships | 2,862.2485, 2,277.9524, 2,187.4934 | **2,277.9524 ms** |

These are separate synchronous calls, summed over the three Terminal lowering
invocations in each compile. They are not overlapping parent/child timers.
Payload membership transport accounts for over **99.98%** of this region in
each run. The earlier grouped label did not establish that descriptor-residence
planning was expensive; this split shows its costly analysis path is not entered
on this program.

The main emitted module has **202 machines**. Its membership pass selects 109.
In the middle timing run, its 2.2779505 seconds split into:

| Disjoint part of that same invocation | Time |
| --- | ---: |
| Type clone, Terminal graph and outer analysis preparation | 204.9118 ms |
| Reference-membership queries | 514.1859 ms |
| Remaining membership work and destruction | 1,558.8528 ms |

The last row is subtraction, not a finer attribution of view/operand/history
queries or cleanup costs. Summing separate component medians would not give
that decomposition; these three values deliberately come from one run.

## Work counts

Two separate count runs produced identical non-timing metrics. For the main
module:

| Observation | Count |
| --- | ---: |
| Operations scanned by place reconciliation, once | 2,702 |
| Leaf-copy results encountered | 281 |
| Missing declarations added | 0 |
| Membership candidate machines | 109 |
| Membership fixed-point sweeps | 127 |
| Sweeps that added membership rows | 18 |
| Operations scanned across membership sweeps | 3,313 |
| Eligible structural-operation visits | 418 |
| Reference-membership queries | 418 |
| Payload-target queries | 173 |
| Proposed contract applications | 439 |
| Outer retained DescriptorAnalysis constructions | 1 |
| ReferenceResultMemberships context constructions | 127 |

There is one final unchanged sweep per candidate machine and 18 additional
changing sweeps. The data does not show a runaway number of fixed-point rounds.
Proposals can contain existing or duplicate facts; 439 is not a count of new
proof facts. The first scan's zero additions does not authorize deleting that
pass for other programs.

`ReferenceResultMemberships::new` is lazy. Its 127 constructions do **not** mean
127 whole-graph builds: the graph is built on demand for qualifying calls.
Likewise, the single outer analysis count does not count analyses constructed
inside reference-query helpers. Those helpers may instantiate their own replay
analysis. Actual internal graph-build counts and exact-context query duplication
are not measured here. Reusing a replay across different receiving annotations
or storage versions needs its own correctness argument.

## Validity and controls

- Compiler source remains at semantic base
  `456c5e984452a45d977295e254b3ca9ae3cc7750`; worktree commit
  `8150d7c081d` adds only the prior measurement record. The temporary diagnostic
  patch and executable/lock hashes are in [identity.json](identity.json) and
  [instrumentation.patch](instrumentation.patch). Both producers report opt level 1.
- Reused the [same source manifest](../input-manifest.json). Source inputs were
  checked before and after every run. Each executable used its own normally
  accepted lock; [policy comparison](producer-policy-comparison.txt) has equal
  policy fingerprints and zero changed source entries. No policy gate was bypassed.
- All nine fresh-cache compilations (three timing, two counts, two untouched
  controls and two collection-off controls) executed natively with the authored
  opening description and exit 0. Every image has SHA-256
  `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.
- The instrumented producer also passed public `omega run --both --keep`.
  Native and Terminal stdout matched byte for byte, with exit 0 and the same
  retained image hash. This check is separate from timing.
- Untouched control wall times: 22.969 and 21.012 seconds. Instrumented
  collection-off: 22.034 and 20.226 seconds. Timing-on: 22.858, 21.182 and
  20.054 seconds. Host variation prevents resolving small observer overhead;
  these are not optimization speedup observations.
- `OMEGA_PHASE_PROFILE=0` throughout. Three local operation timers and two
  accumulated sub-timers avoid the general profiler's per-event global mutex.
  Per-operation counters are enabled only in the separate counts runs. Measured
  intervals stop before formatting their observation row.
- The selected observer test passed: 1 test run, 6 skipped. See
  [build-and-test.log](build-and-test.log). The analyzer validates disjoint
  sub-timer accounting, sweep/count invariants and repeated count-run agreement.

The host was shared and busy. Times are elapsed wall times, including possible
preemption, and do not establish a global serial fraction or an optimal thread
count. This pass establishes attribution on one program, not general compiler
acceptance or a safe cache key.

## Replay and decision

[measure.py](measure.py) reuses the parent's fixed-workload runner and performs
the interleaved controls, fresh-cache runs, output/hash checks and Terminal
comparison. It expects the archived baseline and a freshly built instrumented
producer under `build/proof-profile/producers/{baseline,reconcile}/`, with each
producer's accepted `omega.lock` beside it. Prepare these through ordinary
package review; the ignored local retained artifacts are not a trust bypass.
Use the recorded base, source manifest and patch, and a short Windows build
path as described in the parent record. Existing result directories are refused.
The separately run `recon-counts-pilot` is the second count observation.

[raw](raw/) retains result records and both output streams, bound by
[raw-manifest.json](raw-manifest.json). Recompute the summary from the repository:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/reconciliation/analyze_results.py wiki/drafts/measurements/proof_parallelism_2026_10_28/reconciliation/raw
```

Decision: retain the attribution and remove temporary hooks from compiler
sources. No production optimization, cache, parallelization change or speedup
is claimed. The next useful probe belongs inside payload membership replay,
especially its remaining 1.56-second portion, rather than the inexpensive place
and residence passes. Full corpus, other programs, other hosts and gameplay
routes beyond EOF were not tested.
