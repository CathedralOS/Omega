# increasing_to_range

The cursor itself increases: the `Nat::IncreasingTo(limit)` view
owns the bounded remaining-distance normalization, and the
optional `in 0..=limit` range constrains the produced rank.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
