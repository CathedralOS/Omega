# claim_lineage_order

First sample proving repeated invocations of one producer are
distinct live claims: `combine` issues `right` then `left`
(establishment order is the reverse of field order) and
`capture` forwards the `Pair` whole — both invocation
lineages survive rather than collapsing to the shared
declaration.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
