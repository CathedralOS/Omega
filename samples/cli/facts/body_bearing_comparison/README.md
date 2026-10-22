# Body-bearing match equality

Equal `f64 in Marked` carriers. The selected `==` body returns false, so the
direct comparison is false and the match takes the wildcard. Builtin equality
would do the opposite.

Owning task: **OPERATOR-MACHINE-SUPPLY**. This folder is the std-free witness
for a body-bearing spelled comparison. A bodyless floating comparison of `f64`
stays on the comparison route in `float_comparison_route`.

Positive: [`main.omg`](main.omg). `Expected exit: 0`. `build.omg` binds
`ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and
`linux_arm64` with no `Build::depend` line.

Before the rewrite, checking refused: `` `same` was selected for its fixed
operator token `==` in a position whose body supply is not implemented ``.
Windows, after the rewrite, `omega run --both`: native exit 0 and interpreter
exit 0. The other three hosts were not executed here. Bodyless
`float_comparison_route` on the same compiler: native exit 0 and interpreter
exit 0.
