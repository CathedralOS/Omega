# Field replacement transports contained loans

Replacing an occupied field retires the displaced carrier and installs the
replacement's exact reference leaves. The replacement selects a different
backing (42 instead of 39), while a disjoint sibling retains its original
backing and a scalar prefix stays 7. Native and Terminal execution must both
exit 42. Equal types do not imply equal loan roots or equal storage offsets.

After normal package review, run from the repository root:

```sh
omega run --both tests/omega/pass/ownership/runtime_carried_loan_field_replacement_exit/main.omg
```
