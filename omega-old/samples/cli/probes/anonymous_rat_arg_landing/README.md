# anonymous_rat_arg_landing

First sample pinning single-rounding argument landing: an
anonymous exact-Rat tree `1.0 + 0.0000000596046483281043038005`
rounds ONCE at the `got: f32` transition parameter (up to
1 + 2^-23), unlike a per-op f32 tree.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Learnings

- `(anonymous expr) as f32` IS accepted when the value is
  exactly representable in the target format — refines the
  per_op_f32_rounding gap: only non-exact-representable
  computed casts need the named `F32::from_f64` route.
