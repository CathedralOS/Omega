# build_depend_unaliased

`Build::depend` + `Build::build_depend` select `dependency/` with no alias.
The dep declares `Build::package(builder, "custody_kernel")`.

Both scopes try the dep's declared name: `use custody_kernel::main` in
`build.omg` and in `main.omg`. If the unaliased family names imports by
declared package name this compiles; if the family names them by directory
or merges unqualified the `use` fails and this becomes a control.
