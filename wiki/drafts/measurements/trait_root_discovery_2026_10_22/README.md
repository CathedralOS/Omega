# Avoid repeated trait-root discovery work

This is the next bounded compiler-throughput experiment after
[declaration field inventory reuse](../compiler_throughput_2026_10_21/README.md).
Its baseline is local commit `1c2376d0575`, including that preceding improvement.
The target is full sample-to-native compilation on this Apple M4 macOS ARM64
host with the repository's optimized dev profile, not faster generated code.

## What the 21 calls mean

The length workload traverses an initial source program, a private compile-time
projection, a seeded continuation and executable preparation/checking. Those
program states differ. In particular, preparing logical callable signatures adds
signature-owned occurrences that later discovery must see. Root counts alone
cannot establish unchanged inputs, so a persistent cache or one shared answer
across all calls would be unsound without further invalidation analysis.

[The call trace](trace-summary.json) shows four projection calls and one retained
implementation validation call with no retained applications to consume their
answers. The candidate omits only those five queries. It still validates the
nonempty collection containing six retained selections, and still prepares fresh
roots after source mutations.

Each query also used three identical `TypeOccurrences` visitors over the same
immutable program, consuming a different output list from each. The candidate
collects those lists once and passes them to the existing consumers. This changes
21 root queries and 63 occurrence walks to 16 queries and 16 walks on length.
The query counts are observed; walk counts follow the three baseline call sites
and the single candidate call site. These counts concern this query only, not all
source traversal throughout the compiler.

The attribution captures measure 47.844 ms and 39.493 ms in root queries,
respectively. These single instrumented captures locate avoided work; they are
not the controlled end-to-end performance comparison. Tracing was removed before
building the final candidate.

## Why behavior should remain unchanged

`TypeOccurrences` already collects all three lists together. The change retains
its type/reference deduplication, lexical owner handling, call qualification and
conformance selection rules. Conformance handles remain in static-application
arena iteration order; root categories and their concatenation order are
unchanged. Source-field discovery consumes the same ordered type occurrences.
Two obsolete single-output helpers were removed after migrating their only
consumers. The standalone conformance-occurrence query remains for its other
callers and uses the same ordering operation.

The shared census belongs to one immutable `applied_trait_roots` call. It is
construction scratch, with no persistent cache, program revision heuristic or
cross-stage invalidation protocol. It includes applied signatures and retained
source scopes, rather than scanning discarded arena nodes.

Projection still prunes removed owners before testing whether its remaining
application collection is empty. Empty validation returns only from the retained
selection check; the caller continues validating ordinary authored arguments.
For every nonempty collection the original liveness, scope identity, supplying
application and argument checks execute unchanged. No proof or trust boundary,
refusal expectation, source oracle or target selection changed.

[Research and the rejected cross-mutation sharing approach](research.md) records
the question, source and acceptance chosen before implementation.

## Validation and replay

Build the CLI and corpus runner with:

```text
mbx build -p omega --bin omega -p compiler --test corpus_runner --target-dir build/gate-cli
```

Freeze their executables under `build/root-discovery` as `omega-baseline`,
`omega-candidate`, `corpus-baseline` and `corpus-candidate`. Run `validate.py`
with `baseline` or `candidate` from the repository root, then `compare.py`.
The scripts use the existing corpus runner and outcome comparator and additionally
compare exact diagnostics, fixture membership and execution facts.

The core baseline records are the previous experiment's **candidate** records.
Reuse is applicable because their CLI/runner identities equal this experiment's
baseline, and the measured fixture/library trees and target/engine settings
have not changed. The baseline contains the first optimization with no temporary
probes. Calling-policy/private-projection selections receive fresh baseline and
candidate captures. Historical records retain their original timing and identity;
they are not described as fresh measurements.

The final implementation builds, file-scoped formatting succeeds, and
`mbx clippy -p typed-trees -p validation --target-dir build/gate-cli` succeeds.
Clippy reports existing warnings on unchanged code (including eight in
typed-trees and 158 in validation); none points to changed lines. Python capture
and comparison scripts pass syntax compilation.

