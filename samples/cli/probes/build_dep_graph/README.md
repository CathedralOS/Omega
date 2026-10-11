# build_dep_graph

Dependency-closure validation at resolution time:

- `invalid_dep_cycle`: a package→package dependency cycle (pkga → pkgb
  → pkga) is detected during closure resolution — "resolved package
  source closure failed 1 graph validation check(s); dependency
  cycle: pkga -> pkgb -> pkga".
- `invalid_missing_source`: a `Source::Path` naming a nonexistent
  directory rejects at the source adapter — "cannot resolve package
  source: .../missing_dir: No such file or directory (os error 2)".
