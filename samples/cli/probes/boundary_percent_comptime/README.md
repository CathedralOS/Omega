# boundary_percent_comptime

First sample with a boundary-declared operator served by a
provider during const evaluation: `boundary machine %
Math::remainder` + `satisfies` impl computing `left | right`
— a callable precondition `requires value <= bounded(256)` proves the
provider body ran at check time (builtin `%` would fold 1,
the provider yields 7).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
