# anonymous_integer_arith

Anonymous literals take their type from the result carrier — `3+4` in a `-> u8` machine is u8 arithmetic; `(255+1)-1`/`(0-1)+2`/`u64max+1-1` fold inside `ensures` without `as`.

Compile-checked only: run a check target, not native.
