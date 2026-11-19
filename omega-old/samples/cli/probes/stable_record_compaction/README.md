# Stable record compaction

Filter complete event records in place through a mutable borrowed slice. Keep
selected records in their original order, preserve wide integer fields and
leave sentinel records outside the borrowed window untouched. A second pass
filters the retained prefix using the first result as its range endpoint; an
empty window performs no access. The same generic kernel filters scalar scores
with another predicate, including all-retained and none-retained cases.

Run from the repository root:

```sh
omega run --both samples/cli/collections/stable_record_compaction/main.omg
```

Expected exit: 0. Each field and count is checked; a mismatch traps.

The count contract bounds each result by the supplied window's length. Exact
caller arithmetic, range checking and loop-exit proof all consume that same
relationship; no fixed-size caller assumption substitutes for the contract.
