# IEEE guard ranges

```sh
mbx run -p omega -- run samples/cli/probes/ieee_guard_ranges/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/probes/ieee_guard_ranges` from the repository root.

Expected exit: 0 natively and in Terminal. Checks ordinary values, signed zero,
NaN and infinity through ordered and negated guards, finite source guarantees,
captured Boolean/value copies, indexed values and disjoint guard effects.

NaN exclusion is independent of an ordered interval. A false ordered comparison
does not establish it; a later true ordered comparison or reflexive equality
can. Guard effects must preserve the value being transferred.