For performance, run `python3
wiki/drafts/measurements/trait_root_discovery_2026_10_22/measure.py` after this
session's builds and validation processes finish. It reuses the existing benchmark
primitives, retains warmups and every observation, alternates producers, shuffles
subject order, uses fresh CLI processes/build directories and executes every
emitted image against exit 0. Both binaries and the Rust patch are digest-bound.
The three existing fact workloads and their oracles are unchanged. CPU time and
peak RSS accompany elapsed time; external workstation activity is left alone.
For another host, use host-built producers and change the script's explicit
target. This experiment establishes no performance result on Windows or Linux.

## Result and decision

Retained. Twelve alternating pairs per subject plus six retained warmups produce
78 native compilations/executions. Every image exits 0; within each subject all
baseline and candidate images have exactly the same SHA-256. No run or outlier
was discarded. No build or corpus process from this session overlapped timing.
The one-minute load average ranged from 4.94 to 5.76. Processes start fresh with
warm filesystem state; these are not cold-start or incremental-build results.

| Workload | Baseline median (range), ms | Candidate median (range), ms | Reduction |
| --- | --- | --- | --- |
| Length | 685.775 (676.521–720.485) | 672.180 (657.423–696.076) | 1.98% |
| Dynamic selector | 682.753 (671.294–713.677) | 676.552 (658.496–724.681) | 0.91% |
| Nine-level record | 683.058 (666.838–747.987) | 675.475 (656.950–694.256) | 1.11% |

All three means also decrease: 688.326→674.468, 684.310→680.181 and
690.147→674.375 ms. Median user CPU time decreases by 9.829, 7.125 and
7.533 ms, respectively. The paired median candidate-minus-baseline differences
are −13.992, −5.896 and −11.008 ms. Individual trials overlap and some candidate
trials are slower; there is no universal or tail-latency guarantee. Median peak
RSS is lower by about 1.97, 2.19 and 3.91 MB, but ranges overlap and this single
batch does not establish a durable memory improvement. The concrete work removal,
consistent elapsed/CPU direction and unchanged outputs support retention.

[Raw paired observations](paired.json) and [summary](summary.json) retain every
trial, stage times, source/producer identities, hashes, CPU time, memory and
ranges. Timing differences against the previous experiment's absolute times are not attributed
to this patch; only alternating observations within this batch are compared.

- [Core source checks](check-comparison.json): all 1,053 fixture outcomes,
  diagnostics and facts unchanged (438 checked, 611 rejected, four incomplete).
  The same 37 baseline issues remain: 26 positive/run refusals or incomplete
  cases and eleven invalid controls missing expected diagnostic fragments.
- [Additional source checks](policy-check-comparison.json): 37 fixtures,
  with 17 checked positives and 20 rejected controls, all diagnostics/facts
  unchanged and no baseline issues.
- [Core engine comparison](both-comparison.json): 21 native/Terminal exit-0
  agreements. The same two fixtures refuse on both producers: dynamic selector
  through a parameter and applied binding field telescopes. Neither is counted
  as runtime acceptance.
- [Calling-policy/projection engine comparison](policy-both-comparison.json):
  all 17 publish, ten have native/Terminal exit-0 agreements (the nine transport
  places and private implementation projection). Seven additional cases report
  Terminal exit 0 without a recorded native execution; they are not counted as
  cross-engine agreements. Every recorded diagnostic and execution fact matches.

That is 1,090 source-check comparisons and 31 cross-engine agreements, with
baseline failures retained. The raw candidate records and fresh extension
baseline records are saved here; the core baseline remains linked from the
previous experiment. [Producer identities](identities.json) bind the exact final
Rust patch and frozen binaries. Diagnostic patches apply to the recorded baseline
and include all temporary trace code; no trace code remains in production.

This completes the bounded local throughput iteration; it does not close a
language capability or authorize remote publication. The remaining 16 queries
observe separate preparation/checking states. Further reduction should start by
identifying which source declarations actually change between those passes,
rather than assuming their equal root counts permit shared results.
