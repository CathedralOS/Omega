# shadow_const_where

First sample where a `where` fact's field names shadow a
module constant: `Holder`'s common fact and `Slot::Found`'s
payload fact both name `value` beside `const value: u64 =
99` — inside facts the field wins (else `99 <= 5` is
unsatisfiable); inside the state body the bare `value`
still selects the constant.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
