# Partial rollback over a composed authored selection

`build.omg` authors `CopyPropagation` and `DeadPureScalarElimination`
together; `rollback.txt` declares only `CopyPropagation`. The request's
overlay therefore subtracts one authored rule while the other stays
effective -- the composition axis a single-rule fixture cannot reach.

On the check leg the corpus runner compiles this fixture to its retained
Terminal artifact twice and records:

- `rollback:requested=[CopyPropagation];applied=[CopyPropagation];effective=[DeadPureScalarElimination]`
  -- the settled receipt shows the surviving effective selection;
- `unrolled-receipt:none`, `unrolled-semantic:`/`rolled-back-semantic:`,
  and `module:differs` -- the rolled-back rule demonstrably rewrote the
  module, so its subtraction is not identity.

The `rollback_composed_partial_control` fixture compiles the same
`main.omg` authoring only `DeadPureScalarElimination`: this row's
`rolled-back-semantic` equals the control's fingerprints in the golden,
pinning that partially rolling back a composed selection rejoins a build
that authored only the surviving rule.
