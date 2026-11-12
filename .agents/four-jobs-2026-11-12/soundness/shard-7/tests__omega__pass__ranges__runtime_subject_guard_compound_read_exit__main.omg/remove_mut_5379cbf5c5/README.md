# Compound guard transport

An explicit `Main` subject carries a compound row-major bound into the
`selected` state, which reads element 2 of `[11, 22, 70, 44]`. The caller observes
that read by exiting with 70. A different subject binding supplies no bound;
a write to either index operand invalidates the transported relation.

After normal local package review:

```sh
omega run --both tests/omega/pass/ranges/runtime_subject_guard_compound_read_exit/main.omg
```

Native and checked-interpreter execution must both exit 70. Compiler main
`f64782b8911` refuses the same source with `cannot prove index` in `Main::main::selected`.
