# Integrated compiler-throughput validation

The two bounded optimizations in [field inventory reuse](../compiler_throughput_2026_10_21/README.md)
and [root census reuse](../trait_root_discovery_2026_10_22/README.md) were integrated onto
main `ae4ec531488e910b06517bfd2ed8066ff3b6c14a`. The optimized producer is `541cbaa31d3f15fa595c758b8157429fd2072bd8`; the following documentation
commit changes no producer inputs. Historical measurements retain their actual producer
identities and are not relabelled as final-candidate runs.

## Incoming changes and scope

A fresh broad comparison on main `2e9dd5369249a0b19bf4881e062cb1b6d86c216a`
and candidate `fb2e2146e5158aae58cc1cf29dae8e06e889b74f` checks 1,103 source
fixtures: 458 checked, 641 rejected, four incomplete. Fixture membership,
statuses, diagnostics and facts are identical. The same 39 baseline issues remain.
The engine selection contains 44 cases, with 36 native/Terminal agreements,
seven additional built cases without a recorded native agreement, and the existing
`applied_binding_field_telescopes_exit` source rejection. All engine records match.
Raw records and identities are retained under `preceding-comparison/`.

Incoming main then changed declared-domain exclusive-borrow carrier matching and
nested mutable-call result origin propagation, plus crash-entry and nested-helper
fact rows. Neither implementation touches the five optimized files. Their consumer
paths can participate in checking, so they receive fresh baseline/candidate checks
rather than a claim that the earlier binary tested the later compiler. The focused
selection includes both new fact rows and reject twins, domain-bearing borrows,
retained/private application selections, generic calling policies, source field
contracts and the original engine consumers. The complete selection is in the
filter files; no source, oracle, expected rejection or golden row was edited.

## Observations on the first integrated revision

The final source comparison covers 129 cases and the engine
comparison covers 68 cases. Both comparisons have zero gains,
losses, diagnostic changes or execution-fact changes. There are 59
recorded native/Terminal agreements. The source selection retains
2 baseline issues; the engine selection retains
2. See the comparison JSON and raw records for their
exact identities; these are preserved failures, not clean-suite claims.

Both producers were freshly built with `mbx build -p omega --bin omega -p compiler
--test corpus_runner --target-dir build/gate-cli`. Their frozen CLI and runner
hashes and verified optimization profiles are recorded in the producer receipts.
Pinned qualification captures the focused comparisons and timing experiment on one
clean candidate with full compiler, bundled source, fixture, harness and toolchain
input identities. The local immutable qualification state is digest-bound in
`qualification.json`. Ordinary exact-SHA landing verification remains separate.

Edited-file Rust 2024 formatting, `git diff --check`, and scoped Clippy pass.
Package-wide formatting reports pre-existing blank lines in untouched
`checking/src/frozen_program.rs`; no unrelated formatting was changed. Clippy's
existing warnings are retained. The initial file-format invocation used the wrong
Rust edition, refused, and was rerun successfully with the workspace's 2024 edition.

## Paired sample-to-native measurements

Twelve alternating pairs per subject plus six retained warmups produce 78 builds
and native executions. Every image exits zero, and every image for each subject
has identical bytes across producers. No own build or corpus process overlaps
this timing capture. External workstation activity is left untouched and load
averages are retained. CPU, memory, full ranges and every trial remain in
`paired.json`; this is warm-filesystem, fresh-process behavior on macOS ARM64,
not a Windows/Linux or cold-start claim.

| Workload | Main median, ms | Optimized median, ms | Reduction |
| --- | --- | --- | --- |
| length | 704.282 | 687.596 | 2.37% |
| dyn_dispatch_selector_payload_bound | 691.648 | 681.020 | 1.54% |
| nested_nine_level_record_field_bound | 706.219 | 692.295 | 1.97% |

The combined comparison measures both optimizations together against the integrated
main baseline. Individual trial ranges overlap; the measurements do not promise
that every invocation or tail latency improves. The implementation's scope remains
query-local immutable reuse and removal of unused queries, without cross-mutation
caching or changes to proof, ownership, target selection or emitted semantics.

## Latest main integration

Main advanced to `f39545ad5c8b0a6ebc7373df9438825fac416397` with saved array-element scalar capture after the
preceding qualification. The complete incoming Rust delta adds a fallback from
unfolded checked scalars to an existing live scalar fact, before the existing
selected-live capture. This can affect source proofs involving indexed copies
and later writes, so the saved-observation nine-place row and its overlapping-write
reject twins receive fresh baseline/candidate checking and native/Terminal runs.
The selection also retains length, dynamic selector payload, applied source-field
and private implementation consumers. Earlier observations keep their actual
producer revision; they are not asserted to have executed this newer compiler.

Both main and optimized compiler `0134b05a68f566144545a6cf90a7bfce73b267ca` were freshly built and frozen.
The scoped source comparison covers 66 fixtures; the engine
comparison covers 36, with 36 recorded native/Terminal
agreements. Membership, statuses, diagnostics and execution facts match exactly.
There are 0 existing source-selection issues and
0 engine-selection issues, explicitly named in
`latest-main/*-comparison.json`. The original qualification history is retained
and amended to capture these new producers and checks, not overwritten or
relabelled as reused execution. The final documentation edit changes no compiler,
consumer, oracle, toolchain or harness input; the subsequent assessment and
mandatory landing verification bind it to the publication candidate.

The paired experiment is repeated against this actual main producer after the
checks finish: 78 successful native executions and identical image bytes within
each subject. All warmups and 12 alternating pairs are retained in
`latest-main/paired.json`, including CPU, memory, load and timing ranges.

| Workload | Latest main median, ms | Optimized median, ms | Reduction |
| --- | --- | --- | --- |
| length | 696.796 | 687.815 | 1.29% |
| dyn_dispatch_selector_payload_bound | 709.396 | 690.989 | 2.59% |
| nested_nine_level_record_field_bound | 697.952 | 683.185 | 2.12% |
