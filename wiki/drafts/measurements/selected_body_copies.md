# Real Build routes copy retained typed storage

## Historical Windows observation

Two unchanged programs each caused seven deep copies of typed compiler storage,
including a roughly 32 MB allocation request when selected-body activation had
no pending body to complete. This establishes work during an empty selected
activation, but does not establish a bottleneck that warrants splitting the compiler's
declaration and body storage now. This PR retains a portable measurement harness;
it does not repair ownership or claim a compiler speedup.

Both observations used one selected continuation with zero pending bodies and
73 selected source files. Each later completed one authored batch (1,748 or
1,746 machines), followed by an empty batch. Neither exercises repeated,
nonempty selected-body completion. The existing ownership concern remains open;
the proposed wide storage change still needs evidence on that missing workload.

## Workloads and boundary

The ordinary release producer is fixed to
`62c864eed68bda01f4a1a60bd08d95414957abb3`. Results below describe that historical
source, not current main. The Windows workstation had 24 logical CPUs and
128 GB RAM. Processes ran at Idle priority, affinity `0xFFFFC0`, with
`MBX_SCHEDULER_CPUS=18` and `CARGO_BUILD_JOBS=18`; one team heavy chain was admitted.

Inputs are byte copies of existing fixtures, including their authored Builds:

| Name | Existing fixture | Main SHA256 | Build SHA256 |
| --- | --- | --- | --- |
| Scalar | `tests/fixtures/run/calls/scalar_contract_route_exit` | `abe2e510430a8bf7caf7abc2641bd18e4f6b40b09a82b69c554158e675a66280` | `92025abbc82980543cb4ad025dac0f980827eccc1607541820f474345f00a649` |
| Borrowed | `tests/fixtures/run/ownership/borrowed_root_replacement` | `a3aa641667fdc07a6912eab8a4e747ea549b0c2f77106eedff31747c654ec342` | `4a341f0fbc552427c9d7350dd878b5c5f75c0b80508202e94f53d7639dc89cc7` |

These are a scalar-contract route and a small mutable-record route, both with
bundled library input. Each default CLI control reported 238 source files.
Both Builds bind all four hosted targets; observations request only
`windows_x86_64`. They are two focused examples, not a population distribution
or evidence for a Pareto model. Much of their retained storage is shared library
work; the similar measured shapes are therefore informative rather than two
independent sizes in a scaling curve.

The first planned second input was the unchanged
`tests/fixtures/pass/targets/shared_program_distinct_roots_exit`. Its two-target
capture failed because authored `ControlFlowCleanup` had no second lowered-form
check on this base. No source, optimization, or checker was altered to accept
it. Borrowed replaced it for measurement; distinct-target coverage was lost.

## Actual live copies

The patch instruments the real Build orchestration. Numerical thread-local
allocator counters surround the original `Arc::make_mut`, `Arc::unwrap_or_clone`
or direct `TypedTrees::clone`; they retain no extra Arc owner. Geometry is read
before the operation, and snapshots/logging occur afterward, before semantic
mutation. Every deep copy below cloned 102 arenas.

| Site, in execution order | Scalar allocations / requested bytes | Borrowed allocations / requested bytes |
| --- | ---: | ---: |
| Shared typed mutation before selection | 14,125 / 31,835,318 | 14,125 / 31,835,318 |
| Selected activation (empty body batch) | 14,125 / 31,835,318 | 14,125 / 31,835,318 |
| Declaration projection | 14,125 / 31,839,610 | 14,125 / 31,839,610 |
| Retained predecessor for validation | 14,708 / 33,368,559 | 14,708 / 33,368,559 |
| Shared typed mutation before authored completion | 35,517 / 73,784,007 | 35,515 / 73,761,151 |
| Authored body completion | 35,526 / 73,758,967 | 35,524 / 73,736,111 |
| Final owned typed extraction | 48,497 / 116,170,768 | 48,489 / 116,104,527 |
| **Sum across these operation windows** | **176,623 / 392,592,547** | **176,611 / 392,480,594** |

Eight additional observed Arc operations per route made no allocation/arena
copy, including extension and the first unique body-completion owner. Two
unique operations freed 26,032 bytes each, hence negative net live deltas.
The largest single-window outstanding requested payload was 116,164,688 bytes
for scalar and 116,098,447 for borrowed; these are not simultaneous route peaks.

