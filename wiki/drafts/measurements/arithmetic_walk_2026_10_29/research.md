# Remaining arithmetic walk

ITERATION-PERFORMANCE, fixed base
`5f5f5c64e5a2d8f8fb78b2a649ccb3025779bf2f`. Continue
[endpoint indexing](../statement_remainder_2026_10_28/README.md), retaining its
optimized producer `build/statement-remainder/candidate/omega.exe`, SHA256
`139ad7c5be5564fc9633e50e1d0a41744e7770c4bea909d476cd0845d7e8a558`.

The prior candidate's profile still spends 653 ms in statement validation.
The previous detailed baseline attributes most statement cost to local and
return-expression arithmetic walks, but mathematical intervals and completed
operand proofs are now small. Do not optimize those obsolete hotspots again.
Separate call-result inference, folded values, range recipes, binary traversal
and declared type queries before selecting another change. Scopes are inclusive
and nested; no sum of them is a wall-time claim. Keep the fixed stack32 source
and exact native image oracle, with no new fact or source rewrite.

Read-only source inspection found existing memoization of builtin binary
meaning and an early operator guard on short-circuit environment construction.
Do not add another cache or duplicate those guards without measured evidence.
The [previous graph/query research](../statement_remainder_2026_10_28/research.md)
remains available; a new mechanism needs its own applicability argument.

The diagnostic build finished in 2m20s at about 00:03:54 local, but an automatic
`mbx gc --automatic` child retained the redirected command stream afterward.
It was left running. No cache configuration or external process was changed.
Another Omega worktree started a release build around 00:05:11, and a separate
project test was also active. This shared-host profile is attribution evidence;
it cannot establish a whole-compile gain against earlier timings.

## Call-result attribution and research

The completed profile preserves the exact native image and exit 0. Call-result
analysis totals 785.03 ms inclusive: declared guarantees 512.45 ms, body inference
259.98 ms, actual interval capture 0.03 ms. Folding, recipes and basic type queries
are negligible. A second diagnostic separates declaration dependency collection,
the independence walk, guaranteed intervals and normal-return recipes.

Source inspection finds `declaration_call_dependencies` reconstructing direct
dependencies from contracts, bodies, target families and retained static selections
on every visit. If that dominates, the existing `CheckingMemos` owner can retain
this exact query's result by complete symbol identity. Mutable bare trees have no
memos; projected inputs drop them; context changes reset non-retained fields.
Do not retain a new dependency query in `retain_typed_tree_memos` or
`clone_typed_tree_memos`: dependency selection may observe context.

