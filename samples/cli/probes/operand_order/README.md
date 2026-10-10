# Operand Order + Operator Overload

Compile-scope probe: ordinary `machine +` operator overloading on user
data, plus the argument-evaluation contract. No standard-library
dependency.

- `machine + Score::add` — ordinary operator on `[copy]` data; the
  result carries a `u64 in Wrapping` domain (return type need not be
  the operand type).
- `forward<Value>` — unconstrained generic identity by value.
- `in_order(tick, Ordered{second: tick, first: tick}, tick)` — record
  fields initialize in WRITTEN order, not declaration order: the
  side-effecting `tick` counter reads 1, 2, 3, 4.
- `hits` counts arms; 1 proves all four arguments ran in order.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/operand_order/main.omg
mbx run -p omega -- update --project samples/cli/probes/operand_order
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
