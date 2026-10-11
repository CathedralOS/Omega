A write-only child captures a runtime-selected record in the original array.
Changing the selector afterward must not redirect either ordinary helper call
or the subsequent direct field store through the captured reference.
Both engines must preserve the neighboring value and both padding fields.

Run `omega run --both tests/omega/pass/borrow/runtime_write_only_array_selector_snapshot_exit/main.omg`
with stdin closed. The authored native and Terminal exit is 0.
