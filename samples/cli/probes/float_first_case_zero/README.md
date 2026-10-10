# float_first_case_zero

A `[copy]` data type whose first case carries an `f64` payload is zero-constructible: all-zero bytes are +0.0, so `Holder { ready: true }` (omitted `sample` field) mints `Sample::Reading { value: +0.0 }` and the transition selects the first case; a later `Missing` store reselects correctly.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
