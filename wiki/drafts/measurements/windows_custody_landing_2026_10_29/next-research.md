# After custody: attribute retained-owner materialization and retirement

2026-10-29. Primary source inspected at merged worktree
`f2f856215a1893cd46d20e419f8ec03caec57484`, incorporating fixed main
`ec727c80cdfd469e03c6ca9e8f8a48cab29e730b`. Read the parallelism ledger,
invocation-setup attribution and custody-composition record before the targeted
external search below. This is research, not independent approval of the PR.

**Next choice: one warm restore/retirement attribution diagnostic, not a lazy
decoder implementation yet.** There is plausible small-program headroom in the
combined retained-owner lifecycle, but no measured current-main unused fraction
or teardown cost establishes another 20% opportunity.

## Measured denominator versus current hypotheses

The composed producer's retained diagnostic profiles (source `3cd599`, not the
new merged source) give these disjoint read/digest/decode observations:

| Subject/cohort | Process ms | Read ms | Digest ms | Decode ms | Combined / process |
| --- | ---: | ---: | ---: | ---: | ---: |
| length/screen | 485.7 | 13.0 | 16.6 | 67.1 | 19.9% |
| dispatch/screen | 494.8 | 20.2 | 17.9 | 71.0 | 22.0% |
| length/confirmation | 378.4 | 12.0 | 16.6 | 62.7 | 24.2% |
| dispatch/confirmation | 391.4 | 13.4 | 17.8 | 64.7 | 24.5% |
| stack32/confirmation | 974.1 | 20.5 | 17.3 | 77.1 | 11.8% |

All had a loaded graph/prepared hit. These are individual diagnostic profiles,
not the uninstrumented cohort medians. Sources are the corresponding
`build/windows-custody-composition/capture/{comparison,confirmation}/profile-candidate-*.compile.json`
and `logs/profile-candidate-*.log`. Decode alone is only about 14–17% of the
small processes. Even perfect removal of read/digest/decode cannot yield 20%
on stack32; reaching 20% on the small cases would require removing nearly all
this combined cost, or finding additional related work. Do not add these
fractions to the published custody improvement or treat them as merged-source
measurements.

The earlier invocation-setup record rules out a worker-lifetime rewrite from
its measured 0.9–1.9 ms spawn/join overhead. Its approximately 103.5 ms warm
version-process startup also prevents identifying process-minus-CLI elapsed
as destructor time. No new startup/destructor cost is asserted here.

## What the current graph actually restores

`driver/src/checked/compilation_session/source_graph.rs` loads a root-scoped
record, verifies the complete payload digest, then uses one `RecordReader` to
decode `SourceGraph { preparation, sources, checks }`. `restore_source_graph`
installs the whole owner before any fresh coordinates are allocated. Root
scoping already avoids unrelated projects; repeating that optimization is not
new work.

`SourcePreparation` owns a source catalogue and `SourceFrontends`;
`PreparedCheckedSource` owns the parse checkpoint and a roster of typed
frontiers. `SourceFrontends` retains completed declaration/pending-body inputs;
their `AuthoredCompletions` retain completed typing. `RetainedCompilationChecks`
holds Build/Product checks and sibling rosters, including each exact typed
preparation input and checked result. Thus the record can contain multiple
historical candidates and parsed fallback state beyond the particular warm hit.
That **does not prove their payload is unused**: Arcs share pages across these
owners, and catalogue prefixes, lookup contributors, declaration syntax and
pending bodies preserve exact cross-owner correspondence.

Actual selection is later: `CompilationSession::prepare_source_with_inputs`
matches root/inputs/capture and fresh source identity; `PreparedCheckedSource::into_frontier`
matches complete assembly inputs and rejoins authority; source activation checks
new lookup contributors/imports/aliases; checking compares complete preparation
input/admission before reuse. `SourceContinuation::rejoin_current_authority`
also rejoins stored authored completions, not only the one later selected by
`AuthoredCompletions::find`. Count this work before proposing demand-driven
rejoin. `SeededTypingBase` already shares resolved/typed storage through Arcs;
a derived clone is not evidence of a full-tree copy.

