# measure_field_cycle

A mutual call cycle ranked by a declared custom struct-field view:
`Countdown::Remaining` reads the exact `remaining` coordinate of each
member's record subject. The forward edge preserves the record; the
rebuilt literal strictly decreases the coordinate.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
