# Complete performance tooling validation, 2026-10-29

The Rust tooling now covers paired compiler/runtime comparisons, native CPU and
resident-memory observations, phase/count attribution, opt-in allocation accounting,
explicit runtime suites, retained-evidence validation, regression budgets,
compatible historical comparisons, and separate WPR/perf capture adapters.
The [guide](../../../../tools/benchmark/comparison.md) describes the contracts.
These runs validate the instrument. They establish no compiler speedup.

Fixed base: `0add3ad35224b3b037f3952d652453236310673c`. Implementation is the source
overlay in [source-identities.json](source-identities.json). Host: shared Windows
x86_64, pinned nightly 2026-09-04, optimized development profile (`opt-level=1`).
No CPU affinity, frequency, cache or external-contention isolation is claimed.
Build/test chains used `tools/build_slot.py`; our own builds finished before
the integration measurements began.

## Focused validation

| Check | Observed result |
| --- | --- |
| Scoped nextest: omega-bench, allocation-observation, bounded-process; fixture and allocation features; one test thread | **39/39 passed**, run `78c0545d-7822-4ba7-b09f-699443cb4635` |
| Tooling/allocation Clippy, all targets, no dependency lint, `-D warnings` | Passed |
| Formatting for the three affected tool/resource packages | Passed |
| All-target tooling/allocation check for `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin` | Both passed; compilation evidence only |
| Default and allocation-instrumented Omega builds | Passed; both report optimized profiles |
| Existing Python benchmark tests | 46 passed; implementation unchanged |
| Four runtime A/A cases, 12 measured pairs plus one warmup pair each | All complete and validated; 104 child executions |
| Length/local compiler A/A, six pairs plus one warmup pair | All 14 compiles and 14 native verifications passed |
| Final evidence consumer against all six completed real runs | Passed |
| Corrected wrapping-square-sum native sample gate | 1/1 passed on Windows x86_64 |
| Wrong expected sum control | Native trap; compiler benchmark refused a success report |

[Final consumer checks](final-consumer-validation.log) and
[producer/tool build chain](final-validation.log) retain the command outputs.
Existing compiler warnings and bounded-process's deprecated `fetch_update`
warning remain outside the changed behavior. No full workspace/corpus sweep
was run. The new CI workflow runs native instrument tests on Linux, macOS and
Windows; local cross-compilation is not reported as those native runs.

The allocator control first reproduced a false live-byte peak of 96 rather than
64 when a free was physically completed before its accounting update. The fix
retires requested bytes before releasing storage. Its barrier-based test passes.
An additional negative control first demonstrated that a substituted native
execution path could pass retained-record validation. The final validator binds
the execution path, empty native argument vector, compiler publication and
retained artifact to the same product. Stream tampering, fabricated recomputed
summaries, missing pairs, changed inputs, malformed profiles, checksum failures,
timeouts, overflow and unavailable counters also have focused controls.

## Real observations

The plain compiler is
[`055b078090d693606126c472c2a97640778ebadcf74b4f94a6790902e32a6b3f`](omega-plain.exe.identity.json).
Its version says `unrecorded`; no revision is inferred from it. The fixed base
and recorded source overlay identify its build inputs. The instrumented compiler
has a distinct [digest and feature-marked version](omega-allocation.exe.identity.json).
MiMalloc remains the allocator in both builds.

Every length/local image is 1,536 bytes with SHA-256
`6a82736272f38a07eea38db3b71a681d2da4f09f856f7a5e3d7ee049bae41b50`.
Every image ran successfully against the authored empty-output/exit-zero oracle.
Compilation latency medians were 714.673 and 797.842 ms for the same producer.
The paired percent-change interval was **[-0.559%, +21.477%]**;
[`check ... 5` returned 2, inconclusive](compiler-budget.stdout).
This noisy shared-host A/A run demonstrates why the median alone must not
promote a performance claim. All observations remain in the journal.

The Windows provider recorded actual CPU times, peak resident bytes, total
faults and cycles. For example, the first compiler warmup used 423,112,704 peak
resident bytes and 20,633,214,150 process cycles. Scope is the primary process;
descendants are excluded. Minor/major faults are explicitly unavailable on
this provider. The calibration fixture separately touches 32 MiB and does CPU
work, requiring nonzero resource observations.

The separate [phase profile](phase-profile/profile.json) captured 114 timing
labels and 104 event/count labels. Nested timing scopes are inclusive and
must not be summed. The separate [allocation profile](allocation-profile/allocations.json)
observed 1,113,533 allocations, 48,657 reallocations, 280,482,651 requested bytes,
and a peak of 143,683,009 live requested bytes. This covers Rust GlobalAlloc
bookkeeping, not RSS or every custom/native allocation, and adds overhead.

The [runtime suite](runtime-suite/summary.md) checks independently calculated
checksums for runtime-sized compute, indexed memory, indirect calls and allocation.
These are Rust reference kernels, not generated Omega workloads.
[Assembly observations](assembly-observations.txt) show the emitted compute loop,
indexed memory loop, indirect vtable call and allocation/free loop at opt-level 1.
The checksum and assembly evidence jointly address elimination of the measured
work; checksum alone would not. The Omega square-sum remains a constant-input
optimizer subject, now with a consumed accumulator and falsifiable result.

[History output](history.stdout) uses independent resampling for two separately
captured compute runs. It does not invent pairs across time. Changed workload
input bytes are rejected by the history controls.

## Capability limits and retained material

[WPR](wpr-profile/profile.json) returned `unavailable`:
“Failed to enable the policy to profile system performance”, error `0xc5585011`.
Its raw start output is retained. No trace was captured, no permission or system
setting was changed, and no default recording was canceled. The unique owned
instance protocol remains implemented. Linux perf is unavailable on this Windows
host; its [explicit refusal](perf-unavailable/profile.json), parsing controls and
cross-target compilation are recorded. Actual permitted Linux perf capture and
successful WPR capture remain host-specific validation, not claimed results.

[Integration commands](integration-commands.json) and [script](integration.py)
retain exact local paths and exit codes. Full generated executables, accepted
locks and build trees remain in the ignored local directory
`build/performance/complete/`. This archive keeps reports, journals and raw streams
but omits generated images/build trees. Compiler `validate` requires those native
images; an archived report alone is insufficient to replay validation.

Capture used the recorded `omega-bench.exe` digest. The only later executable
change tightened native-artifact binding in `records.rs`; the final consumer
validated all retained runs. Measurement, statistics, resource capture and
workload source remained unchanged. [Source identities](source-identities.json)
distinguish the capture runner and final consumer. The earlier
[first-version snapshot](../performance_tooling_2026_10_29/README.md) remains
historical; its `/1` records are not relabelled as current `/2` evidence.
