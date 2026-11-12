# subject_field_where_index

A data `where` fact bounds a field read through the machine's explicit
subject in a SUCCESSOR state, under a computed index: `main.k + 1`
discharges `arr`'s bound by interval arithmetic over `k`'s standing
facts. The literal store's exact value does not survive the transition —
only the where fact does — so the successor arm proves the index from
`0 <= k, k <= 3` alone. Array elements live in `i32 in Wrapping`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
