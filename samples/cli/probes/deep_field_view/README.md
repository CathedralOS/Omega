# deep field view

Compile-scope probe. No standard-library dependency.

- `&packet.samples.items[..]` — doubly-nested-field slice view
  from an owned state arrival; the record stays readable beside
  the loan.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/deep_field_view/main.omg
mbx run -p omega -- update --project samples/cli/probes/deep_field_view
```
