# inclusive_subslice_bounds

Inclusive range literals slice a view: `view[0..=2]` is a 3-element
prefix and `view[1..=3]` a 3-element middle — the `..=` upper bound is
part of the slice language, not only of `u64 [a..=b]` types.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
