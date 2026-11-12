Run `omega run tests/omega/pass/slices/runtime_boolean_alias_guard_transport_exit/main.omg`.
The authored result is exit 0 in native and Terminal execution.

A Boolean alias carries its builtin comparison, and a subsequent selected
index guard carries the exact view bound into the reading state. Neither an
already-handled alias nor a ready comparison may be mistaken for the other.
