# Spill-slot reuse: keep the resource saving, investigate its analysis cost

2026-10-28, Windows x86_64. **Rejected:** forcing every spill into a private
slot. It passes this witness but increases the larger function's explicit stack
reservation from 648 to 19,944 bytes (30.8x). All diagnostic compiler edits were
reverted; this iteration retains the investigation and replay inputs only.
No new compiler speedup is claimed.

## Fixed base and publication

[PR #740](https://github.com/CathedralOS/Omega/pull/740) passed the
[source check](https://github.com/CathedralOS/Omega/actions/runs/40813553657/job/123249250851)
and [merge-group check](https://github.com/CathedralOS/Omega/actions/runs/40813814472/job/123250098268).
It merged as `2d87f18a195ec8523aeb4cdccc4ca0f793ba0e1b`; fetched-main ancestry
was verified. No CI repair was necessary. [Receipt](landing-740.json).

This experiment fixes that published commit as its base, using the detached
`.agents/worktree/pass-ablation` checkout. Intervening allocator changes made
the previous iteration's binary unsuitable as this baseline. The authored
workload remains `samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_machine`.
Source SHA256 is `93cf1dddd3ab249a9aa6f9f68ca198394affb199ec6697c6da39608db37e7686`;
Build SHA256 is `467ba43680810802f19ba2c399ff5d359059c6404cb9b3bf71d061b819729005`.

## Question and decision rule

The [previous profile](../checking_simplification_2026_10_28/README.md#next-question)
identified repeated spilling as a substantial remaining allocator cost.
First locate the cost within derivation; then temporarily remove slot reuse to
measure its cost and resource effect. A shipping optimization must preserve
behavior, source-bound admission, work budgets and slot-lifetime safety without
trading away the existing stack saving. A smaller timer alone is insufficient.

The touched route is allocator recovery -> spill derivation -> slot assignment
-> rewrite -> sealing. Both proposal and independent replay consume the shared
assignment routine. No source fixtures, target policy, callers, public API or
configuration changed. The diagnostic patch changes only the final reuse
argument; the ordinary budget calculation and all other checks still run.
[Research and prior-work review](research.md).

## Attribution

All three profiles compile and execute the same source successfully and produce
the same 114,176-byte PE with SHA256
`55adb749405f33a9dcefcadb5a0c3f165452d1a8f05993f11f30ce8114aeb8b8`.
They are diagnostic observations, not matched speedup measurements.

| Observation | Wall ms | Spill derivation ms | Slot assignment ms |
| --- | ---: | ---: | ---: |
| Fresh base | 5118.5 | 296.2 | Not separately measured |
| Derivation detail | 7715.3 | 593.6 | 412.5 |
| Slot detail | 5462.6 | 391.2 | 277.7 |

The slot-detail observation makes 3,309 assignments. Candidate collection costs
2.380 ms, window construction 14.309 ms, choosing 242.336 ms, and mapping 0.384 ms.
Choosing includes 94.700 ms scanning existing slot accesses (3,306 calls) and
95.797 ms preparing/querying candidate summaries (58,212 calls). Nested scopes
overlap; instrumentation bookkeeping also contributes to enclosing scopes.
Replacing candidate-list duplicate checks is therefore deprioritized.

| Producer | SHA256 | Source delta | Observation |
| --- | --- | --- | --- |
| Base | `75dcfeb5061db9f92709ad3d9a6f8246c7fb789a2222b910d662bc3f91f74b1d` | None | [JSON](baseline-profile.json) |
| Derivation detail | `005e5a1e788149323f977834aedb608408edf2c758c2d6bbc6db36521505c598` | [Patch](detail-instrumentation.patch) | [JSON](detail-profile.json) |
| Slot detail | `ccf4418af15b583c34a5d624f06757402c2cc594d473c7b8d6ec29aa5ea92d9f` | [Patch](instrumentation.patch) | [JSON](slots-profile.json) |
| Private-slot ablation | `8ad081ee85ec0deea790d08fae4591aad0e398be8eb457f8e3ebb05c482c3e22` | [Patch](ablation.patch) | [A/B JSON](ablation-runs.json) |

## Ablation

Six isolated-cache, profiled observations in ABBAAB order; three observations
per variant. A is the base, B disables slot reuse. Every compile and native
execution exits 0, with empty stdout/stderr. Source and Build hashes match;
producer and output hashes are stable within each variant. No observations
were discarded. Each observation has a fresh project, build directory and
process-local `LOCALAPPDATA`, with ordinary package review settled before timing.

| Metric, ms | A median [min, max] | B median [min, max] |
| --- | ---: | ---: |
| Compile wall | 7677.2 [7519.4, 9310.1] | 6609.0 [6366.3, 8898.5] |
| Unchanged source checking | 3612.7 [3546.3, 3888.1] | 3053.0 [2927.1, 4180.4] |
| Spill derivation | 417.6 [375.2, 652.8] | 143.2 [104.0, 178.1] |
| Register allocation | 1926.0 [1742.5, 2879.8] | 1888.8 [1405.3, 2254.2] |

These are a small diagnostic comparison on a shared host, not an unprofiled
performance qualification. A concurrent release build was observed in another
worktree ([process snapshot](concurrent-builds.json)); it was not interrupted.
The unchanged checking stage varies materially, so the overall wall-time
difference cannot be attributed wholly to this patch. Peak compiler job memory
medians were 1,402,650,624 bytes for A and 1,348,964,352 bytes for B; these noisy
observations likewise do not establish a memory optimization.

Both images remain 114,176 bytes, but B's SHA256 becomes
`d50e22ad84371e6410e33fcc15a0b1f14db1cb0f04dd763f4ebbc7ec8d368cef`.
The [disassembly excerpts](resource-evidence.txt) show the resource regression:
A reserves `0x288` bytes; B reserves four `0x1000` pages plus `0xde8`, or 19,944
bytes, with page touches. This is an encoded function reservation, not a claim
about whole-program worst-case stack usage. Equal image size and exit code did
not preserve the resource behavior.

**Decision:** reject private-slot-only spilling and remove all instrumentation.
The next highest-value question within this path is whether existing block
access summaries can feed candidate checking without rebuilding per-slot block
vectors on every spill. Any reuse must track exact block contents and candidate
membership and preserve declaration-order slot selection. No such cache is
implemented or claimed safe by this record. This bounded iteration ends with
the resource tradeoff resolved; it does not claim the allocator is optimal.

## Replay and checks

[Environment and build receipts](environment.json) record the toolchain,
optimization profile and the baseline shell-receipt limitation. All measured
producers report optimized dev `opt-level=1`. Retained local executables are
under `build/spill-loop/{baseline,slots-profile,ablation}/omega.exe`; build logs,
complete compiler transcripts and disassemblies are under `build/spill-loop`.
The intermediate derivation-detail binary is not separately retained, but its
patch and digest are recorded. The build target still holds the rejected
diagnostic binary; use the retained base for this experiment, or rebuild from
the restored source before treating that target as current.

Apply a diagnostic patch to the fixed base in an isolated checkout, then build
under one build-slot hold:

```powershell
python tools/build_slot.py -- mbx build -p omega --locked --target-dir build/pass-producer
python tools/build_slot.py -- python wiki/drafts/measurements/spill_loop_2026_10_28/profile.py <producer> <new-output-directory>
python tools/build_slot.py -- python wiki/drafts/measurements/spill_loop_2026_10_28/ablate.py <baseline-producer> <private-slot-producer> <new-output-directory>
```

Nine measured compile/native executions passed: three attribution observations
and six A/B observations. The harness assertions check successful compilation,
published artifact existence, optimized exact producer identity, input stability,
native exit, and repeatable per-variant output. Both scripts also pass
`python -m py_compile`. `git diff --exit-code -- omega-rust` confirms all compiler
changes were reverted. No full corpus, other-host execution, Terminal/native
differential suite or independent semantic review was run for this discarded
experiment. No compiler implementation is being promoted by this commit.
