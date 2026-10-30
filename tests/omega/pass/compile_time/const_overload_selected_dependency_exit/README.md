# Selected overload dependencies in a constant initializer

`ROOT: u32 = choose(1u32)` selects the `u32` overload, which returns its operand.
The unselected `u64` overload reads `ROOT`; inspecting that body as an initializer
dependency would invent a `ROOT -> ROOT` cycle. `SELECTED` must be 37, so choosing
the other body cannot pass by coincidentally returning ROOT's value of 1. The
runtime `u64` call must read the settled ROOT and return 1 rather than its input 73.
All four hosted targets bind `main`; authored native and Terminal exit is 0.

The [reject twin](../../../fail/compile_time/const_overload_selected_dependency_cycle_rejected/main.omg)
changes only the selected `u32` body from `value` to `ROOT`. That application has a
real initializer cycle and must reject for that dependency. These are authored
oracles, not observed results. The parent shared lane owns the frozen R25 baseline
and candidate runs; no sidecar execution has been performed.
