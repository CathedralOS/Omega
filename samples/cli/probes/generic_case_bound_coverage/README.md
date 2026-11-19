# generic_case_bound_coverage

A case `where T == i32` refutes on `Value<bool>`: `Integer` is an
impossible case on this instance, so a transition covering only `Boolean`
is already exhaustive — no `_` arm needed.

## Learnings

- When the refuted case is the FIRST case, the instance has no valid zero
  image: `Value<bool>` cannot sit in `data Main` (entry context requires a
  zero value) — keep it a local.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
