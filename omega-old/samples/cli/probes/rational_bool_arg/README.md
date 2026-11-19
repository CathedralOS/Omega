# rational_bool_arg

Rational expressions as bool const args: `Flag<(1/3 < 1/2)>` ≡ `Flag<true>`; `(0.1 + 0.2 == 0.3)` is TRUE under exact comptime rationals — no binary rounding trap at the type level.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
