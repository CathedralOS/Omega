# Placed-result discovery: bypass rejected

The early resolver produces no placement selections on the three original
workloads, but its output is required on placed-result paths. Removing it
preserves the tested native bytes while silently dropping some invocation
locations used by placement admission. **Reject unconditional removal. No
production compiler change is retained.**

This continues the [boundary measurement](../README.md). The experiment asked
what `retain_applied_results` actually consumes and whether its full resolver
could simply disappear. A temporary binary supports unchanged discovery (`full`)
and a diagnostic early return (`skip`). Both use the same executable and all
other compilation paths. The bypass is deliberately not a proposed default.

## Measured work and consumed output

| Input | Resolved expressions | Calls scanned | Machines in probe | Selection records | Placement applications | Invocation locations, full → skip |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `length/local` | 5,575 | 224 | 126 | 0 | 0 | 0 → 0 |
| `dyn_dispatch_selector_payload_bound/local` | 5,559 | 224 | 126 | 0 | 0 | 0 → 0 |
| `nested_nine_level_record_field_bound/local` | 5,601 | 225 | 127 | 0 | 0 | 0 → 0 |
| returned placed view | 5,613 | 229 | 129 | 6 | 1 | 4 → 2 |
| forwarded placed view | 5,615 | 230 | 130 | 9 | 1 | 7 → 4 |

Each discovered call result contributes three selection records: policy
declaration, schema declaration, and the sealed `Placed` declaration. Their
source coordinates retain both the exact declarations and the use sites. The
returned-view program's surviving two locations are explicit type annotations;
the bypass loses the factory-call and returned-call locations. Forwarding adds
another call location and loses three in total.

In the original three workloads, skipping the pass removes one numeric-probe
resolution and one expression-table clone: eight copies become seven, with
5,575 / 5,559 / 5,601 fewer live rows copied. All instrumented counts and field
totals repeat exactly in both modes. This establishes work avoided by the
diagnostic bypass; it does not establish that the bypass is valid generally.

The consumer in
[`applied_results.rs`](../../../../../omega-rust/psi/compilation/compile-time/src/layouts/placed_views/applied_results.rs)
needs selected call targets, declared return types, exact static actuals, and
sealed borrowed-View prototype validation. It does not consume executable body
semantics. However, resolving call targets still needs ordinary lexical and
declaration selection. Replacing this with spelling checks would not preserve
that contract. The existing const-argument lexical-input selector only lowers
type/index inputs, not these ordinary call-result occurrences, so it is not a
drop-in replacement either.

[`placed_views.rs`](../../../../../omega-rust/psi/compilation/compile-time/src/layouts/placed_views.rs)
retains the invocation locations and passes each to
`require_common_floor_for_invocation` before evaluating the placement policy.
The [placed-access contract](../../../../../wiki/spec/resources/placed_access.md)
requires exact policy/schema identities, visibility and direct-dependency
authority before synthesis. Losing these inputs is a semantic difference even
when the local examples emit identical machine code. The four invalid controls
tested here do not demonstrate an accepted invalid program under the bypass;
the rejection decision is based on the lost admission inputs and placement
demand, not a fabricated runtime soundness failure.

## Quality evidence and limits

The [42 observations](observations.json) contain 21 successful native executions
across five positive inputs, with identical image hashes per subject in baseline,
full, and skip modes. All four intended invalid controls retain their baseline
diagnostics in both modes: wrong backing, overlapping loans, ambiguous attached
policy owner, and attempted mutable access from shared storage. The two placed
positives also pass native and Terminal execution in all three modes: six actual
`run --both` observations in [controls.json](controls.json).

The instrumented full mode matches baseline images and failure diagnostics.
Diagnostic comparison strips only probe records and the benchmark harness's
recognized timing lines. Its first attempt compared timing lines as diagnostics;
that assertion failure is preserved in `summary-initial.log`, and raw outputs
remain unchanged. [summary.json](summary.json) contains the verified counts.

Three exploratory witnesses remain explicitly classified as baseline failures:

- [Inline field use](witnesses/inline_result/main.omg) creates one placement
  application in baseline/full mode, then fails Terminal lowering with
  `argument field requires its source record or selected case`. Bypass mode
  creates zero applications and instead fails earlier because the placed read
  lacks an exact accessor/value tuple. This proves a downstream dependency on
  discovery, but is not a passing source-to-native witness or a newly introduced
  baseline regression. The existing lowering failure was not repaired here.
- [Implicit discard](witnesses/discarded_result/main.omg) is rejected because a
  non-unit result must be consumed or explicitly discarded. It is an invalid
  draft, not performance evidence.
- [Explicit discard](witnesses/explicit_discard/main.omg) still fails on the
  baseline's opaque-representation obligation. All modes produce zero
  applications and the same refusal. It supplies no runtime acceptance.

These original failures, sources and hashes are retained rather than replacing
their oracles to manufacture a passing comparison.

Timing is inconclusive. The one-minute load average ranged from 7.38 to 19.07;
the ordinary `length` full runs took 1,054 and 1,997 ms while skip took 1,046
and 3,962 ms. The full/skip order reverses for the second ordinary-workload
round, but two samples under changing contention do not establish a speedup.
Probe logging and detail collection also add overhead. No timing sample is
discarded, and no memory-saving claim is made.

## Identity, replay and next experiment

Compiler source base: `114906953e11e283b8082f03d8da99fbf39dff7e`; this checkout
was at the preceding evidence-only commit `21c23c3cca858725394e4eb19f0044d1f3103e55`.
Fetched `origin/main` was `ab30b42820ea94873f5b6da512fe14130b648d1a`; the placed-view
discovery files and resolver package were unchanged from the frozen base. Other
incoming changes exist, so this remains a measurement of the recorded base.

The diagnostic executable SHA-256 is
`a2ec7aed3b9aaa415be90436dba2fdaea02d1b79ae1e2cc810bd3072adab3c5c`.
The [profile receipt](instrumented-producer.json) binds effective opt-level 1
for compiler library, Omega library, and CLI to the actual artifacts. The
baseline is the unchanged frozen executable from the parent record. Inputs,
library tree, binaries, raw commands and output hashes are in the observations.
The build waited 232 seconds for a slot and took 3m43s; those durations are
workflow observations, not compiler-runtime benchmarks.

Replay from the recorded base with the parent frozen baseline available:
create `build/placed-discovery/base.txt` containing the source SHA, then run
`build.py` from this directory through `tools/build_slot.py`. It installs the
temporary probes, freezes the diagnostic producer, and restores exactly its
recorded compiler paths. Run `capture.py`, `controls.py`, and `summarize.py`
from the repository root. The [probe patch](instrumentation.patch) is preserved
for inspection. Failed builds may leave probes installed; preserve the failure
and restore the listed paths before resuming. No compiler probes or bypass
remain in the retained source.

The next implementation candidate is a compact selected-call-result carrier
produced by the existing resolver's lexical selection, with exact declaration,
argument and invocation identities preserved. Compare its complete selected
output with this full-probe reference before replacing the consumer. A name
filter, a deleted invocation, or a cache keyed only by a source revision would
not discharge that requirement. The inline-result lowering gap remains a
separate baseline limitation when choosing its executable witnesses.

[Research notes](research.md) record the primary-source query and why a general
query cache was not selected for this experiment.
