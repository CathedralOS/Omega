# Cathedral UI: preparation repaired and complete warm compilation measured

The frozen combined UI export workload now completes normal package review and
acceptance, publishes both native UI components, and passes Cathedral's seven
unchanged native consumer tests. The decoder result below measures a complete
warm compile through native publication and process exit.

| Batch | Pairs | Before median | After median | Median paired saving | 95% interval for paired saving |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial | 32 | 7.621 s | 7.723 s | -10.213 ms / -0.145% | -3.136 to 2.900% |
| Confirmation | 16 | 9.386 s | 8.303 s | 362.526 ms / 3.680% | -0.517 to 12.307% |

The warm comparison does not establish a repeatable end-to-end UI speedup.

The two marginal medians are descriptive. The paired saving and interval come
from individual within-round differences and percentages. Initial and
confirmation are separate, prespecified batches: 32 pairs and 16 pairs, each with
two extra warmup pairs and alternating AB/BA order. All valid samples remain.
The analysis independently reproduces both Rust reports’ 4,000-resample
intervals before exploratory pooling. A shared desktop can vary with contention
and frequency; these intervals do not model temporal correlation or workload
selection. No additional batches were chosen to seek a favorable result.

## Preparation and the repair

The fresh pre-repair opt-level-1 producer's normal review outcome is
`review-timeout` at a 600-second per-command bound (606.570 s including
termination/cleanup). This is a censored setup observation, not a completed
compile duration or an exact speedup denominator. A pre-existing root lock is
not acceptance; the normal review outcome governs. Its source-equivalence record
joins the retained Matrix producer to this task's fixed base. The retained
repaired opt-level-1 producer completed the same normal workflow in 255.969 s;
the final release candidate completes normal preparation in 205.586 s.
Profiles and setup costs remain separate from the warm decoder comparison.

Owned-thread diagnosis found nested application resolution, arithmetic premise
projection and selected result-equation queries. The repair keeps arithmetic
re-entry inside the active resolution budget, removes duplicate intrinsic scalar
bound intake (observed integer carriers already own it), and stops equation
queries whose existing owners cannot produce an equation. Ordinary call
formation and entry requirements remain enforced. Remaining selected queries
establish prerequisites inside their recurrence scope.

Completed resolution and predicate queries can reuse exact answers only for
matching full authority-bearing payload, immutable proof/storage context,
depth and observed external recurrence memberships. Hits replay dependencies;
field tracing or incomplete observations disable reuse. Fingerprints only filter
candidates. Payload equality walks semantic values once and independently keeps
hidden descriptor/type/reference/float-landing metadata. Cache removal still
timed out at 600 s and was rejected. All failed/diagnostic trials remain in the
experiment ledger; none becomes a timing sample.

A bounded weak-key ownership trial passed 39 unit tests and seven native consumer
tests, but its first native publication after review showed no relevant peak-memory improvement.
It was parked; the planned follow-up screen was not run. The retained strong-key
implementation has 37 checker tests. Its memory remains a limitation, not an
unmeasured claimed gain.

Native publication also writes an authoritative requirement/image/export TSV
from existing NativeComponentProduct facts. Component descriptions alone list
both provider candidates and do not identify their physical export mapping.
The adapter validates TSV rows against actual ELF exports, then executes the
unchanged consumer. It never guesses by filename or code size.

## Workload and matched producers

The frozen authored UI tree is 172 files / 3,101,352 bytes, 161 of them Omega files,
from the earlier qualified Cathedral e493d887 snapshot plus its recorded working
changes and compatibility migration. It is not a claim about current Cathedral
HEAD. No authored UI file was changed for this repair. Both producers use one
physical ui-shared root, the same explicit Omega library, their own normally
reviewed accepted locks and private warm LOCALAPPDATA retained stores. All
accepted locks and retained-record bytes are checked before and after.

Both final producers use the pinned Rust toolchain and release opt-level 3.
The earlier Matrix comparison used opt-level 1; this protocol adjustment was
recorded before any final UI timing to avoid roughly 2.7 hours of opt-level-1 publication
work. Do not transfer the Matrix effect size or absolute times to this result.
All UI checking and native-index repairs are common to both producers. Their
only production-source difference is compiler-storage/src/buffer.rs: before
restores b1d401fd1d3ddc5470d915bc5ab5044c2e093677; after retains the committed
small generation-domain registry. No source/lock admission rule is bypassed.

- Before executable SHA-256:`0d29cc74ab001314457bb4bdb0e910097528dcdf225440fb090b9a57988b6f6f`.
- After executable SHA-256:`232ebd795c7fae9dfe4e696b107d46bd18f55bde882b0d22e5f3a1ed0fd9dbc3`.
- Complete validated publication aggregate SHA-256:`684381a49a8c10a800488f83e2d3cf5cd230ecd82b6bafddf8f930eb4a7187a3`.

The first native publication after package acceptance took
64.779 s before and 62.634 s
after. Those two publications also fill additional retained compilation caches;
they are bootstrap qualification, not paired timing samples. Each role then has
two warmup pairs before its batch. The primary measurements compile unchanged
source with fully warmed compiler caches into fresh native directories. No
edit-to-build or cold-start speedup is claimed. The retained cache records are
snapshotted after bootstrap and checked byte-immutable across the measured work.

