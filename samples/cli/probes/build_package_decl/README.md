# build_package_decl

Package-kind declaration vocabulary and module resolution: the
positive sample self-imports its own module (`use
build_package_decl::main` compiles — self-edges by declared name are
idempotent).

- `invalid_dup_kind`: `Build::application` twice rejects "application
  build declares its application name 2 times" — kind calls are
  at-most-once and the conflict names the name argument.
- `invalid_two_kinds`: `application` + `package` rejects "build must
  declare exactly one kind; package, application, and workspace
  member declarations cannot be mixed".
- `invalid_missing_module`: `use own::dep` where `dep.omg` does not
  exist rejects "captured entry is absent: dep.omg" — module rows
  bind to real files.
- `invalid_nested_entry`: a bare `entry()` statement inside a machine
  body parses as the retired nested-member form — "explicit nested
  `entry` / `pub entry` machine members are retired".
