# build_package_root

A root package declared `Build::package` checks without any entry
machine: `--check` on a library root validates the package's own
sources with no application entry to select. Contrast with
`omega run`, which needs one exact `ProgramEntry` bind.
