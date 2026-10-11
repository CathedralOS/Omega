# copy_view_descriptor

`[copy]` record `{label: &[u8] in Utf8, rank: u64}` crosses state edges by value — the view field travels as its two-word descriptor beside the scalar; each state reads its own copy.

Compile-checked only: run a check target, not native.
