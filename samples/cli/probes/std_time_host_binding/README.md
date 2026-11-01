# std_time_host_binding

`Time::now`, a 50 ms `Time::sleep_for`, and `Time::elapsed_since` use the package-selected `TimeHost` provider. The unchanged program requires `slept == 50` and an elapsed duration of at least 50 ms, then exits 70.

This is the TimeHost cohort of `BUILD-OWNED-TARGET-SELECTION` and `BUILD-OWNED-NATIVE-BINDINGS`. `std/build.omg::configure_time_host` selects ordinary Windows, Linux x86-64, Linux arm64 and shared Darwin implementations for the five hosted target rows. The owning package supplies the private raw clock and wait requirements directly through Build; callers retain the public TimeHost contract. The existing four Windows native rows retain their signatures and locators.

The customer is pinned at `tests/omega/pass/time/runtime_std_time_host_binding_exit`; `runtime_time_host_native_darwin_exit` checks all six operations, including the nanosecond calibration and Unix epoch. Fresh macOS arm64 native and Terminal execution both exit 70. The selected-target control substitutes a PE locator for the Darwin clock read and is rejected as inapplicable to macOS. Existing dependency-private native binding and package-default override controls retain their obligations.

These results cover the macOS customer path. Native execution on other matching hosts and the parent tasks' remaining source migrations stay open. `runtime_time_host_virtual_exit` retains its older checked-interpreter-only 1000 Hz oracle; it has no ProgramEntry and supplies no Terminal or native acceptance for this cohort.

Expected exit: 70.
