# build_use_forms

`use` rows are idempotent: repeating `use kit::main` imports the same
module twice without conflict.

Controls:

- `invalid_use_wildcard`: `use kit::*` — there is no wildcard import;
  the `*` rejects at parse ("expected identifier, found
  punctuation"). Each imported module must be spelled out.
