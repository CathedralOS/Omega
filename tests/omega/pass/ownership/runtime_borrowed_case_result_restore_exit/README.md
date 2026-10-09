# Recoverable outcomes restore caller-owned storage

A consuming transform returns an ordinary success/failure sum with one active
linear payload. The failure arm returns the original token; the success arm
disposes it and returns its replacement. Both outcomes repair the same borrowed
field, retain its sibling and execute through selected control-flow cleanup.
Native and Terminal must both exit 0.

Run `omega run --both tests/omega/pass/ownership/runtime_borrowed_case_result_restore_exit/main.omg`.
