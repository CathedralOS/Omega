# Permission-event preparation

ITERATION-PERFORMANCE continuation at fixed local source
`505424a1b12340a3001b1d28fc5ac298e5de4ec8`, submitted in PR767.
Keep the warm source and retained exact producer rather than chasing main.

The preceding [conjunction experiment](../conjunction_flow_2026_10_29/README.md)
measured 439.1 ms in permission-event preparation on the fixed stack32 witness.
Its state loop accounts for 428.5 ms: setup 65.9 ms, permission-schedule walk
362.2 ms. These are inclusive diagnostic scopes, not an additive wall budget.

First experiment: temporary nested scopes divide the walk into selected-case
preparation, evaluation setup, permission evaluation, selection recording,
completion, publication and exits. Restore production source after building.
Compare the diagnostic with retained producer `12ca439b...` on unchanged source,
Build and native oracle, requiring an identical image. A small isolated subcall
is not enough to justify a production change; identify the work and its semantic
owner before choosing an optimization. No candidate implementation yet.

Existing research covers ordered transfer and narrowed work domains in the
previous record; a new mechanism will get a focused primary-source search after
this attribution resolves what is expensive. Retain source order, exact claim
identity, mutation invalidation, conditional cleanup and failure behavior.

## First attribution

The six-run diagnostic preserved the same native image and exit. The first
instrumented observation reports selected-case preparation 161.9 ms, permission
evaluation 16.1 ms, completion 19.3 ms, publication 7.4 ms, evaluation setup
2.4 ms and exits 2.4 ms. Those nested scopes point to selected-case evidence,
not the visible linear-permission evaluator, as the next question. A second
diagnostic separates `CaseEvidence::prepare` from `inactive_owned_cases` and
splits observation collection, normalization and sorting. No production edit
has been chosen from timing alone.

Research 2026-10-29: `site.llvm.org MemorySSA walker caching invalidation clobber
queries`. Read the default walker APIs, use/def optimization and invalidation
sections of https://llvm.org/docs/MemorySSA.html. Useful constraint: a reusable
answer needs an exact access/context identity, and IR mutation requires updates.
Its deferred preparation also motivates preparing only work a consumer needs.
Omega's permission and cleanup rules are not LLVM MemorySSA; no analysis or
cache is imported. Reject cross-statement case-evidence reuse without a complete
invalidation argument. Prefer removing repeated work inside one existing
immutable evidence batch if the deeper profile supports it.

## Publication

PR767 submitted `505424a1b12340a3001b1d28fc5ac298e5de4ec8` and published
`9774b83a2f6f57359386f4984c83fef348985eb3`. PR compiler-check run 40851995158
and queue run 40852372940 succeeded; fetched-main ancestry was verified.
Post-merge smoke run 40852742565 subsequently completed successfully (job
123376014759); the native/Terminal smoke and refusal-path steps passed.
The next experiment remains on its warm local source; publication is not a
reason to rebuild or relabel these measurements as current-main observations.

## Root ownership prerequisite

The deeper profile attributes 120.7 ms to inactive owned-case queries. A third
instrumented run records 175,147 subject-type queries (95.5 ms), 74,674 variant
walks (48.0 ms), and only 5,669 root queries (1.8 ms). Scope times overlap;
different diagnostic rounds are not a speedup comparison. The type-cleanup
classifications already use frozen-program memos, so another cache is not the
first experiment.

An initial candidate tried pruning descendants of a root classified as having
no owned cleanup. It built, but code review identified an unnecessarily broad
proof obligation: selected cleanup belongs to exact subjects, while the
recursive contents classifier and declaration multiplicity answer different
questions. The patch is retained as unpromoted, not as a demonstrated bug or
performance result. Its screen did not run: the output directory already held
the earlier attribution experiment, and the harness correctly refused reuse.

The selected candidate instead reuses the existing root ownership result.
`canonical_owned_place_type_reference` requires every prefix, including the
empty root, to resolve and be non-reference storage. A failed root check is
therefore a failed prerequisite for every descendant query in that same
state/statement. Invocation, type, expression and symbol roots all preserve
this prerequisite. The attached-machine field special case explicitly observes
the logical empty prefix too. This removes repeated negative queries; it
introduces no answer cache across contexts and changes no cleanup classifier.

Screen: six interleaved A/B runs on unchanged stack32, exact native bytes and
exit, then the existing three-workload comparison if the screen supports it.
Behavior gate: nine saved-case-predicate positives with native/Terminal
agreement and nine fact-sensitive reject twins, plus observed cleanup,
replacement and loan controls. Reject on any semantic difference; retain a
performance claim only if unprofiled whole-compilation observations support it.
