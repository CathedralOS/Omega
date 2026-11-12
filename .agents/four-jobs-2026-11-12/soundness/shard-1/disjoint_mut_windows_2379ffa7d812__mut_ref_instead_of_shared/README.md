# disjoint_mut_windows

Provably-disjoint mutable windows of the same base: `items[0..2]` ends
where `items[2..4]` begins, so the literal window bounds prove the two
`&mut [i32]` views never alias and both borrows may live at once.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
