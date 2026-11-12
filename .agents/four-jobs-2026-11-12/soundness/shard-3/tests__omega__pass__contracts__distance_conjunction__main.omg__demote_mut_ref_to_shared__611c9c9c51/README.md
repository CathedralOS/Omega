# distance_conjunction

A bounded distance proved through a conjunction guard: either
conjunct of `index < limit && index < 4096` that names the bound
keeps the cursor below it, for a scalar bound and for a collection
length (`index < bytes.len`) alike.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
