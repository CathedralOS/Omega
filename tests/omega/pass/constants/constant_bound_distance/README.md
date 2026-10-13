# constant_bound_distance

A bounded distance whose bound is a literal: `(index, 8) -> Nat::BoundedDistance` and `Nat::IncreasingTo(16)` rank the cursor's distance below a constant — the bound has no parameter to thread through the cycle.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
