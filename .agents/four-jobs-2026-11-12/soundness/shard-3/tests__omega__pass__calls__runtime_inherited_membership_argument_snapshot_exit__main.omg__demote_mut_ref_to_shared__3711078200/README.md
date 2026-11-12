An inherited requirement binds through the declaration's parameter identities,
even when the implementing machine uses different names. The earlier owned
argument keeps its issued value after a later argument replaces the source.
The callee observes 41/9; the caller observes 0/0.

Run `omega run --both tests/omega/pass/calls/runtime_inherited_membership_argument_snapshot_exit/main.omg`.
Both engines must exit 0.
