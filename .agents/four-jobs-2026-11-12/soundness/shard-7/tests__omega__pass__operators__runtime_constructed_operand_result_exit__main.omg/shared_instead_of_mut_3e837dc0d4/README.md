# Constructed operands in scalar expressions

Run through the public compiler on macOS ARM64:

```sh
omega update --project tests/omega/pass/operators/runtime_constructed_operand_result_exit --target macos_arm64 --offline
omega run --both tests/omega/pass/operators/runtime_constructed_operand_result_exit/main.omg
```

Both engines must exit 0. The program keeps constructions inside their authored
expressions: declared operators and named calls consume record values, while
ordinary calls consume scalar arrays and cases. The two nominal operand types
select different operations despite sharing a layout. The widening calculation
distinguishes an f64 result from premature f32 rounding.

A counter checks once-only evaluation across mixed scalar and structural
arguments, including fields authored out of declaration order. Runtime
short-circuiting must skip those effects. Conditional transfer consumes an
earlier affine local only on the selected path, while another local survives;
normal cleanup must match each path's actual owners.

Independent controls include named linear argument transfers in
`pass/calls/runtime_linear_guarded_unit_owner_disposition_exit`, value snapshots
across later mutation in `pass/structural/runtime_structural_operand_snapshot_exit`,
and duplicate-move rejection in `fail/structs/affine_result_constructor_reuse`.
`fail/operators/constructed_operand_wrong_nominal_type` rejects same-layout
operands whose declared types do not select the operation.
