# copyable_view_survives_pass

Copy-eligible fixed arrays pass by value without moving:
`consume(main.items)` on `[u32; 2]` (Unrestricted
multiplicity from scalar elements) copies — the live
`&main.items[..]` view reads `view[0]` afterward.
Affine arrays would reject (see
fail/borrow/affine_array_move_under_live_view).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
