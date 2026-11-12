# match_guard_slice_index

A `match` bool guard discharges a slice index:
`count > 0 && count <= values.len && 0 < values.len` admits `values[0]`
in the true arm; `values.len == 0` gates the complement arm of
`last_or_zero` — both directions of an empty-slice split.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
