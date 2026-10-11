# build_alias_multi

One source under two aliases: `kit1` and `kit2` both name `dep/` —
aliases are the import identity, so the same package is importable
twice under different names and both `use` rows resolve.

- `invalid_use_as`: `use kit::main as k` — `as` aliasing does not
  exist on use rows; parse rejects "expected `;`, found keyword
  `as`". Alias selection happens at the `depend_as` row, not the
  import.
- `invalid_bad_field`: `builder.bogus = 1` rejects "data `Build` has
  no field `bogus`" — field names are a closed schema.

Field schema is kind-agnostic: `builder.freestanding` writes are
legal on a `package` build too — the flag simply has no artifact to
apply to.
