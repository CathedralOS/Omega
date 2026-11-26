# build_alias_multi

The Build registers `dep/` under two aliases, `kit1` and `kit2`.
The Product imports `main.omg` from both dependency roots and adds
`|kit1|::v()` and `|kit2|::w()`. The result contracts expose 1 and 2 for the exact sum.

- `invalid_use_as` omits the identifier after `as`. The parser requires a file alias there.
- `invalid_bad_field` assigns `builder.bogus = 1` and checks the missing-field
  diagnostic.
