# operator_requires_effects

Runnable probe; no standard-library dependency. `omega run main.omg` must exit 0
natively after `omega packages update --project .` reviews the local package inputs.

A boundary operator's operand `requires` is discharged through an
effect-bearing call: `boundary machine == Integer::equal ... requires
right >= 0`, `prepare(&mut held) ensures value >= 0` —
`prepare(&mut held) == held` proves the right operand after the effect lands.
The operands are `i32 in Counted`, the probe's own domain home: bare i32
operands would join the core's primitive `==` family.
`build.omg` selects the checked satisfier `CheckedEquality::equal` for the
`Integer::equal` slot, so the comparison runs its body. The source's direct
boundary declaration fixes `==` to that slot; the Build row supplies only its
provider and cannot activate or change the token binding.

Without a satisfier, native closure review refuses the program
(`reachable requirement ... resolves to 0 selected provider rows`), pinned by
`tests/omega/fail/providers/boundary_operator_without_provider`.
