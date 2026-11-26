# build_module_paths

`use` paths are module directory paths: `use deep::nested::main`
loads `deep/nested/main.omg` from the declaring package — each `::`
segment is a directory (or the alias of a declared dependency) and
the last names the module file.

- `invalid_undeclared_dir`: `use ghost::main` with no declared alias
  and no `ghost/` directory resolves the prefix as an own-package
  directory and rejects "captured entry is absent: ghost".
- `invalid_depbuild_in_build` registers a package under the alias `kit`
  and returns `stage.omg` as its next Build. That Build tries to import
  `{kit}/build.omg`. The dependency’s entered Build file is private to
  its planning sequence.
