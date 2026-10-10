# boundary_scalar_requirement

`boundary requirement TimedWait::poll(descriptors,
count, timeout) -> i32 reaches Clock` — a bodyless
consuming contract publishing a scalar result; a later
state forwards `TimedWait::poll(0, 0, 20)` as the
machine's `-> i32` result. The caller declares
`reaches Clock` for the service it invokes.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
