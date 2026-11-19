# build_mod_resolution

`use kit::util` accepts either module shape: `util.omg` (flat) or
`util/mod.omg` (directory). The positive sample resolves the
directory form.

- `invalid_dir_no_mod`: a `util/` directory containing no `mod.omg`
  (and no sibling `util.omg`) reports a raw os error naming only the
  `util.omg` candidate — the `mod.omg` alternative is silently
  dropped from the diagnostic. Same minor legibility leak as
  missing-module imports elsewhere.
