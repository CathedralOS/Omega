# element_view_write

`slots[index] = value` through a `&mut [i32]` param — the element-view
write op: the view's length witness feeds the bounds proof under
`requires index < slots.len`.

## Learnings

- `2 < main.items[..].len` is NOT statically discharged at the call site
  — a literal index against an inline slice still carries a dynamic len;
  the requires survives as an actual contract (the corpus checks the
  signature without a call).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
