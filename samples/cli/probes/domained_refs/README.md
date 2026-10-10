# domained refs

Compile-scope probe. No standard-library dependency.

- `counter: &Counter in Counter::Ready` — domain-qualified BORROW
  parameter; `domain X::ReadyAlias = Ready` alias.
- `[Receipt in Issued; 2]` array-of-domained linear values;
  `Slot { receipt: Receipt in Issued }` domain'd linear field.
- `(Counter{code:0} as Counter in Ready)` — `as` grants a domain on
  a record literal.
- Learning: `pub domain X::D` requires `pub data X`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/domained_refs/main.omg
mbx run -p omega -- update --project samples/cli/probes/domained_refs
```