The Build prelude is already conditional: `prepare_build_prelude` injects it
only for an authored Build machine without its own Build data declaration.
`build_bodies` selects toolchain and Build-scope pending bodies. Removing
unreferenced prelude declarations or checking only the final native entry could
change lookup, formation diagnostics or Build demands. No measured bulk of
unnecessary prelude restoration has been established.

## One smallest falsifiable diagnostic

Use one frozen optimized merged-source producer and one seeded warm Windows
length/dispatch/stack32 capture, preserving exact images/native oracles. Add
diagnostic-only scopes for the three `SourceGraph` fields **within the same
reader**, reader finalization, retained-frontier/authority rejoin, and complete
session retirement. Record consumed bytes/allocation-budget charges and owner
roster counts per field, plus candidates examined/selected for prepared sources,
typed frontiers, authored completions and Build/Product checks. These charges
are codec budgets, not measured heap/RSS. Count unique shared owners/definitions
rather than summing Arc aliases; first-definition byte placement is not exclusive
ownership or a reachable-size proof. Do not recursively traverse every node
merely to collect a counter or log each decode event.

For retirement, `application/build/src/project_compilation.rs::compile_returned`
owns `BuildSession` through package completion and then drops it on return.
Time an explicit drop at that same boundary; it includes compiler owners and
package disposal. `CompilationSession::drop` may first store a changed graph,
then Rust drops its preparation/source/check fields. Distinguish store from
destruction and require warm unchanged-store counts. Separately time final
report destruction after CLI output at its existing lifetime boundary. The
current `print_elapsed` renders profiling **before** final report destruction;
late scopes need one later diagnostic snapshot. Do not skip drops, leak graphs,
move work past process exit or weaken resource closure to improve the timer.

If nearly all restored unique owners feed the chosen frontier/check or retire
cheaply, stop the selective-restoration direction. If one large owner family is
restored/rejoined and retired without consumption, test producer-side retention
of the exact demanded **transitive owner closure** next. Preserve stable source
coordinates and semantic arena generations; do not compact handles or construct
independently decoded halves. Fresh source/library/custody admission, Build
execution/charges, declaration authority and failure fallbacks remain unchanged.
Only proceed if measured removable lifecycle cost can plausibly exceed roughly
20% of that same complete process; otherwise return to a freshly dominant scope.

## Selective/parallel restore: applicable precedent and hard boundary

`compiler-storage/src/buffer.rs` has a single ordered shared-object table:
definitions must precede references, with exact type and one reservation each.
Its generation map gives all appearances of a source-text page one fresh tag;
syntax text storage checks conflicting payloads and remaps handles against that
same map. Semantic arena generations/source coordinates otherwise stay intact.
Arbitrary parallel field readers or byte-slice skipping would break these
contracts. Keeping global shared tables behind a lock is not evidence of useful
parallelism, and deferred malformed sections must not silently become admitted
results. A genuinely independent chunk needs an explicit indexed owner/dependency
boundary and complete bounded validation; no such format is proposed yet.

New exact query: `site:clang.llvm.org docs PCHInternals lazy deserialization declarations types identifiers`.
Read [Clang PCH internals](https://clang.llvm.org/docs/PCHInternals.html), especially
ID/offset tables, dependency loading, on-demand declarations and required eager
declarations. In actual [LLVM 23.1.1 ASTReaderDecl.cpp](https://github.com/llvm/llvm-project/blob/llvmorg-23.1.1/clang/lib/Serialization/ASTReaderDecl.cpp#L3982),
`ReadDeclRecord` resolves the declaration ID to its owning module/offset and
seeks that cursor before reading. This supports indexed, dependency-aware
materialization as a design precedent; it supplies neither Omega's ownership
proof nor an expected speedup.

Rejected next steps: unchanged buffer preallocation/primitive-vector decoding,
generation lookup-container swaps, spill-slot/cap32 changes, parser-worker
tuning, PGO/LTO, or overlapping only read/hash for a few milliseconds. Those
either repeat prior experiments or lack 20% whole-run headroom. No builds,
benchmarks, Rust/config edits, Git mutation or messages to other chats occurred.
This single ignored note is intended for the parent's later wiki copy.
