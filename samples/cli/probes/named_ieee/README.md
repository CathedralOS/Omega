# named ieee

Compile-scope probe. Imports `omega::language::core::float_operations`
(depend-free).

- `F64::negate`, `F64::minimum`/`maximum` (NaN-asymmetric:
  minimum(NaN,1)=1, minimum(1,NaN)=NaN, minimum(0,-0)=-0),
  `F64::square_root`/`F32::square_root` nearest-even.
- Spellings learned: sqrt is `square_root`; `0.0/0.0` is 'anonymous
  division by zero' — NaN must come from named storage
  (`main.zero / main.zero`).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/named_ieee/main.omg
mbx run -p omega -- update --project samples/cli/probes/named_ieee
```
