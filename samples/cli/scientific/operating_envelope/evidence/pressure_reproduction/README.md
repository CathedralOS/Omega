# Complete intended-valid allocator reproduction

The program combines the existing 32-term trapping-add pressure control with
the application's ordinary decimal formatter state loop. Expected native
stdout is exactly `1120` (no newline), and expected native/Terminal exit is 70.
These oracles were fixed before executing the reproduction.

From repository root, prepare its build metadata using ordinary `omega update
--project samples/cli/independent/operating_envelope/evidence/pressure_reproduction`,
review required std/Console decisions, then run:

```sh
omega run --both --keep samples/cli/independent/operating_envelope/evidence/pressure_reproduction/main.omg
```

The current-main candidate fails native allocation before executable creation:
`RuntimeSpill(Homes(UnresolvedEntryTransitions { function: 2, register: 7, count: 2 }))`.
The excluded failed prototype and both exact verdicts are in the review bundle.
This is intended valid source, distinct from the sample's intended-invalid
radius source and invalid runtime inputs.
