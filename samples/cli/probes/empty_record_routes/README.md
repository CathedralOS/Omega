# empty_record_routes

First sample covering zero-storage custody: `Empty {}` values
forward to a nested machine, die unused, and replace a field of
a mutable-borrowed record beside a scalar sibling — none owes
bytes, a move window, or cleanup code.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