New question versus the earlier query research: can direct declaration adjacency,
including negative results, be reused over the exact frozen selection context?
Search (2026-10-29): `site.rustc-dev-guide.rust-lang.org query memoization dependency
graph immutable results invalidation`. Read the
[query evaluation model](https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html),
purity, inputs, caching and cycles; also the query-system section of
[parallel compilation](https://rustc-dev-guide.rust-lang.org/parallel-rustc.html).
The latter labels much implementation detail outdated, so no claim about current
rustc performance or APIs follows. Applicable principles: immutable keys and
results, complete context ownership, and no lock held during computation.
The prospective change reuses Omega's existing memo owner and permits duplicate
concurrent misses; it adds no pending-result semantics, global cache or framework.

The governing safety control is the existing `independent_call_result_bound`
fact row: every target-family member remains in the dependency graph, so a
wrapper cannot justify its own guarantee through another member. Ordinary
guarantees remain partial correctness; no termination requirement is introduced.
The source-level A/B validator will exercise those positive/circular pairs and
retained-selection controls if the measured collector cost justifies the memo.

The finer profile rejects that adjacency-memo hypothesis: 4441 dependency reads
take only 7.74 ms, 991 independence walks 18.35 ms, and recipes 0.03 ms. The
33 normal-return contract projections consume 649.55 ms inclusive. No dependency
memo is implemented. A diagnostic roster now records each complete projection
input and elapsed time to distinguish repeated identical queries from costly
first computations. It includes the invocation memo identity, callable, result
type, exact parameter/contract slice identities, declared interval, scalar
actuals and descriptor-length actuals. This diagnostic does not cache or change
answers. Identity addresses are process-local attribution data only.

## Selected experiment: memoize complete guarantee projections

The roster has 33 queries and 22 unique complete keys. Two expensive 32-formal
keys occur twice each (290.65 and 285.94 ms total); other actual vectors remain
distinct. Addresses in the roster only attribute a diagnostic process.
The candidate reuses the existing immutable CheckingMemos owner, keyed by
callable, result type, declared bounds and both complete formal-keyed scalar
and descriptor-length actual vectors. Eligibility requires the exact stored
parameter and contract slices. Unrecognized/projected contracts use the existing
computation. No lock spans computation; poisoned locks fall back to computation.
The field is not retained across context reconstruction. Existing independence
and formation checks remain before this query. Projection order is unchanged.

The primary-source query principles above apply to this measured repeated query;
no dependency memo, framework, pending query or new dependency is introduced.
Validate distinct-actual positive/reject fixtures and the existing circular
call-result row, formation, mutation and contract controls. Compare current
baseline against candidate for attribution, then directly compare the cumulative
candidate with the start-of-continuation producer (67c79dda...) using the existing
three-subject, unprofiled interleaved harness. Do not multiply per-step gains.

The first candidate chain used Windows PowerShell 5 with Stop-on-error and
stopped when a native stderr deprecation warning became a PowerShell exception.
No benchmark ran. The same build under PowerShell 7 exited zero (optimized,
1.58 s incremental); its validation chain checks every native exit explicitly.
The completed source checks also cover collection-result bounds with distinct
windows, wrong actuals and descriptor rebinding, so both memo coordinates have
semantic controls. These are existing fixtures, not rewritten benchmark inputs.

## Existing diagnostic-oracle mismatch

All 70 A/B execution/checking observations agree, but final authored-oracle
checking finds `fail/arithmetic/collection_result_rebound_descriptor` stale on
the fixed baseline: it expects `no exact incoming reference origin for values`;
both optimized producers emit `cannot prove index `count` is within length 2
in rejected::entry`. No source or canonical oracle was edited. The replay wrapper
pins the existing source and oracle hashes and exact observed refusal for this
one named pair, explicitly records authored_oracle_matched=false, and keeps the
original failed validation log. Thus 68 observations satisfy the intended/authored
oracles and two establish unchanged refusal with an inherited oracle mismatch.
The resume verifies producer/source/build identities before reusing completed
observations; it does not rerun or relabel those executions.

## Cumulative result and final attribution question

The direct cumulative unprofiled comparison reports stack32 medians
7629.64 -> 6539.04 ms (14.29% lower), checking 3113.43 -> 2322.64 ms
(25.40% lower). All four adjacent A/B pairs favor the candidate, but ranges
overlap and host timing varies. Length and dispatch medians improve 3.39% and
4.38%, with identical images and successful native exits on all 36 runs.

The six-run incremental profile screen is not a standalone speed win:
A's wall median is 6523 ms, B's 6872 ms, amid a large downward drift across
observations. Do not discard or relabel those runs. Before retaining this final
memo based only on the cumulative bundle, a diagnostic adds hit/miss/bypass
logging with complete keys and elapsed times to verify that expensive duplicate
queries actually reuse answers. It changes no query result or source program.
This resolves mechanism attribution, not a new whole-compile performance claim.

## Rejected after exact hit attribution

The final diagnostic records eight misses and three hits. All six expensive
projections are misses (103-118 ms each); the three hits reuse only about 0.01 ms
queries. No expensive reuse is demonstrated. The prior process-address roster
was insufficient evidence of shared lifetime: distinct contexts can reuse an
allocator address after release. The actual memo correctly resets at those
boundaries. Do not broaden its lifetime to chase the apparent duplicate.

Remove the production memo, preserve this failed experiment, and compare the
two retained compiler changes directly against the starting compiler. This is a
changed candidate, not a repetition seeking a favorable timing. The positive
and negative actual-substitution fixtures remain useful source controls for the
retained proof optimizations; no language capability closure is claimed.

Source inspection after rejection identifies repeated condition-environment
construction inside conjunction projection as a possible future question. It
is not implemented or claimed as a bottleneck: preserving selective evaluation,
partial-operand formation, effects and branch joins needs its own attribution
and controls. The retained two-change cumulative comparison is the stopping
check for this session; do not turn the rejected address-based memo hypothesis
into a broader or longer-lived cache.
