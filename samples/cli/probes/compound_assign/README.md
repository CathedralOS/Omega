# compound_assign

`+= -= *= /= %=` compound assignment desugars to `x = x OP y`; chained
writes observe the prior write (no stale entry-value fold).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
