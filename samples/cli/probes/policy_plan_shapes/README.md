# policy_plan_shapes

A wire-policy plan vocabulary: `FieldPlan [copy]` with `At{offset}` /
`Bits{container, container_width, lsb, width}` / `Varint{tag}` /
`LengthPrefixed{tag}` cases beside a `Plan { fields: [FieldPlan; 32] }`
roster — the wire family's placement shape, distinct from the layout
`Bits` fragment vocabulary. `transition field { Case{p} -> (expr) }`
dispatches on all four.

## Learnings

- Decision 17 is operand-driven: `(a as u64 in Wrapping) + b` mixes
  domains — wrap EACH operand (`offset as u64 in Wrapping`, literals
  included) or re-domain intermediates.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
