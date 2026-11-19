# build_alias_multi

The Build registers `dep/` under two aliases, `kit1` and `kit2`.
The Product imports both package-root modules and adds `kit1::v()` and
`kit2::w()`. The result contracts expose 1 and 2 for the exact sum.

- `invalid_use_as` checks that the parser rejects `as` in a `use` item.
  Build dependency registration supplies the alias.
- `invalid_bad_field` assigns `builder.bogus = 1` and checks the missing-field
  diagnostic.
