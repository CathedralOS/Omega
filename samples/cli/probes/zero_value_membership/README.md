# zero value membership

Compile-scope probe. No standard-library dependency.

- `ensures zero_value<Maybe<T>>() in Maybe::None` — case membership
  of a generic sum's zero value in a contract.
- `proof [erased]: Evidence` erased case payload; `Maybe::Some
  { value, proof as _ }` discard.

## Gaps found while authoring (filed)

- Turbofish call `zero_is_none::<i32>()` does not parse; a zero-arg
  generic machine can't infer T — its ensures is declaration
  surface only.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/zero_value_membership/main.omg
mbx run -p omega -- update --project samples/cli/probes/zero_value_membership
```
