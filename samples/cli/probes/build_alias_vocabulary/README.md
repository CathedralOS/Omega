# build_alias_vocabulary

Dependency registration validates aliases as snake_case Omega identifiers.
They begin with an ASCII lowercase letter and contain only ASCII lowercase
letters, digits, and single interior underscores. `kit9` is legal here and
`kit_a` is used by `build_dep_diamond`. The positive probe explicitly captures
its nested `dep` package together with the root sources.

Each negative checks the registration diagnostic naming its invalid alias
and the exact spelling rule.

- `invalid_alias_empty` supplies the empty string.
- `invalid_alias_upper` supplies `KIT` with capital letters.
- `invalid_alias_underscore_edge` supplies `_kit` with a leading underscore.