The timer is existing capture_compile's compiler process startup through source
resolution/graph restoration, checking and native publication, output capture,
and exit. One reported-work unit is one complete compile. A fresh native output
directory is used every time. Input hashing, ELF checks and consumer execution
are outside the timer. Process exit must be zero and no compilation error may
appear. The exact two shared components, two descriptions, probe and TSV must
match the native-qualified six-product byte set on every observation.

## Quality and evidence

All 112 publications match that complete native byte set: 96 measured observations,
8 warmups, 2 bootstrap publications and 6 separate instrumented phase runs. Both
bootstrap publications pass seven unchanged Cathedral Rust consumer tests and
their standalone Linux probe exits zero under WSL Ubuntu. The consumer runs
validated ELF code/data through System V ABI mappings on Windows. It checks
instance/stale-node isolation, event/edit behavior, failure recovery,
geometry/motion, clipping/labels and malformed packets. This is actual engine
execution; it does not establish interactive desktop or full OS acceptance.

The focused checker suite passes 37 tests. The 27 affected fixture observations
select six existing positives and seven rejects before the repair, then those
same controls plus the new nested-guard arithmetic positive after it.
11 positive observations pass
both Terminal Psi and native execution with authored oracles; all 14 negative
observations reject. There are 1
matched inherited positive-fixture limitations, explicitly retained as failures,
not passes. Before/after source bytes and full diagnostics (only private cache
paths normalized) must match for those cases. Every previously passing positive
and the new nested-guard fixture must pass after the repair. No new regression
is admitted. The initially selected contextual byte-view fixture
had an inherited Build/static-argument issue; a temporary correction exposed an
old Terminal byte-backing-loan failure despite native exit zero. Its tracked
fixture was restored and its raw failure retained. The selected-storage positive
replaces it in both final roles. The initial unsupported --offline run flag was
also a preserved harness failure, never an executed program or timing sample.
A final focused Boolean-result control also passes both engines before and
after, preserving a Boolean value used to prove a caller requires clause. This
supplement does not alter the timed cohort. Scoped rustfmt and Clippy for validation and
compilation-products pass; Clippy warnings remain in the raw log. No whole
workspace/corpus sweep or other-host execution was performed.

[observations.zip](observations.zip) retains exact adapters, source/producer/lock
identities, journals/reports, commands, phase and validation logs, frozen seven
consumer sources, external ELF checker, all trial patches and the experiment
ledger. [evidence.json](evidence.json) indexes and hashes the archive.
[result.json](result.json) contains matched statistics, raw pairs and phase
medians. Three separate instrumented runs per producer show these median inclusive scopes:

| Scope | Before | After |
| --- | ---: | ---: |
| source.graph.load.decode | 864.827 ms | 715.993 ms |
| source.graph.decode.checks | 591.165 ms | 460.824 ms |
| build.resolve_sources | 1,630.643 ms | 1,823.687 ms |
| 05.lower_terminal_selection | 1,627.025 ms | 1,666.410 ms |
| 06.0.e_produce_native_product | 2,847.503 ms | 3,009.896 ms |

The decode medians are consistent with less targeted work, while the complete
warm compile remains unresolved. Three instrumented runs do not establish a
phase-level confidence interval. These scopes overlap: do not sum them or
substitute a favorable phase for the end-to-end result. Frozen UI source bytes remain in
[../frozen-inputs.zip](../frozen-inputs.zip), ui-exports/source/.

Compiler executables, native products and private retained caches stay in ignored
local build/performance/cathedral-ui-stall; their hashes remain in the evidence.
PDB files also stay local as diagnostic material, not semantic qualification.
Archive paths are repository relative. Reproduction needs the fixed base and
candidate patch, the frozen authored input, the same library and consumer, fresh
normal acceptance for each rebuilt producer and bootstrap native qualification.
Moving the physical source root changes nominal identities: requalify the native
oracle rather than copying a stale expected hash. Historical diagnostic scripts
are evidence, not a request to rerun every discarded experiment.

The work is local. No code branch was pushed. An initial claims.py call did push
advisory coordination metadata to refs/coordination/omega-claims/main despite the
no-push instruction; this was disclosed during the task. No later remote write
was made, including no claim renewal or release.

Descriptive timing ranges (minimum / tool observed p95 / maximum; seconds):

- initial: before 7.045 / 12.375 / 13.837; after 7.022 / 12.849 / 13.362.
- confirmation: before 7.139 / 13.772 / 13.772; after 7.284 / 14.666 / 14.666.

Windows job peak committed bytes across all 106 ordinary publications (including warmups and bootstrap):

- a: 53 observations, median 2,264,039,424 bytes, maximum 4,770,422,784 bytes.
- b: 53 observations, median 2,261,651,456 bytes, maximum 5,005,094,912 bytes.

These job values cover the owned compiler and any descendants, not parent Python or other desktop work. Tail estimates are descriptive at these sample counts. No slow valid sample was excluded.
