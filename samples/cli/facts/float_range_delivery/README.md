# float_range_delivery

First sample on bracketed float ranges
`f64 [0.0..=0.5]` as parameter and local types: a
narrower declared range (`[0.0..=0.25]`) delivers into a
wider one, and literals prove the bracket directly.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