| Geometry before deep copies | Scalar | Borrowed |
| --- | ---: | ---: |
| Selected activation: machines / expressions / statements | 1,100 / 19,187 / 2,016 | 1,100 / 19,187 / 2,016 |
| Authored completion: machines / expressions / statements | 4,122 / 40,550 / 3,971 | 4,120 / 40,534 / 3,971 |
| Final extraction: machines / expressions / statements | 4,122 / 68,739 / 9,633 | 4,120 / 68,690 / 9,629 |
| Sum of cloned arena slots, including dummy slots | 2,601,706 | 2,601,242 |
| Sum of nominal arena payload bytes | 346,379,140 | 346,275,584 |

Nominal arena payload means `slot_count * size_of::<T>()`, including padding;
it excludes slot metadata and nested owned payloads. It is logical record volume,
not hardware traffic. Requested bytes count successful allocations/reallocations.
Live/peak deltas track net outstanding requested payload on the operation's
thread; successful realloc observations exclude allocator-internal transient
coexistence. They are neither allocator footprint nor resident memory.

Instrumented copy-window times summed to 134.412 ms (scalar) and 62.173 ms
(borrowed). Export after one operation can warm later operations, and tracing
affects timings. These sums cannot be divided by ordinary CLI time to establish
an uninstrumented time share. Cross-thread work is outside the allocation scopes.

Source-level work counters reported one Build execution, one source completion
and one publication including the Build artifact; checked product count was one.
The included legacy driver's settled-check counter was zero but has no production
caller at this base. Checker-call count was not measured. Accepted checked
outcomes, exit status and exact output are the acceptance oracle.

## Counter-free original CLI controls

Both slots used the same archived ordinary compiler, default
`--check --offline`, unchanged source/Build/library inputs, empty stdin and a
600-second bound. Each used one warmup per slot plus eight alternating ABBA
pairs. A separate successful probe froze exact stdout/stderr/exit; all 36 paired
and warmup invocations across the two controls matched their oracles. The probes
are retained separately. There is no optimization candidate in this experiment.

| Control | Wall median A / B | Peak resident median A / B | User CPU median A / B | System CPU median A / B |
| --- | ---: | ---: | ---: | ---: |
| Scalar | 3.786 / 3.544 s | 1,000,835,072 / 1,003,767,808 B | 3.336 / 3.148 s | 0.320 / 0.320 s |
| Borrowed | 3.134 / 3.265 s | 1,011,427,328 / 1,008,236,544 B | 2.844 / 2.914 s | 0.398 / 0.250 s |

Scalar's same-executable paired wall difference was -3.91%, with 95% bootstrap
interval for the median difference [-473.732, -95.107] ms. Borrowed was +3.24%,
[-72.271, 160.094] ms. The nonzero scalar control difference prevents treating
small differences as an optimization result. These are observed control runs,
not a reproducibility guarantee; intervals omit between-run drift and workload
selection uncertainty.
The shared workstation was not verified quiescent: other chats' compiler jobs
were observed during the session but were not tracked per observation.

Peak resident measurements use Windows `GetProcessMemoryInfo` PeakWorkingSetSize;
CPU uses `GetProcessTimes`. Their scope is the primary compiler process, excluding
descendants. Instrumented process peaks include snapshot serialization and are
not substituted for these ordinary CLI peaks. `--verify` was excluded because
it withholds accepted package checkpoints/restricted grants at this base,
changing the workload; it does not request independent stage validators.

## Snapshot custody and replay

Both accepted captures exported seven snapshots (ordinals 1,2,3,4,9,10,14).
Replay verified the completed record, log digest, exact roster, each byte length
and SHA256, then bounded decoding, semantic table equality after cloning and
unchanged ProgramIdentity: **14/14 passed**. Snapshot decoding omits/reconstructs
derived indexes, so replay does not recreate live allocation state. For example,
the scalar activation live clone requested 31,835,318 bytes in 14,125 allocations,
versus 31,700,164 bytes in 13,650 allocations for the reconstructed owned payload.
Live counters, not replay extrapolation, establish the copy cost above.

