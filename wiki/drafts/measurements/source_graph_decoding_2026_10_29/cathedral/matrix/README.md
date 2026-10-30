# Cathedral Matrix: measured warm component compilation

**The generation-domain registry optimization saves about 240 ms on this real
Cathedral component build.** The primary 32-pair comparison estimates a 9.8%
median paired reduction; a separate 16-pair confirmation estimates 8.3%. This is
complete warm compilation and publication of the Matrix policy component used
by Cathedral's Rust desktop. It does not measure the entire OS or UI gallery.

| Batch | Pairs | Before median | After median | Median paired saving | 95% interval for paired saving |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial | 32 | 2,449.811 ms | 2,205.565 ms | 240.601 ms / 9.803% | 9.328–10.490% |
| Confirmation | 16 | 3,123.986 ms | 2,901.176 ms | 227.534 ms / 8.323% | 3.510–12.214% |

Each batch has two additional warmup pairs and alternates AB/BA order. All valid
observations are retained. Confirmation ran on a slower, noisier shared desktop;
its result is reported separately. Exploratory pooling of all 48 compatible
within-run pairs estimates **238.556 ms / 9.724%** median paired saving, with a
95% percent interval of **9.193–10.497%**. These are paired estimands, not
subtraction of the two marginal medians. Within-pair bootstrap intervals do not
model host temporal correlation or workload selection. The analysis reproduces
both individual Rust reports' 4,000-resample intervals exactly before pooling.

## What was measured

The frozen production Matrix tree contains 24 authored files / 47,106 bytes:
policy, configuration, lifecycle, recovery, startup, launch and the native
wrapper. It comes from the prior qualified Cathedral `e493d887` snapshot plus
its recorded working changes and compatibility migrations. It is not a claim
about current Cathedral HEAD. The manifest and exact source bytes are retained.

One physical `matrix-shared` root is used by both producers. Separate copies
were rejected as a comparison because absolute package identities changed
component names and bytes. Each producer normally reviewed and accepted its own
lock for that same root. The adapter restores the entire corresponding accepted
lock before timing and verifies its digest before and after compilation. Both
use the same explicit Omega library, with private warm `LOCALAPPDATA` stores.
No lock token, cache envelope, compiler identity or admission rule is bypassed.

The existing `capture_compile` timer covers compiler process startup through
source resolution, source-graph restoration, checking/reuse, native publication,
output capture and compiler exit. `omega-bench` receives this through
`reported_work_ns_per_unit`, where one unit is one full compile. Adapter work,
hashing, structural validation and runtime checks are outside that timer.
Parent Python resource counters are not compiler-only measurements.

Both initial and confirmation journals pass `omega-bench validate` and `check`.
Private retained-record hashes are unchanged across both batches. Their graph
records are 414,126,823 and 414,126,841 bytes respectively; cross-producer private
cache bytes are not asserted equal.

## Matched producers and the necessary common repair

Both optimized producers use opt-level 1, the pinned toolchain and the default
allocator. Their source base is `4144122f9aa4f30ab870a78ef8b9090d0237c7d5` plus
the identical provider-plan receiving-schema repair recorded here. Their only
production-source difference is `compiler-storage/src/buffer.rs`: baseline
restores it from `b1d401fd1d3ddc5470d915bc5ab5044c2e093677`; candidate retains
the generation-domain registry optimization from `4092626edbe`.

- Before executable SHA-256: `0ce563da66114d49e73f380fd3e4994929553cd7f2c5d273b52b0e2016a68dcc`.
- After executable SHA-256: `6098777dbc4f1c4ae62fed8671035124c838b9b038f7bd2ac1fefbdcf43efce9`.

Fresh Matrix publication initially failed independent component verification:
`provider closure is inconsistent: unknown provider plan input schema`.
The writer emits v5 but the bounded retained-input reader still expected v4
and omitted v5's provider-application and receiver-application fields. The
shared repair consumes those fields and uses the writer's existing domain
constant. Complete commitment hashing, bounds, ordering, EOF and exact checked
body joins remain enforced. Both measured producers contain this same repair;
its effect is not credited to the registry optimization.

