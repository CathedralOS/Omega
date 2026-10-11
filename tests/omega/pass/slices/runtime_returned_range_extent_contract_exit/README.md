# Returned descriptor extent

`omega run --both tests/omega/pass/slices/runtime_returned_range_extent_contract_exit/main.omg`
must exit 0 in native and Terminal execution. A checked named implementation
proves the length of its actual returned slice; the caller uses that published
guarantee to call a nonempty-view consumer without establishing a fresh guard.
Bounds and the backing loan remain independent obligations.
