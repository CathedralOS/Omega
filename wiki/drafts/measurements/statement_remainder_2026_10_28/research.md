# Remaining statement arithmetic

ITERATION-PERFORMANCE, fixed base `cb77f31d5ef` (full identity in run records).
Continue the [difference-constraint result](../contract_preparation_2026_10_28/README.md).
Retained optimized baseline: `build/contract-loop/candidate/omega.exe`, SHA256
`e2254027ace19070bf27f9209fa8642f39812ed7ac8a473d1ae63b0f5683c500`.
The new numerical test file is under cfg(test); it does not change this producer.

The preceding candidate profile still spends 636.606 ms in statement validation,
194.709 ms in value-position calls and 1551.875 ms in allocation. Statement
arithmetic is the next bounded question because its earlier equality-closure
repair has an attributable benefit and leaves a directly inspectable hot path.
Allocator batching and slot-storage experiments already have retained/rejected
records; do not repeat them unchanged.

Diagnostic: split mathematical intervals, dependent arithmetic refinements,
subtraction-floor scans and place refinement on this exact new baseline. These
scopes nest; they are attribution evidence, not additive wall time.

Read the [previous arithmetic research](../statement_validation_2026_10_28/research.md)
and [ordered graph research](../stage_five_2026_10_22/ordered_graph/research.md).
The former explicitly deferred endpoint indexing in favor of the smaller closure
reuse change. The graph's symbolic lookup remains linear per pair even though
its immutable vertex roster and cached numeric rows already exist. If closure
seeding dominates, resolving each endpoint once before that pair loop is a
specific next hypothesis. Preserve exact structural identity, pair order, checked
gaps, relation order, finite round/comparison budgets and query-local lifetime.
The existing compute-once/read-many findings apply directly; no new graph
algorithm, interning scheme, framework or dependency is proposed. Measure before
selecting this change, and use the existing symbolic-walk oracle and source
controls if selected.

## Selection

The first profile measures subtraction at only 15.87 ms and mathematical
intervals at 17.25 ms; that earlier target is now small. Place refinement is
225.45 ms. A second, fuller statement profile measures equality seeding at
186.58 ms, against 5.81 ms collecting terms and 10.73 ms propagating closure.
The setup is 851 immutable queries, so resolve the same endpoint roster once
before its quadratic pair scan instead of searching symbolic operands on every
pair. Estimated opportunity is a few percent of whole compilation, not the
entire statement-validation timer. Preserve every pair and budget debit.

Scope: `Equalities::prepare`, the private `DifferenceGraph` lookup and indexed
query, and the existing numerical graph oracle. Extend the oracle to compare
both symbolic and indexed mutual queries against the former independent walk,
including positive cycles and checked-overflow cases. Source controls retain
generation identity, constructor equality, mutation, subtraction and finite
limits. No fixtures, syntax, public exports or configuration change.

Decisive comparison: the existing three-subject unprofiled ABBAABBA comparison,
with the previous candidate as A, five focused graph tests, and the previous
statement validator's 68 A/B behavior observations. Preserve all valid timings;
retain only attributable benefit with unchanged behavior. If this small target
does not establish useful benefit, revert production and retain the record.
