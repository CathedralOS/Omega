# computed_measure_range

A declared measure computes `value * 2` over its `u8` parameter; the
authored range `0..=200` constrains the produced rank, not the
`[0..=100]` subject — entry proves `2 * remaining <= 200` and the
guarded self edge proves strict descent of `2 * (remaining - 1)`
against `2 * remaining`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
