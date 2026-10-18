# ref type args

Compile-scope probe. No standard-library dependency.

- `ceiling<&Element>()` — `&T` reference TYPE as a static generic
  argument; `where value <= ceiling<&Element>()` result as symbolic
  bound in a data field; `unused<&Element, 7>` relayed.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/ref_type_args/main.omg
mbx run -p omega -- update --project samples/cli/probes/ref_type_args
```
