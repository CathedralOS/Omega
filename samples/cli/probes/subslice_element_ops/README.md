# subslice_element_ops

First sample with nested `&mut` subslice narrowing:
`exits[1..3]` keeps its own loan/coordinates, a second
`tail[1..2]` narrows further, and whole-element replacement
`tail[1] = blank_exit(43)` lands through the view. Requires
roster on the slice param (`exits.len >= 4`); whole-array
views spell `&mut exits[..]`.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
