# sibling_dep_queries

Sibling path dep resolved by declared package name: `Build::depend(..., Source::Path{location:"dependency"})` composes `query_dependency`, imported as `use query_dependency::value`. The dep's own build machine runs too — `BuildProduct::entry(&builder.product, path, "windows_x86_64::ProgramEntry")` entry queries are build-time observations; package-level `unused` machines stay private.

Compile-checked only: run a check target, not native.
