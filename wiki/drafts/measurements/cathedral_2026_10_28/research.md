# Shared compilation investigation, 2026-10-28

Question: can Cathedral's UI layout and frame products reuse checked dependency
work instead of reconstructing it for each product?

Local evidence: both Build roots depend on `../../../platform/native`, with
different provider selections and entry machines. Recorded process wall times
are 14.016 and 12.889 seconds. Shared source does not establish shared semantic
inputs or the amount of duplicated work. Existing nested timers do not account
for the whole critical path, and checking is not a proof-only timer.

Primary source read: [Rust incremental compilation in detail](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html),
direct URL open, no search-engine query. Read dependency tracking, red-green
invalidation, stable identities, and persistence costs. Reuse requires tracked
inputs; unchanged outputs can stop transitive recomputation. Fingerprinting
and serialization themselves cost time, so a persistent fine-grained cache is
not automatically a win.

Proposed experiment: first count repeated work and account for process wall
time for these two products, then prototype invocation-local reuse of the
largest repeated immutable result through the existing compiler drivers.
Preserve all required target-body checking and product-specific Build meaning.
Compare both artifacts and runtime oracles; alter a dependency contract to
verify stale results cannot survive. Reject this candidate if duplicate work
is small or the end-to-end saving is lost to retention overhead.

At that initial checkpoint there was no prototype or measured reuse benefit. Persistence and a general query
engine are deferred until measured reuse justifies their costs. Cold full
composition and incremental edit latency remain separate acceptance metrics.

The completed experiment and selected implementation are recorded in
[shared compilation](shared/README.md). One existing Build requesting both exports
removes more repeated work than retaining a compiler across two root builds.


## Native UI critical path, 2026-10-28

Question: after sharing layout/frame compilation, which repeated analysis or
persistence costs still dominate the complete invocation? The fresh optimized
producer measures roughly eight seconds with retained inputs. Phase scopes show
native production and Terminal lowering, rather than warm checking, dominate.
See [the next measured round](native-round/README.md).

Exact search queries:

- `site.llvm.org docs DataFlowAnalysisIntro worklist reverse postorder fixed point`
- `site.llvm.org Greedy register allocator live interval splitting compile time`

Read [Clang dataflow analysis introduction](https://clang.llvm.org/docs/DataFlowAnalysisIntro.html),
the worklist/convergence discussion: monotone transfer on a finite-height lattice
converges, and a worklist revisits nodes whose inputs changed. Applicability:
Omega payload membership retention and register allocation already use fixed
points; measure which structures they reconstruct before adding another cache.
This supports preserving the fixed point while removing unaffected work, not
omitting proof obligations.

Read [LLVM RegAllocGreedy.cpp](https://llvm.org/doxygen/RegAllocGreedy_8cpp_source.html),
LLVM 24.0.0git, specifically `growRegion`: it tracks recently added bundles and
uses a compile-complexity budget with a failed-attempt result. Applicability:
Omega has repeated spill/reanalysis rounds, but already retains per-function
facts and reanalyzes changed functions. LLVM's budget is not permission to skip
Omega admission or claim a faster allocator without testing emitted programs.
No LLVM implementation was copied. A register strategy change is deferred until
its benefit and native-code cost can be measured separately.

The first diagnostic disabled only source-graph retention in the owned workload
copy, preserving native-component cache hits. Its two unretained runs took
10.715 and 10.784 seconds versus retained 7.780 and 8.084 seconds, with identical
native artifacts. Reject deleting the source cache. Narrow timers also ruled
out descriptor residence scheduling (0.034 ms) and deprioritized repeated
compiler-library identity (89.843 ms); payload membership propagation remained
637.352 ms in that observation. All nested timings overlap their parent scopes.

A Windows samply/xperf recording failed while processing its ETL with 'The handle
is invalid'; it supplies no sampling attribution. Continue with existing phase
scopes rather than treating the failed capture as evidence.


The narrower experiment selected query-root graph construction. Retention adapts
only the complete transitive invocation closure of its candidate query machines,
including all dynamic alternatives, through the existing adapter. Four interleaved
warm observations per compiler reduced mean wall time from 8.561 to 8.174 seconds;
the graph phase fell from 666.872 to 397.519 ms. All six emitted artifacts were
byte-identical in every run. Concurrent host builds make the wall-time estimate
noisy; no subsecond or cold-build result is claimed. See the linked round for
focused native/invalid controls and the unchanged baseline interpreter defect.
