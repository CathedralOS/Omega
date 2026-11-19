# truncating_division

First sample pinning per-carrier division selection:
`u64 / u64` realized-exact, `i32 / i32` truncating, `%`
the paired remainder — through let binding, transition
argument, and guard operand; `(0-17)/5 + (0-17)%5` folds
signed truncation to -5.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
