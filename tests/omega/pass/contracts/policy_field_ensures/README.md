# policy_field_ensures

A guarantee over a policy field keeps the field's own arithmetic:
`countdown.remaining - 1` subtracts modulo 2^32 (`u32 in Wrapping`)
or clamps (`u32 in Saturating`) in the contract exactly as in the
body — the exit proof of `remaining - 1 < 8` needs `1 <= remaining`
supplied by the `start + 1` store under `requires start < 8`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
