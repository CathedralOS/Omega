# Generic cleanup integer prerequisite

`forward` transfers the same owner into `retire`. Both invocations retain the
hook's integer prerequisite without adding an authored caller requirement.
The exact value satisfies the clause; successful cleanup exits 0.

```sh
cargo run -p omega -- run tests/omega/pass/drops/runtime_generic_nominal_integer_prerequisite_exit/main.omg
```
