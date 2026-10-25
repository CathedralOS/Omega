# guard_chain_exact_cast

First sample proving a narrowing cast by a two-hop guard
chain: `requires 0 <= value && value <= 255` discharges
`value as u8` directly, and sequential transitions
`value >= 0` then `value <= 255` accumulate the same
proof across nested-machine hops. Also the `-> (0)`
parenthesized-expression arm result and exact i8→i32
widening.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
