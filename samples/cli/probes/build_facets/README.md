# build_facets

`builder.source` is a live compiler-issued `BuildSource` facet: borrowed
in place it answers `resolve`, `open`, `read`, and `close` against the
activation's source root — no named input assignment needed.

Controls:

- `invalid_source_move`: `let src: BuildSource = builder.source` tries
  to MOVE the non-copy facet out of `builder` — rejects "cannot
  transfer a non-copy value out of borrowed storage without replacing
  its owner ... `builder.source` must hold a value again". Compiler
  facets are borrow-only.
- `invalid_workspace_root`: a root `build.omg` declaring
  `Build::member` — "expected a package build declaration, found an
  explicit workspace declaration". A workspace cannot be the
  `--check` root, matching the dependency-side rejection in
  `build_edge_cases/controls/invalid_member_in_dep`.
