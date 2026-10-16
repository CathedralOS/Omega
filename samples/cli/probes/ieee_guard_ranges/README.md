# IEEE guard ranges

```sh
mbx run -p omega -- run --both samples/cli/probes/ieee_guard_ranges/main.omg
```

Expected exit: 0 natively and in Terminal. Checks ordinary values, signed zero,
NaN and infinity through ordered and negated guards, finite source guarantees,
captured Boolean/value copies, indexed values and disjoint guard effects.

NaN exclusion is independent of an ordered interval. A false ordered comparison
does not establish it; a later true ordered comparison or reflexive equality
can. Guard effects must preserve the value being transferred.

The matching invalid controls live under `tests/omega/fail/float/guarded_range_*`.
