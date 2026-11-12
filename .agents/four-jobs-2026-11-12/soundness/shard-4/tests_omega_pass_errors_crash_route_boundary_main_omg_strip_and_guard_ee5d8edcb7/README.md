# crash_route_boundary

`boundary machine ==` declares an operator with an operand-guarded
crash route `crashes Trap !(right >= 0)`: `requires` discharges it,
propagating callees re-cover with `crashes Trap`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
