# operator_requires_effects

Runnable probe; no standard-library dependency. `omega run main.omg` must exit 0
natively after `omega update --project .` reviews the local package inputs.

A boundary operator's operand `requires` is discharged through an
effect-bearing call: `boundary machine == Integer::equal ... requires
right >= 0`, `prepare(&mut value) ensures value >= 0` —
`prepare(&mut value) == value` proves the right operand after the effect lands.
`build.omg` selects the checked satisfier `CheckedEquality::equal` for the
`Integer::equal` slot, so the comparison runs its body.

Without a satisfier, native closure review refuses the program
(`reachable requirement ... resolves to 0 selected provider rows`), pinned by
`tests/omega/fail/providers/boundary_operator_without_provider`.
