# build_dep_graph

Dependency-closure validation at resolution time:

- `invalid_dep_cycle`: a package→package dependency cycle (pkga → pkgb
  → pkga) is detected during closure resolution — "resolved package
  source closure failed 1 graph validation check(s); dependency
  cycle: pkga -> pkgb -> pkga".
- `invalid_missing_source`: a `Source::Path` naming a nonexistent
  directory rejects at the source adapter — "cannot resolve package
  source: .../missing_dir: No such file or directory (os error 2)".
- `invalid_dup_alias`: two edges under the same alias conflict —
  "dependency alias `kit` conflicts between occurrences 0 and 1"
  (distinct aliases on one source are legal; see
  `build_dup_product_edge`).
- `invalid_double_application`: one build declaring `application`
  twice rejects — "application build declares its application name 2
  times".
- `invalid_file_source`: a `Source::Path` naming a file rejects —
  "source root `.../main.omg` is not a directory".
