# Real Build routes copy retained typed storage

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

[Portable commands and boundary definitions](../../../tools/benchmark/selected-bodies/README.md)
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
