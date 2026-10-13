# bounded_distance_rank

The argumented bounded-distance ranking selected explicitly: a
climbing-index loop proves termination with
`terminates by (index, limit) -> Nat::BoundedDistance`. The arrow's
left side is uniformly the ranked subjects, bound in order to the
named view's (lower, upper) parameters — the use site reads as a
ranking selection, not ad-hoc proof arithmetic.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
