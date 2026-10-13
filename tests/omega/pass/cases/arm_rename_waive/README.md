# arm_rename_waive

Arm-position record binding shares the LET record-pattern grammar:
`dx as step` renames the binding for the `if` guard and target args,
`dy as _` waives a field, `..` opts out of the exhaustiveness law —
three spellings of partial case patterns.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
