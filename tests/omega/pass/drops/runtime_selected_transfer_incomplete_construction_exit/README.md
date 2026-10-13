Run `omega run --both tests/omega/pass/drops/runtime_selected_transfer_incomplete_construction_exit/main.omg`.
A deferred binding (`let mut pair: Pair<Token>;`, `Token.value` ranged so zero
bits are not a value) is established one field store at a time, and a selected
(`&&`-guarded) consuming call transfers one field while the construction is
still incomplete. Both engines exit 0 with the authored hook order:

- `mid_construction`: first stored, selected consume of `first`, then second
  stored. Retained path `CBA`; transferred path `ACB`.
- `reversed_establishment`: second stored before first, then a selected
  consume of `first` after the construction completed: `CBA` / `ACB`.
- `reversed_mid_construction`: second stored, selected consume of `second`,
  then first stored. The retained path completes the aggregate, which folds
  into one value and dies in declared order: `CBA`; the transferred path
  disposes only `first`: `BCA`.
- `incomplete_reversed` (a three-field record whose third field is never
  stored): second stored, selected consume of `second`, then first stored.
  The aggregate never completes, so the retained path dies in reverse
  establishment order, `first` before `second`: `CAB`; transferred path `BCA`.

Each line is one call: a consumed token's hook prints inside the callee, then
the test machine's exit prints the youngest local (`C`) and the pair's
remaining fields. The fixture explicitly assumes finite normal completion of
its reviewed byte-write provider and binds real libc/MSVCRT imports, like
`runtime_selected_partial_owner_cleanup_exit`.
