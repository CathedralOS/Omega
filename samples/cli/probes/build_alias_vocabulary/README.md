# build_alias_vocabulary

Dependency aliases are validated as snake_case Omega identifier
spellings at dependency projection: lowercase letters, digits after a
leading letter, underscores strictly interior (`kit9` legal here;
`kit_a` used by `build_dep_diamond`).

Every boundary spelling rejects with one diagnostic — "dependency
alias `<x>` must use snake_case Omega identifier spelling":

- `invalid_alias_empty`: the empty string.
- `invalid_alias_upper`: `KIT` — capitals.
- `invalid_alias_underscore_edge`: `_kit` — leading underscore
  (trailing `kit_` rejects identically, as do `9lives` leading digit,
  `kit-name` hyphen, and `ki t` space — all verified).
