# mut_slice_establish

`let cells: &mut [i32] = &mut bag.cells[..]` — mutable element-view
establishment lends an array member's contiguous storage under an
exclusive loan; writes through the view are observed through the owning
collection.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
