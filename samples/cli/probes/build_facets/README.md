# build_facets

`builder.source` is a live compiler-issued `BuildSource` facet: borrowed
in place it answers `resolve`, `open`, `read`, and `close` against the
activation's source root — no named input assignment needed.

Controls:

- `invalid_source_move`: `let src: BuildSource = builder.source` tries
  to MOVE the non-copy facet out of `builder` — rejects "an owned value
  was moved out of borrowed storage, and the place does not hold a value
  again where the machine returns the borrow". Compiler
  facets are borrow-only.
- `invalid_workspace_root`: a root `build.omg` declaring
  `Build::member` — "a workspace declaration does not name a Product
  source owner". A workspace cannot be the
  `--check` root, matching the dependency-side rejection in
  `build_edge_cases/controls/invalid_member_in_dep`.
