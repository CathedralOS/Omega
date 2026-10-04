# Discarded reference releases its backing

Run `omega run --both tests/omega/pass/borrows/runtime_discarded_reference_release_exit/main.omg`.
Both engines must exit 70. A helper returns a shared primitive reference through
its transition, and the caller discards the result. The immediately following
statement mutates the same backing field. The guard checks its new value and
the untouched neighboring field, exiting 71 if either value is wrong.

The discarded call still requires ordinary call and reference-result validation.
Its anonymous result must end before the following mutation restores exclusive
access; merely omitting the result's lifetime is not sufficient.