Private export/replay limits are 256 MiB encoded bytes, 2 GiB decoded allocation,
134,217,728 cumulative sequence elements, depth128. A finite validated runtime
setting can adjust packaging without rebuilding. The same parser/configuration
is recorded and used by capture and replay; compiler/proof/wire limits are
unchanged. The first scalar run exceeded the old 4,194,304-element cap at byte
28,160,139. The next completed checking but failed a stale auxiliary counter
assertion. Neither failed run contributes final measurements.

## Reproduction and identities

[Portable commands and boundary definitions](../../../_recycling/tools/benchmark/selected-bodies/README.md)
cover preserving the producer, freezing inputs, applying the opt-in capture patch,
building via mbx, generating plans, capture/replay, and same-executable controls.
No PowerShell or Python reproduction artifact is required. The patch is pinned
to this historical compiler base; adapting it to another compiler revision needs
source review and new measurements. It is reversed before landing: ordinary
compiler dependencies, allocator and execution contain no hooks.

SHA256 identities:

| Artifact | SHA256 |
| --- | --- |
| Original ordinary omega | `72054a3f4c9e64cc98e99b4eda3b70aa3aab0e59bdc4f2b1d4c8372c8fb8a6d4` |
| Capture executable | `2cc28cf98fc94988b69d61f9af9bcc3816a7b64caf63ff176fad9e215ec3de34` |
| Replay executable | `0bbd035394dcd6b2875755fe18d1043b09199fed104ae85f8a777d244aa6b831` |
| Scalar capture/control omega-bench | `b1a928932c4a8519727f4f9f00f332c4d6395ec5ceb801021fb118a42ca969a7` |
| Borrowed capture/control omega-bench | `0da365e1f0e0e7b92d19f7d97fa9ac7b3c102e8e68c136975be90838349a3a8b` |
| Applied capture.patch | `b97e4927f6420d8a422aee52cb6b02b234ac0ec7e9ca5d5c4ff22ae457a5aa12` |
| Scalar accepted clone log | `f857b34aca0d563eab17e32890bb09c98bf596350c901d8bd6aec463b32b8a28` |
| Borrowed accepted clone log | `5836e62198d29bff0cf751dad1640d00350d672314e350071574dea09e41cda3` |
| Scalar accepted profile record | `ff4a2de5fc46dabe58b67a04cf249a89bc0c5a79812a07f8bfb94a173dec5ae2` |
| Borrowed accepted profile record | `fe48e2d2e1ff53672cb601816ffed4a88a0d2dfb3e2c007916d243a55e3c7ac8` |

Raw evidence remains ignored under worker31
`build/performance/selected-bodies/`: baseline binaries/log/identities, original
`inputs` and replacement `inputs-v2` manifests, `capture-v1/v2/v3`,
`profile-scalar-01/02/03`, `profile-targets-01`, `profile-borrowed-01`, both replay
logs, both source plans/probe oracles/control journals/reports. Each accepted
`profile.json` retains every snapshot digest, effective limits, executable and
input identities. Failed records and original manifests remain untouched.

The separate constant-validation clone comment now says its owned tables are
copied once per validation. This clone is outside the captured continuation
sites; no measured frequency, allocation, or time is claimed for it.

Scoped validation used mbx: omega-bench's 21 unit tests passed; 16 existing
runner tests passed initially and the seventeenth passed after updating its
expected backend-list message. Its invalid-backend refusal/no-run assertions
remain intact. Release all-target Clippy passed for omega-bench (fixtures
enabled) and the private allocation observer; both capture/replay examples
passed release Clippy. The observer's thread-local test and private finite-limit
parser test also passed. Formatting and diff whitespace checks passed. No
compiler behavior change or additional sample/native regression was needed.

## Retained side-record follow-up on Linux

A later ownership repair keeps unchanged side records in immutable batches. A
successor copies the small directory that names the batches; appending output
never clones an old record payload. Logical ordering remains separate from
record identity, and every row read still checks its owning arena's generation
and extent. Rewriting one row copies that row only. This is a bounded side-table
repair, with the existing body activation, preparation and validation order.