Validation: all four retained-input tests pass, including populated application
framing, retired-schema rejection with a matching digest, and changed-commitment
controls. The complete `effects` suite has **13 passes and one existing failure**:
the no-string-identity source guard flags unchanged `authored.rs` and
`evaluated_bindings.rs`. Running the same guard against unmodified HEAD source
reproduces those exact violations. `component-description` compiles; it has zero
tests, and its nextest exit 4 is not reported as a test pass. Logs are archived.

## Publication equality and runtime boundary

All **112 same-root complete publications** (96 measured, eight warmup, two
bootstrap and six separate profiles) produce the same three files byte for byte:
the shared component, its `.so.component` description, and `omega-program`.
There is no export index for this single-component workload. All 336 actual
files were rehashed. The aggregate set checksum is
`6d01f7f36cbd52c02a8f2546dcf168f3da5962a493b6b2885855c0697b0fd646`.

Both bootstrap components were actually loaded and called through their exported
pointer/u64 ABI under WSL Ubuntu. Eleven source-derived lifecycle cases compare
all 128 packet words, including neighbor and tail canaries: unknown, begin,
sent, answered, finish, finish-loss, close-child, close-clean, close-uncertain,
gate and gate-refuse. Both producers pass. Timed publications must reproduce
the complete validated artifact set; runtime replay is outside timing.

**The separately linked standalone wrapper fails with SIGSEGV for both
producers**, POSIX return code -11, expected zero. Its original expected-zero
qualification remains red. Before paired timing, the experiment explicitly
selected component publication with actual production component ABI validation;
the wrapper is still emitted and strictly compared. This result establishes
component compile latency and component ABI behavior, not successful standalone
application execution. The common wrapper defect was not repaired here.

## Separate attribution

Three instrumented runs per producer give these median inclusive scopes:

| Scope | Before | After | Change |
| --- | ---: | ---: | ---: |
| `build.resolve_sources` | 1,986.918 ms | 1,760.876 ms | -11.4% |
| `source.graph.load.decode` | 1,444.858 ms | 1,158.573 ms | -19.8% |
| `source.graph.decode.checks` | 932.016 ms | 749.561 ms | -19.6% |

All six profiles load one graph with eight sources, retain eight Build/Product
checks, reuse seven prepared graph views and hit one component cache entry.
These observations support the proposed decoding mechanism; they are separate
from the main timing samples. Nested inclusive scopes overlap and cannot be
summed. They do not establish an independent phase confidence interval.

## Evidence and limits

[`result.json`](result.json) retains all 48 raw pairs, the separate summaries,
phase medians and classified setup failures. [`audit.json`](audit.json) records
the final source, lock, cache, output and runtime audit.
`observations.zip` contains raw journals/reports/plans, streams and observation
records, exact accepted locks, frozen Matrix input, adapters, ELF validator,
source overlay patch and validation logs. `evidence.json` binds the archive.
It excludes large executables and private graphs; those remain in the ignored
`build/performance/cathedral-matrix-generation-domains/fixed/` directory. Paths
and lock identities are as-run provenance, not a promise that moving the archive
preserves package identity or reproduces admission automatically.

Initial path normalization, single-component file roster and `$ORIGIN` dependency
roster mistakes are retained as unqualified adapter attempts. Separate-root
bootstrap observations are retained but excluded from timing. No valid paired
observation was removed. Matrix was selected after the UI setup failures and
before its performance result was observed; it is one selected workload.

The [UI/gallery setup attempts](../README.md) remain blocked and provide no
speedup estimate. No cold-cache comparison, full OS build, gallery interaction
or full corpus gate was run. The standalone wrapper and unrelated source guard
remain failing. This work is local; nothing was pushed.
