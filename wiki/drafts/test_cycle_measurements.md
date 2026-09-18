# Test-cycle measurement reference

Temporary evidence for choosing the next local test-performance experiment,
especially distinguishing compilation, test execution, and repeated rechecks.
Remove this note when a controlled replacement measurement establishes the
relevant scheduling/selection costs, or when the measured workloads are no
longer used. It is not validation policy or a current checkout baseline; see
[AGENTS.md](../../AGENTS.md#validation-scope) and
[selector usage](../../tools/testing.md).

## Scope

Recorded on 2026-09-05 on Windows, AMD Ryzen 9 7950X (16 cores, 32 logical
processors), mbx 1.8.1. The scheduling samples used existing library binaries
without compilation on an active workstation; background desktop/indexing work
was not disabled. The recorded worktree head was
`014327fb2ce19367a8b14468641fa78be185fc16`. These are workload timings, not a
measurement of hardware capacity. macOS scheduling performance was not measured.

## Scheduling samples

All durations are seconds. Each source run had 150 passes, one local-origin
failure, and 12 ignored tests; manager runs had 262 passes and two ignored tests.
Those are historical outcomes, not claims about today's tests.

| Workload | Recorded samples |
| --- | --- |
| package-source, 32 threads | 66.180, 63.884, 62.318 |
| package-source, 8 threads | 48.332, 58.196, 53.957 |
| package-manager, 32 threads | 65.608, 68.279, 76.395 |
| Both package binaries together, 8 threads each | 95.654, 96.588, 105.522 |
| Source confirmation, 8 threads | 61.203, 64.053 |
| Source confirmation, 32 threads | 64.697, 59.051 |

The confirmation runs had no other agent-owned build/repository work alongside
them and did not reproduce the apparent eight-thread advantage. Overlapping
the package binaries reduced their combined elapsed phase compared with the
sum of standalone measurements, but substantial interference remained. These
results do not justify a universal thread cap. Splitting Cargo package builds
also changes feature-unification conditions: package-manager enables
package-source's `test-fixtures` feature.

## Whole-run and selection limits

A single full eight-thread workspace run at the recorded revision took 168.522
seconds including build work, with 7,681 passes, three failures, and 14 ignored
tests across 117 targets. The failures were the package-source local-origin
case and two platform-custody persistent-journal cases. This uncontrolled run
does not establish a workspace speedup.

A nextest 0.9.140 experiment with per-package groups of eight stopped after
240.846 seconds total (57.96 seconds compilation) and was incomplete. A later
nextest library run on a migration tree based on
`ac3b45e9b462def078772e5bd5ca31be0a544ec0` took 357.071 seconds of execution,
following roughly 72 seconds of compilation, with 7,684 passes, the same three
reported failures, and 14 ignored tests. Neither establishes a nextest-only
speedup or a passing baseline.

An actual narrow nextest listing for an `x86-encoding/src/lib.rs` change selected
1,067 active library tests in 20 library binaries, compared with 7,687 tests in
117 binaries normally. This measured test-count reduction, not elapsed-time
improvement; package-manager remained selected.

Three warmed Windows README-only selector CLI trials took 28.060, 21.888, and
23.469 seconds end to end. Each ran 448 architecture checks and one corpus
audit, with no library tests. These samples include metadata, nextest
startup/build checks, and execution. Different cache conditions prevent a
matched comparison with the earlier full library run. The useful result is
that document-only selection omitted unrelated library execution, not a claimed
full-suite speedup ratio.

## macOS package-review classification experiment

On 2026-09-13, macOS ARM64, base
`0989d752aeef9bb17b36f25b6c0ee993e0752d29`, the unchanged hosted-consumer
review test was compared with an unpublished crash-guard classifier prototype:

```sh
RUST_MIN_STACK=67108864 cargo nextest run -p package-manager \
  --test semantic_binding_review --no-fail-fast \
  -E 'test(=macos_entry::target_entry_dependency_discovery_requires_explicit_consumer_acceptance)'
```

Both used the same checkout, shared target directory, default debug/test
profile, and unchanged application/standard-library inputs. No concurrent
agent-owned builds or tests ran. Each passed discovery, missing-acceptance
rejection, exact explicit acceptance, and stale-binding rejection.

| Implementation | Rust build (seconds) | Test execution (seconds) |
| --- | ---: | ---: |
| Unchanged base | 39.18 | 540.531 |
| Snapshot-local integer classification | 8.57 | 532.465 |

The prototype traversed the existing live declaration roster once per immutable
crash-coverage pass, retaining first-match integer/float classification in a
sorted vector keyed by full generational symbol identity. Queries used binary
search instead of rescanning machines, states, locals, owned data and fields.
It preserved existing expression classification and added four passing
equivalence/boundary tests. The affected crate ran 3,673 tests: 3,661 passed;
all 12 failures also reproduced, with the same diagnostics/assertions, in a
focused unchanged-base run.

The single paired execution difference is only 8.066 seconds (about 1.5%);
it does not establish a material whole-route improvement. Different rebuild
work makes the build-time difference unsuitable as a speedup claim. Release
performance and repeat-run variance were not measured. The prototype was
discarded rather than adding an index on this evidence.

The next CRASH-GUARD-COST investigation should measure cumulative phase costs
and repeated checking across the whole review route before choosing another
lookup structure. A two-second sample of the unchanged run reached preliminary
flow-fact construction, illustrating that the earlier crash-classification
sample does not attribute the entire nine-minute route. Neither sample measures
a phase's total share. Preserve the explicit review controls above; no thread
cap, weakened checking, or disabled observation contract follows from these
measurements.

## Windows package-review crash-classification attribution

On 2026-09-15, Windows AMD64 (Ryzen 9 7950X), mbx 1.11.0, base
`24a88339d0a761a07e60e2fbd8f3aac61539947e`, the same hosted-consumer review
test ran unchanged inputs:

```sh
mbx nextest run -p package-manager --test semantic_binding_review \
  --no-fail-fast -E 'test(=macos_entry::target_entry_dependency_discovery_requires_explicit_consumer_acceptance)'
```

The route requires a local two-line `pub(crate)` repair in
`build-output/src/capture.rs` (`is_executable`, `same_file_observation`):
commit `0b404debf1bfea59784cb633ff2bd97fcd352a2b` left the `#[cfg(not(unix))`
definitions private, so this base cannot compile package-manager on Windows
without it. The repair stayed uncommitted and is unrelated to the measured
change.

| Implementation | Rust build (seconds) | Test execution (seconds) |
| --- | ---: | ---: |
| Unchanged base | 56.43 | 708.137 |
| Unchanged base, counter-instrumented | — | 729.687 |
| Declaration-roster classification | 52.42 | 773.603 |
| Declaration-roster classification, repeat | 1.97 | 718.452 |

The instrumented unchanged run appended per-pass counters for
`infer_path_conditioned_guard_coverage` across the whole route (18 passes, one
per checked compilation the review performs):

| Counter | Total |
| --- | ---: |
| Coverage inference elapsed | 6.036 s |
| Checked crash calls | 8,376 |
| Checked crash sites | 0 |
| Incoming-guard applications | 6,892 |
| Entry-meaning filter elapsed | 0.029 s |
| Expression integer classifications | 37,976 calls, 5.548 s |
| Symbol integer classifications | 24,938 calls, 4.226 s |

Every symbol classification rescanned all machines, states, parameters,
locals, machine-owned data, and data members until first match, so repeated
classification was about 70% of the coverage phase and 0.6% of the route.
The landed replacement builds that same roster once per immutable pass —
state parameters, locals, machine-owned data, then data members, in the same
order, keeping the same first-match answer per generational symbol — and each
query is one slot read. It also hoists the per-machine mutable-parameter set
out of the per-guard entry-meaning filter and computes each binary
comparison's operand classification once instead of twice.

The paired execution samples span 708–774 seconds, so run-to-run variance
exceeds the attributed 4.2-second phase saving; no whole-route speedup is
claimed. All crash-guard positive and negative coverage for parameters,
locals, and fields is exercised by the crate's existing checking tests; the
route passed acceptance and stale-binding rejection on both implementations.
Release timings were not measured.

## macOS package-review state-shared guard classification

On 2026-09-17, macOS ARM64, mbx 1.11.0, base
`cec67dc6783664c11e6d13a2a7cd64aaebcd4f6c27`, the hosted-consumer review test
ran under per-pass counters appended inside
`infer_path_conditioned_guard_coverage` (the counters were removed before
landing; both runs carried the same counter code shape). The test target is
now `suite` and the test path gained the `semantic_binding_review::` module
prefix:

```sh
RUST_MIN_STACK=67108864 mbx nextest run -p package-manager --test suite \
  --no-fail-fast -E 'test(=semantic_binding_review::macos_entry::target_entry_dependency_discovery_requires_explicit_consumer_acceptance)'
```

The route performs 18 checked compilations (the application plus standard
library under discovery, missing-acceptance, exact-acceptance, and
stale-binding review); each appends one counter line. Per-pass medians on the
two program shapes and route totals:

| Counter | Per-call derivation (base) | Per-state classification |
| --- | ---: | ---: |
| Large-program pass elapsed | ~190.7 ms ×10 | ~99.4 ms ×10 |
| Small-program pass elapsed | ~23.6 ms ×8 | ~11.3 ms ×8 |
| Coverage inference route total | 2,098 ms | 1,094 ms |
| Checked crash sites / calls | 0 / 8,376 | 0 / 8,376 |
| Entry-meaning conjuncts consumed | 2,838 | 2,838 (identical) |
| Classification derivations | 8,376 (one per call) | 3,736 (one per state) |
| Conjunct derivations | 2,838 | 1,730 |
| Transitive-closure element scans | 11,746 | 5,254 |
| Test execution (instrumented) | 788.525 s | 832.957 s |

Every checked call previously re-admitted its state's applicable incoming
guards through the entry-meaning provenance filter, re-encoded each conjunct's
canonical path predicate, and re-ran the integer-order transitive closure —
although `IncomingGuard::applies_at` keys the whole set by state. The landed
slice builds one `StateGuardClassification` per state hosting checked work
(3,736 builds for 8,376 calls) and lets each site or call clone the shared
conjunct identities and closed consequence set; only the statement-local
fallthrough still derives per site. The stored fields are sorted sets, so the
join is outcome-identical. The paired executions differ by less than the
observed run-to-run variance, so the ~1.0-second removal is a phase-level
result, not a claimed route speedup.

Remaining attribution within the pass after the slice (per large-program
pass): per-state classification ~66 ms (entry-meaning provenance walks and
canonical encodings, now one per state), per-machine guard/fallthrough/entry
collections ~15 ms, content-conservation plans ~5.6 ms per pass, the integer
roster ~0.4 ms once per pass, and the per-call join ~4.4 ms. Coverage
inference is about 1.1 s of the ~540–830 s route (~0.15%), confirming the
item's earlier finding that crash-guard classification is a minor share; the
named remaining candidates `validate_specialized_program` and
`build_flow_facts` sit outside `checks/crashes.rs` and were not measured here.
The `omega --check` whole-route probe on
`tests/omega/pass/terminal_psi/integer_control_contract` stayed at a 4.63 s
median over 5 warm debug runs (documented baseline 4.90 s).
