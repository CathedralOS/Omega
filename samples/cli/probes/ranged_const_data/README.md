# ranged const data

Compile-scope probe. No standard-library dependency.

- `data Buffer<const Capacity: u64> requires Capacity >= 1 && Capacity
  <= 64` — where-gates constrain const data parameters at each
  instantiation (`Buffer<2>`, `Buffer<64>`).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/ranged_const_data/main.omg
mbx run -p omega -- update --project samples/cli/probes/ranged_const_data
```
