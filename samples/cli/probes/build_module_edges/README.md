# build_module_edges

Two module-boundary pins:

- Flat wins: when `util.omg` and `util/mod.omg` both exist, `use
  kit::util` resolves the flat file and `dir()` in `mod.omg` is
  unreachable through the import — deterministic precedence, no
  ambiguity diagnostic.
- `invalid_use_dep_build`: `use kit::build` may not load the
  dependency's `build.omg` — package imports reject the build file
  explicitly ("may not load dependency build file").