The following Linux observations are separate from the historical Windows
measurements above. Baseline source is
`2b586716b443764af2b88616403b9c8b8ee1f2d0`, a test-only descendant of
`fe6e3d2e0104859a25ebfd1b0bfe4b424885e75f`. The controlled comparison excludes
the later canonical-array-identity repair. Both use optimized development
builds (`CARGO_PROFILE_DEV_OPT_LEVEL=1`), the same feature union, three build
jobs, CPU affinity 0–2, and one admitted team build/runtime chain. Captures use
thread-local allocation counters; timing controls use the ordinary System
allocator without counters. The source input and harness are byte-identical
between each matched pair.

The nonempty workload invokes the public resolver and typing stages for 64
selected helpers in sequence. Each helper adds the two requests the unchanged
computed-array program requires: type-expression evaluation and array-extent
evaluation. It retains 128 exact request rows and one pending length body.
Across successor pairs it observes 4,032 old rows; baseline copies each old
payload, while the ownership repair keeps every old row at its old address.
These observations verify sharing through real nonempty completion, including
its first append. They are not 4,032 production copying operations or a
whole-program acceptance claim.

The final controlled source is `b6b44c31fe8693380fcee5aef7ceef540dc6041c` before unrelated main changes.
Across 64 completion/deferral windows, requested allocations change from
261,529 / 49,973,726 bytes to 262,103 / 49,463,494 bytes: 510,232 fewer requested
bytes and 574 more metadata allocations. The record payloads stay shared even
through the first append; this is not whole-Vec copy-on-write. Separate actual-owner probes show that the refusal fork and first append
allocate roster and batch directories while preserving all old record payloads;
these probes are not production call-frequency measurements.

Twelve alternating baseline/candidate pairs, plus one warmup per slot, used
fresh counter-free processes. Whole-driver wall medians were 119.834 / 125.655 ms;
the median paired relative change was +3.169% (95% paired-bootstrap interval
+2.218% to +5.465%). Completion/deferral phase medians were 71.489 / 75.402 ms,
with paired change +3.820% (+2.906% to +6.056%). The separate pointer oracle
medians were 0.461 / 0.848 ms; their difference cannot explain the completion
cost. Process RSS medians were 40,832 / 42,496 KiB. Validated roster-to-record
indirection adds a bounded read cost, rather than repeating the old payload
clone. Inline readers and the existing fresh-generation root invariant reduced
the initial measured cost; all retired-slot checks remain.

The nonempty workload does not change its allocation counts when the separate
derived-composite repair is added, so it does not establish that cache's
benefit. A separate actual `TypeIdentityTable` owner probe forks 512 existing
composite keys, then measures its real `Arc::make_mut` detachment without new
output. Before the repair it requests 1,538 allocations / 322,514 bytes and
shares none of the old key vectors. After the repair it requests 2 / 17,472
bytes and shares all 512 old vectors and their text payloads. Existing borrowed
key hits request zero allocations in both sources, with the query parts already
constructed outside the observation window. These are isolated owner
allocation counts, not production call frequencies or compiler timing.
Canonical numbering, independent fork text ownership, wrong-table refusal and
cache decoder-budget controls still pass. Only new keys allocate new key owners;
map-directory detachment remains real work.

The two original frozen Build/check routes also pass with exactly the
baseline stdout, empty stderr and exit zero. Both keep one Build execution,
one source completion and one publication attempt including the Build;
the legacy settled-check counter remains zero. These counters are not a
checker-call measurement. Both still select 76 sources with no pending selected
body, then complete 1,686 scalar or 1,684 borrowed-root authored machines,
followed by an empty batch.

Each empty activation has 90 disjoint observed field windows. Their requested
allocations change identically for scalar and borrowed-root input:

| Observed owner | Baseline allocations / bytes | Candidate allocations / bytes |
| --- | ---: | ---: |
| Deferred refusals | 1 / 24,128 | 2 / 32 |
| Applied-domain applications | 32 / 22,496 | 2 / 16 |
| Authored service-reach rows | 284 / 20,680 | 2 / 24 |
| Callable identities | 46 / 9,768 | 8 / 64 |
| Service definitions | 53 / 3,840 | 2 / 24 |
| Service rows | 11 / 312 | 2 / 16 |
| Semantic domains | 50 / 6,695 | 2 / 24 |
| External bindings | 1 / 32 | 2 / 16 |
| Ten smaller raw row arenas | 10 / 15,192 | 20 / 176 |
| Proof source spans | 1 / 29,664 | 2 / 72 |
| Ranking custody | 57 / 4,928 | 2 / 16 |
| Borrow factories | 1 / 168 | 2 / 16 |
| **Measured changed owners** | **547 / 137,903** | **48 / 496** |
| Unchanged owners and directories | 50 / 756,896 | 50 / 756,896 |
| **All disjoint activation windows** | **597 / 894,799** | **98 / 757,392** |

