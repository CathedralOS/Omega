# binding_where_record

`data Session where retries <= 3 { console: Binding<Console>; retries:
u64; }` — a data `where` constrains a record's scalar field beside a
service `Binding<T>` field: the facts don't hide the service field from
establishment.

## Learnings

- `Binding<T>` fields require `pub boundary trait T {}` — a non-public
  or missing declaration reports "requires a public boundary trait as
  its stable slot contract" / "does not name an exact boundary-trait
  requirement".

Compile-checked only: `core::binding` is depend-free.
