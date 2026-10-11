# build_module_paths

`use` paths are module directory paths: `use deep::nested::main`
loads `deep/nested/main.omg` from the declaring package — each `::`
segment is a directory (or the alias of a declared dependency) and
the last names the module file.

- `invalid_undeclared_dir`: `use ghost::main` with no declared alias
  and no `ghost/` directory resolves the prefix as an own-package
  directory and rejects "captured entry is absent: ghost".
- `invalid_depbuild_in_build`: `use kit::build` inside build.omg
  over a `build_depend_as` edge — dependency build files are
  load-protected in BOTH scopes ("may not load dependency build
  file"), not just product imports.
