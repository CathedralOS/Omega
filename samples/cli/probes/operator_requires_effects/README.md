# operator_requires_effects

First sample discharging a boundary operator's operand
`requires` through an effect-bearing call:
`boundary machine == Integer::equal ... requires
right >= 0`, `prepare(&mut value) ensures value >= 0` —
`prepare(&mut value) == value` proves the right operand
after the effect lands.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
