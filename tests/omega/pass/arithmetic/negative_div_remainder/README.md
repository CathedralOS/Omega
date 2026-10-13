# negative_div_remainder

Signed `/` truncates toward zero and `%` keeps the dividend's sign at
value position: `(0-17)/5 = -3`, `(0-17)%5 = -2`, pair recomposes -17 —
typed entries selected per operand carrier through transition args.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
