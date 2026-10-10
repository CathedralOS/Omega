# Array Domain Field

Compile-scope probe: byte-sequence domains on sized-array fields. No
standard-library dependency.

- `domain [u8; 8]::Utf8 requires valid_utf8` — the array becomes a
  byte-sequence carrier with whole-replacement + extent.
- `main.label = "abc"` — whole-replacement (extent 3 within
  capacity 8); `main.label[0] = 66` — indexed byte store.
- Reads live one state after the fill: the extent fact follows the
  edge. A plain `[u8; N]` field is a scalar array and carries neither
  whole replacement nor extent.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/array_domain_field/main.omg
mbx run -p omega -- update --project samples/cli/probes/array_domain_field
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
