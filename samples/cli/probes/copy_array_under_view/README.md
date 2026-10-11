# copy_array_under_view

By-value pass of a copy-eligible `[u32; 2]` copies under a live shared view — Unrestricted multiplicity from scalar element type; `consume(main.items)` leaves `main.items` established (affine arrays reject).

Compile-checked only: run a check target, not native.
