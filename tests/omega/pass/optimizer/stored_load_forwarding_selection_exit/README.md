This control selects `SelectedStoredLoadForwardingV1` from an authored Build.
It stores 43 into a context field and immediately observes that field. The
expected native and interpreted exit is 0; an incorrect observation traps.

Run from the repository root with a current Omega binary:

```sh
omega update --project tests/omega/pass/optimizer/stored_load_forwarding_selection_exit --target linux_x86_64
omega run --both --keep tests/omega/pass/optimizer/stored_load_forwarding_selection_exit/main.omg
```

The selection must commit: compare a normal build with the same Build under
`--disable-optimization SelectedStoredLoadForwardingV1`. The selected x86-64
text replaces the field reload with a register copy (76 versus 80 bytes in the
initial witness). The disabled build retains the reload. Neither run changes
the source or its exit oracle.

The selected-stage helper suites independently test exact byte extents,
register transport, joins and cycles, stale/forged edits, and exhausted work
budgets. Distinct place identities alone cannot justify intervening writes;
producer admission and independent replay both conservatively refuse them.
