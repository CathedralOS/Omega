# Research: repeated statement checking

2026-10-28. Continue the bounded optimization loop at local commit
7635e6e61287a9e2c67fb5a85b5debbc064a0b7d. Reused the previous
[pass ablation](../pass_ablation_2026_10_28/README.md) and
[ordered graph research](../stage_five_2026_10_22/ordered_graph/research.md).
The retained stack32 profile attributes 4,711 ms inclusive wall time to the
parallel machine-validation group. Per-machine scopes sum across workers and
must not be added to or directly subtracted from process wall time.

Screened IR BooleanNot removal: selection already emits compare-zero and
materialization, with dedicated branch inversion support. Replacing it with
an equality and a false constant offers no demonstrated code-quality benefit;
not selected merely to remove an enum variant.

New search question: can repeated pure compiler queries reuse their result
without carrying facts across a changed flow environment?
Exact query: `site.rustc-dev-guide.rust-lang.org queries memoization compiler repeated computation`.
Read the full [query evaluation model](https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html),
especially immutable keys, pure providers, context lifetime and cycles.
Applicability: motivates identifying immutable input boundaries and preserving
recursion behavior before reuse. It does not justify caching flow-sensitive
checks or skipping diagnostics. No rustc framework is adopted.

Next experiment: subdivide local statement validation into shape, nested-call,
arithmetic-range, containment and capture scopes on the unchanged stack32 source.
Timing instrumentation is diagnostic only; compare uninstrumented baseline and
candidate separately before retaining an optimization.

## Candidate decisions

The first fresh-directory comparison screened replacing `analyze_meaning` with
`expression_result_type_reference`. Four measured stack32 observations per
producer gave checking medians 6046.527 ms baseline and 6152.254 ms candidate.
No target improvement: reject the candidate and restore both source files.
The partial length observations are recorded but are not sufficient to judge
that workload. Severe unrelated memory pressure was subsequently discovered;
these observations do not establish a reliable slowdown. The earlier same-directory run reused checked results (Stage05
fell to about 2 ms) and is excluded from the checking comparison. It proves
native outcome agreement only.

The next candidate replaces `ValueComparison` in the interval-refinement scan
with the existing `EqualityQuery`. The former reconstructs `Equalities` on each
failed direct comparison. All scan candidates are endpoints of one immutable
relation slice; `Equalities::prepare` already collects every endpoint and its
children. Preparing one subject-rooted query therefore covers the whole scan.
The query remains local to that invocation: no facts survive mutation, joins
or replacement of the relation set. Exact direct comparisons and the existing
finite closure/graph algorithms remain in place; this adds no premises or
new proof rule. The graph test compares batched answers with the previous
separate-query operation, including copied compound values and a fresh empty
relation environment.

This is the compute-once/read-many question described by the prior ordered-graph
research. No new search is needed: existing local lifetime and graph contracts
are the relevant evidence. Expected benefit: remove repeated closure setup from
`place` arithmetic analysis. Retention requires preserved native images and
positive/reject behavior, plus a useful measured improvement in checking.

The initial closure comparison used fresh projects but retained the user's
source-cache location. Its profile exposed unequal accumulated graphs:
324,261,921 checked-source bytes for A versus 63,583,753 for B. Both checked
414 machines and 1,154 statements, and statement checking fell, but elapsed
time and peak memory remain confounded by those different graphs. Retain the
run as diagnostic/output evidence; do not present its elapsed difference as
the optimization's speedup. The final comparison gives each observation a
fresh process-local Windows cache base through the location already read by
SourceResolver. Ordinary package review still runs before the timed compile;
no user's cache is deleted or custody requirement bypassed.
