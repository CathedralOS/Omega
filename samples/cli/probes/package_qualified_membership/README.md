# package qualified membership

Compile-scope probe; FIRST depend-package sample —
`Build::depend_as(builder, "leaf", Source::Path { location:
"leaf" })` with the sibling package under `leaf/` (own build.omg +
module file).

- `use leaf::outcomes;` imports the depend's module;
  `leaf::outcomes::Outcome::Empty` case-membership and constructor
  through the package path; `::Empty` and `::Empty {}` select the
  same payload-free case.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/package_qualified_membership/main.omg
mbx run -p omega -- update --project samples/cli/probes/package_qualified_membership
```
