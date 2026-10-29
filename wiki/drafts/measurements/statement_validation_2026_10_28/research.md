# Arithmetic work inside statement validation

2026-10-28. ITERATION-PERFORMANCE; fixed base
`364e3160ff039fb85161ccd46a9700be6321bf86`. Compiler source is unchanged
from published `2d87f18a195ec8523aeb4cdccc4ca0f793ba0e1b`.
Continue [the prior result](../spill_summary_2026_10_28/README.md).
Reuse the [checking research](../checking_simplification_2026_10_28/research.md)
and its immutable-query lifetime constraints.

Question: which repeated statement operation still dominates the fixed stack32
source-to-native workflow after equality-closure reuse?
The first diagnostic divides statement work without removing checks.
Local arithmetic range scopes total 942.416 ms, terminal-expression range
863.643 ms, versus 1906.922 ms for statement-node validation. Type/shape scopes
are only 7.147 ms combined; placed checks 0.706 ms. These are inclusive diagnostic
scope totals, not additive process time or a claimed available speedup.
The published executable hash still matches the retained baseline and exits 0.

A second diagnostic splits arithmetic place analysis, standing bounds, retained
relation refinement, operand carrier reports, mathematical intervals and
representability queries before choosing an implementation.

Research question refined from generic statement reuse to arithmetic query
lifetime. Exact search: `site.rustc-dev-guide.rust-lang.org queries memoization
arguments type checking`. The search output was lost at context compaction;
no snippet or result from it is used as evidence. Read instead the primary
[Rust query guide](https://rustc-dev-guide.rust-lang.org/query.html), query
execution and provider sections. Reuse needs an exact key and context lifetime;
result copying can itself cost. This is a hypothesis about avoiding repeated
preparation, not permission to memoize effectful checks or flow facts globally.
No framework or dependency is proposed. The earlier source on query evaluation
already covers purity/cycles; it is not researched again.

## Selected experiment

The second profile attributes 1002.844 ms inclusive to mathematical interval
analysis, versus 208.088 ms to place-relation refinement, 16.581 ms to standing
bounds, and 0.910 ms to operand carrier reports. Subtraction interval refinement
calls `subtract_floor_limited`, whose scan constructs `ValueComparison` and
rebuilds finite equality closure after each failed direct comparison.

Hypothesis: use the existing `EqualityQuery` once for each subtraction operand
through that immutable scan. This avoids repeated closure preparation without
changing relation publication, frame admission, match recursion or max-floor
selection. Two separate subject-rooted queries preserve the existing subject
roster; one combined query would add the other operand to each finite closure.
The endpoint-indexing idea is deferred: this measured larger path has an existing
helper and needs no new graph API. The earlier interval-refinement experiment
and query research directly apply; no additional literature search is needed.

Scope: `subtract_floor_limited`, reached by ordinary and match subtraction
queries; existing equality graph tests; the unchanged stack32 nine places and
reject twins plus arithmetic/mutation controls; three workload A/B comparison.
No source syntax, config, exports, library or fixture migration is needed.
Retention requires repeatable whole-compile/checking benefit, exact native image
agreement and preserved positive/reject diagnostics. Overlapping timing ranges
without a clear attributable benefit park the candidate.
