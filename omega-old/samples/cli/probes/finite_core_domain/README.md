# finite_core_domain

First sample on `Finite` — a compiler-known core float
domain from `omega::language::core::float_operations`:
`f64 in Finite` fields and `f32 in Finite` params carry
it, and the data `where`/`in Finite` windows tolerate
transient violations (NaN from `0.0f64 / 0.0`,
out-of-range) repaired before the next transition.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
