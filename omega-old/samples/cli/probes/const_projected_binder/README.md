# const_projected_binder

First sample projecting a const record field into a
const-generic binder: `const PROJECTED: u64 =
SOURCE.value`, `Indexed<PROJECTED>` as the param type,
`-> Indexed<7>` result — the projection and the literal
arrive at the same binder.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
