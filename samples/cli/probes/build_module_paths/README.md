# build_module_paths

The entry imports `deep/nested/main.omg` from its own package and calls
`|nested|::helper()`.

- `invalid_undeclared_dir`: the entry selects a declaration from `ghost/main.omg`.
  Its package has no admitted file at that path.
- `invalid_depbuild_in_build` registers a package under the alias `kit`
  and returns `stage.omg` as its next Build. That Build tries to import
  `{kit}/build.omg`. The dependency’s entered Build file is private to
  its planning sequence.
