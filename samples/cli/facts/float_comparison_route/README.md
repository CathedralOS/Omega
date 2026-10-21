# Declared float comparison route

`left == right` on `f64` selects the declared operator without importing
`omega::language::core::float_operations`. A copy of an `f64` is not this fact.
The values are 212.0 and 2.0, so the comparison is false.

Owning task: **OPERATOR-BOUNDARY-CALLS**. This folder is the std-free witness.
`tests/omega/pending/arithmetic/float_computation_no_route` stays unchanged and
still depends on std.

Positive: [`main.omg`](main.omg). `Expected exit: 0`. `build.omg` binds
`ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and
`linux_arm64` with no `Build::depend` line.

Windows, compiler `30fbbc91026`, `omega run --both`: native exit 0 and
interpreter exit 0. The other three hosts were not executed here.
