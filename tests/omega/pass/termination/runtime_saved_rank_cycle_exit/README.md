# Saved rank arguments across state arrivals

Run from the repository root:

```sh
omega run tests/omega/pass/termination/runtime_saved_rank_cycle_exit/main.omg
```

Expected native exit: 0. The mutually recursive countdown enters a named state
with reordered, same-typed parameters. Its own `requires pending > 0` proves
the decrement after saving the ranked value. A disjoint marker write must still
execute: the nonempty call returns 7 and the empty call returns its original 3.
Either wrong result traps.

The visible `Distance - Distance` declaration is unrelated to the saved
`u32` operand and must not change its builtin subtraction. A matching
`u32 - u32` override does change the meaning; the negative
`saved_rank_authored_operator_rejected` fixture prevents treating it as a
builtin decrease.

The incoming jump independently establishes the named state's requirements.
`unproved_state_rank_arrival_rejected` pins that obligation;
`stale_state_rank_premise_rejected` prevents using a requirement after a prefix
write replaces its input. Machine-entry requirements are not re-assumed inside
named states.