The candidate's 496 bytes in the changed owners are copied ownership-directory
metadata, not retained record payloads. Expression and type-reference directories
remain among the 756,896 unchanged bytes. These sums therefore establish a
137,407-byte / 499-allocation reduction per observed empty activation, not zero
activation cost or a whole-compiler saving. Immutable batches may also retain a
removed record until its batch owners are dropped; pointer sharing alone does
not establish a smaller resident footprint.

Original capture durations were 47.115 / 48.463 seconds for scalar and
45.106 / 44.606 seconds for borrowed-root baseline/candidate. Instrumented runs
are allocation evidence only; these single durations are not a timing result.

Counter-free original controls used the saved MiMalloc package-Build/check
producer with the private observation branches disabled in both slots. Each
input had one warmup per slot and two fresh-process pairs in AB then BA order.
Every invocation reacquired the shared resource guard, so queued functional
work could run between observations. Guard wait is outside the reported wall
interval. All streams, input digests and exits matched their fixed oracle.

| Original control | Wall median baseline / candidate | Two paired wall changes | RSS median baseline / candidate |
| --- | ---: | ---: | ---: |
| Scalar | 44.865 / 44.162 s | -0.954% / -2.179% | 809,726 / 796,880 KiB |
| Borrowed-root | 46.540 / 44.930 s | -5.850% / -0.954% | 822,728 / 825,736 KiB |

These four observed pairs show no slowdown on the two representative inputs,
while resident results differ between inputs. Two pairs are too few to establish
a population timing or memory improvement; the controls support this bounded
ownership repair, not a default-CLI or total-compiler speedup claim. Both original
programs also run on Linux x86-64 with native exit zero and return Unit after
canonical Psi serialization and reload. Their bytes and authored assertions are
unchanged. The first native harness attempt expected two work-counter events
where `measure()` records one before host execution; it failed before execution
and remains a failed receipt. Only corrected fresh runs contribute acceptance.

An initial unpatched `orchestration::package_batch` producer on the rebased Rust source
also accepts both unchanged original programs: Linux native exit zero and
canonical reloaded Psi Unit, with empty stderr. Its executable SHA256 is
`1b34d2a3f0a3636598137b0424ff1e7757ed5ba5043c48b85bd790bfe06a62eb`.
The build uses qualified mbx `1.29.2-omega.1`, SHA256
`5c19a1a26f866928efcd2ccbb7d61ae1d39f01f762a49de361d91ff32aaf1e60`,
with the same optimized development profile and resource limits. This default
route qualification is separate from the frozen-source performance data;
the final span-fixed producer and exact-squash replay are qualified separately.
Parent and child table observations overlap and must never be summed. Existing
key probes isolate the actual recorder but repeat it artificially; their call
counts are not production call counts. Keep payload copies, directory metadata,
derived-cache detachment and new output distinct.

The requested-byte metric counts successful allocation and reallocation
requests on the observed thread, not allocator footprint, physical traffic or
resident memory. Linux RSS is `RUSAGE_CHILDREN.ru_maxrss` in an isolated runner
with one timeout-wrapped child: maximum RSS of an individual reaped process,
not simultaneous memory of a descendant tree. Ordinary wall intervals omit
between-run drift and workload-selection uncertainty. No total compiler
speedup follows from these selected windows.

Seven other top-level vectors had no observed activation allocation in either
original frozen workload: `applied_data_cleanup_selections`,
`plan_laid_layouts`, `access_view_plans`, `wire_schema_plans`,
`target_machine_families`, `evidence_forwardings` and `proof_output_calls`.
This repair does not migrate those unobserved tables or claim universal sharing
of every possible typed field. It closes the measured retained-record owners,
raw arena payload copies and derived composite-key payload copies. Directory
metadata, changed rows and new output retain their own costs.

