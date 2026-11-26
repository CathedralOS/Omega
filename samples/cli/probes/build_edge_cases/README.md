# build_edge_cases

Loose ends of the package-build surface that all reject legibly at
closure resolution or build evaluation — plus one quiet positive:
a package can set `subsystem` and `identifier`. Those settings apply when
Build declares an application.

Controls:

- `invalid_dup_build` declares two free Build entries. Source discovery
  reports both `build` declarations before evaluation.
- `invalid_dep_no_build`: a dependency directory with no `build.omg` —
  "package build file is missing: <snapshot>/source/build.omg" (hash
  path elided). The root-side shape (`Source::Path` on a missing
  directory entirely) is pinned in `build_dep_graph`.
- `invalid_member_in_dep` registers a workspace as a dependency.
  The workspace declaration supplies no Product source owner.
- `invalid_use_fallback` has no `ghost` dependency edge. The entry selects a declaration from
  `{ghost}/main.omg`, so checking requires that missing dependency.
- `invalid_output_fail`: `BuildOutput::fail` surfaces the authored
  message verbatim as the diagnostic — "required output `out.bin` of
  `build` failed: deliberate failure probe".
- `invalid_nested_output`: a required output named with a directory
  component (`bin/app`) never seals — the full
  require→resolve→create→write→close→complete loop returns Retry and
  the obligation ends "declared but never completed". The prelude
  exposes no parent-directory creation, so nested output names cannot
  be discharged from authored code today.
