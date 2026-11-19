# bare_float_literal_compare

First sample comparing computed float results against
BARE float literals (`8.0`, no suffix) in both operand
directions — the literal lands at the format of the typed
operand across `==`, `<`, `<=`, `>=`, `!=`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
