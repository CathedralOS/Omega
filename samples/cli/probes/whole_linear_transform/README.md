# whole_linear_transform

First sample taking a whole linear owner out through `&mut`
(`let removed = inventory`), passing it through a conditional
drop-and-rebuild transform, and storing it back — the place is
empty for exactly the transform's duration.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
