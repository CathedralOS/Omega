# float gate

Compile-scope probe. Imports `omega::language::core::float_operations`.

- `data Main requires meter_value >= 0.0 && meter_value <= 1.0` — float
  where-gate rides the carrier (zero-init satisfies); mid-body
  stores are not construction — `= 2.0` writes out-of-gate, `= 0.25`
  restores.

## Gaps found while authoring (filed)

- `Meter { value: 0.5 }` literal construction FAILS to establish the
  identical gate: 'a where fact is not established by the values
  observed at this construction'. Float where-gates only work on
  carrier fields today — literal gating of float bounds has no
  route.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/float_gate/main.omg
mbx run -p omega -- update --project samples/cli/probes/float_gate
```
