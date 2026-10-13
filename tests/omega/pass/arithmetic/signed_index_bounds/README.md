# signed_index_bounds

Signed indices need explicit lower bounds: `i64` params carry
`requires 0 <= index` for the element read and
`0 <= start && start <= end && end <= items.len` for the range — on a
`&[u8]` slice and on a `&[u8; 4]` array alike.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
