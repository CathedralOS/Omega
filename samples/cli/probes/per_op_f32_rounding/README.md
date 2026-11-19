# per_op_f32_rounding

First sample pinning per-operation binary32 rounding: at
2^24 each `+ 1.0f32` rounds back, so
`16777216.0f32 + 1.0f32 + 1.0f32` plateaus at 16777216.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Gaps found while authoring (filed)

- `(f64 expr) as f32` on a computed tree is an EXACT cast:
  the checker rejects it ("cannot prove Exact
  float-to-`f32` cast operand preserves its denotation;
  use a named conversion to round or truncate") even when
  the mathematical value is representable — only
  `F32::from_f64`-style named conversions carry rounding
  policy.
