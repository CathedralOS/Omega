# nested field loans

Compile-scope probe. No standard-library dependency.

- `Inner::inspect(&outer.inner)` — nested-field borrow through an
  `&Outer` parameter, from the entry and from a forwarded state
  parameter; `-> &u64` loan returning `&inner.value`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/nested_field_loans/main.omg
mbx run -p omega -- update --project samples/cli/probes/nested_field_loans
```
