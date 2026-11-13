# Indexed borrowed call arguments

Run `omega run --both tests/omega/pass/calls/runtime_indexed_scalar_calls_exit/main.omg`.
Both engines must exit 70. Receiver and explicit argument calls borrow the
runtime-selected element through a shared named-state parameter. The test also
checks a different element and a position outside the guarded range.

Call-origin replay resolves the complete storage projection independently of
the stricter rules for establishing an escaping reference value. Bounds, access
and live-loan checks remain operation and custody obligations.
