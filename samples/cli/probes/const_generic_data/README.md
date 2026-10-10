# Const Generic Data

Compile-scope probe: const-generic data parameters. No
standard-library dependency.

- `data Buffer<const Size: u64>` / `data Flag<const Enabled: bool>`
  / `data Indexed<const Selected: [u64; 2]>` — u64, bool, and
  ARRAY-typed const parameters on data.
- Const initializers evaluate `match` expressions and anonymous
  arithmetic with EXACT fractional intermediates (`7/2*2 == 7` —
  comptime `/` is rational, not truncating).
- `keep(Buffer<SIZE>) -> Buffer<7>` / `selected(Flag<ENABLED>) ->
  Flag<true>` — identical selections unify by value.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/const_generic_data/main.omg
mbx run -p omega -- update --project samples/cli/probes/const_generic_data
```

Both green on Linux x86-64 (exact-fraction warnings document the
anonymous-arithmetic semantics); `omega.lock` ignored.
