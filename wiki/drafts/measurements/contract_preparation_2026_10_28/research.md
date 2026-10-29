# Contract proof preparation

Fixed base `6c09718ef1eb2ce913ea4b4385a5df5e574b2cda`. Continue the
[inconclusive empty-list experiment](../call_scanning_2026_10_28/README.md).
The compiler is unchanged from PR 750; retain its verified optimized producer
`build/statement-loop/candidate/omega.exe` as baseline A. The warm build directory
contains the previous rejected experiment and is not a baseline producer.

The measured refutation helper consumed 484 ms. The next diagnostic separates
argument terms, judge construction, authored spellings and actual judgments.
It also distinguishes identity admission from structural term construction in
the shared argument mapper. No semantics are bypassed. Inclusive scopes may
nest and are not summed as disjoint wall time. Diagnostic patch is zero-context;
replay against the fixed base with `git apply --unidiff-zero`.

Treat a repeatable 15% whole-compile improvement on the representative, with
focused behavioral controls and no demonstrated material regression on the two
smaller comparison workloads, as a substantial win for this run. This is a
working stopping criterion, not a claim about every compiler workload.

## Research: scope of repeated proof preparation

2026-10-28. The empty-requirement shortcut stalled; the new question is whether
repeated preparation can be shared within the same caller and immutable input,
rather than skipped. Search: `site.rustc-dev-guide.rust-lang.org query caching
immutable context memoization provider arguments`. Read the primary
[query evaluation model](https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html),
query/purity, memoization and immutable-input sections. This revisits the prior
query research for a changed key/lifetime question: whole judge preparation,
not finite arithmetic closure. Same inputs and a pure provider are necessary;
no flow facts or call results can be shared just because source spelling matches.

Also read the standard library
[LazyCell documentation](https://doc.rust-lang.org/std/cell/struct.LazyCell.html),
construction, force, and poisoning. It can defer one lexical scope's setup,
but it is not Sync and recursive initialization is not a valid provider pattern.
These sources suggest an implementation option only. No cache is selected until
the diagnostic shows that setup, rather than actual judgments, dominates.


## Selected mechanism: complete difference-constraint classification

Preparation profile: argument terms 10.26 ms, judge setup 17.72 ms, judgments
471.91 ms. Do not cache setup. The finer query profile attributes 311.52 ms to
`bounds_are_inconsistent`, within 343.68 ms of integer satisfiability queries.
The latter installs and closes difference bounds before invoking the general
pairwise polynomial contradiction search.

New query: `difference bound matrices consistency negative cycle shortest paths
Miné paper closed DBM`. Read Antoine Mine, [A New Numerical Abstract Domain
Based on Difference-Bound Matrices](https://arxiv.org/pdf/cs/0703073), section 3,
Theorems 1 and 2. Difference constraints are consistent exactly when their graph
has no contradictory cycle. Omega uses lower bounds, so its contradictory cycle
has positive rather than negative total weight. The same graph theorem applies
after negating weights. This is not an octagon, nonlinear, or general integer
feasibility solver.

Candidate: after the existing installation, opaque interval refresh, contradiction
flag and equality-residue checks, return consistent if there are no opaque terms
and every remaining bound is the exact unary or difference shape already seeded
and closed by `refresh_difference_bounds`. Constant false bounds are already
reported by installation. Higher coefficients, sums, products and opaque intervals
retain the original search. No graph closure, operand formation, substitution,
refutation policy or arithmetic law changes. Expected benefit: eliminate redundant
pairwise searches on these complete difference systems, not optimize judge setup.

Scope: one arithmetic helper, its existing callers, and a pure numeric test
comparing 576 small difference systems with exhaustive integer assignments.
Source-level validation uses existing call, arithmetic and stale-fact controls.
The first screen is the existing six-run profile comparison; retain only with
attributable improvement and separate behavioral checks. Whole-compile benefit
must be measured unprofiled before declaring the requested substantial win.