After rebasing onto the canonical-array-identity prerequisite, scoped validation
passes 136 arena tests, all 226 typed-tree tests, the retained-path test, four
source handoff tests, the generic layout-owner test, both affected mathematical
input tests and all-target Clippy for the eight changed crates. Two neighboring
language-semantics tests still report `MalformedIdentity` exactly as at the old
source: `generic_arguments_visit_type_and_const_owners_without_widening_type_roots`
and `dynamic_trait_identity_reads_generic_arguments`. Their parser and test
files are byte-identical to baseline; the other 76 language-semantics tests
pass. No program assertion or parser rule was changed to conceal them.

Mutable span access also retains its original bounds behavior. A checked member
write validates the complete span and ordinal before detaching a record; batch
remapping validates the full span once before any callback. The actual callable
remapping regression first reproduced a changed live prefix before rejection of
a malformed tail, then passed with the shared checked visitor. The controls cover
out-of-span live neighbors, empty and oversized spans, wrong/reused generations,
no detachment on refusal, and unchanged predecessors. These final review repairs
are qualification changes after the frozen performance sources, not new timing
observations.

The real two-helper completion regression failed at the old source and passes
with retained owners. Its exact request ledger, unchanged predecessor, and
unrelated-predecessor refusal remain checked. Generic owner tests exercise
append, sorted insertion, point writes, predicate unwinding, stale/reused
handles, legacy bytes and tighter allocation budgets. The cache's new-key
pointer regression likewise fails before its repair and passes afterward.

[Portable follow-up commands](../../../_recycling/tools/benchmark/selected-bodies/README.md#retained-side-record-measurements)
retain the shared Rust driver and opt-in observer patches. Default dependencies,
allocators, stage APIs and runtime execution contain no observer hooks. The
portable driver adapts formatting and its module path, so rebuilt executables
need new identities and observations; the measurements above use the archived
embedded driver SHA256
`778b1c40a34168e3959732c204c8c938b98ea60c5a76bcc2ab2a9d43b454e35e`.

The frozen library manifest contains 187 files / 1,342,964 bytes. Its SHA256 is
`e29e152d90b228f45c224b267cdc17f1b9ce51f8bc57a5b2ff2eb9fb33bb909f`;
the fixture main/Build hashes remain those in the first table. Accepted capture
and control executable identities are:

| Executable | SHA256 |
| --- | --- |
| Build allocation observer, baseline | `36811b2702c96c30d9d81e5ec96abf890bc9c8c73ece9fe3e9115781ed8d3eb1` |
| Build allocation observer, candidate | `e5b18baae791663e56ce6d1b2680288df5544c7b376e7549856673932c6ea58d` |
| Build control, baseline | `9b174b3c7e2f52be6db3fa89de9884a22887dae0a8824c9beb79477128fbf9d5` |
| Build control, candidate | `d217bf36d1ad2acae3797781ba5152260d7974dd8d9af43fa61128bbb2dfe4c2` |
| Nonempty allocation observer, baseline | `bf5df1beaf20e212ae5b8bbe8b3df2c5e7a8fa11073c635017ece3d6d1970b4b` |
| Nonempty allocation observer, candidate | `d78d63394c6b44330929da23dde75a05ca49fdee96a63a2ab588d56061e0fd74` |
| Nonempty control, baseline | `230573d8a6b4d6ede0ff08096fb088171b6545ef5ecf8841f7d7e63971727a7d` |
| Nonempty control, candidate | `c486cdfda59b9949548909747851d322824b5da2dd34b19d53b0046e8f2c08b2` |

Raw receipts remain ignored under worker-5 `build/side-record-sharing`:
`baseline`, `baseline-diagnostic-02`, `candidate-final-01`,
`nonempty-final-pairs-01`, `composite-baseline`, `composite-candidate` and
`candidate-route-final-01` and `original-controls-02`. Each build records its complete source and private
patch, profile, commands, logs, input identities and executable hashes. Initial
unavailable-time-tool and sandbox-UID custody failures remain qualified failed
records; they contribute none of the accepted observations.
