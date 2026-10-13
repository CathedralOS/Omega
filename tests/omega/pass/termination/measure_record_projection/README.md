# measure_record_projection

A `measure` over a whole record projects its member as the rank:
`Countdown::Remaining(countdown)` reads `countdown.remaining`, and
`terminates by countdown -> Countdown::Remaining` ranks the machine on
the projected field's descent.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
