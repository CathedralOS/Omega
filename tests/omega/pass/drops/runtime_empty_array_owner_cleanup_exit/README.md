# Empty-array owner cleanup

Run `omega run --both tests/omega/pass/drops/runtime_empty_array_owner_cleanup_exit/main.omg`.
The zero-length array contains no gated token, so the containing owner is a
complete value and owes its nominal cleanup. The program expects exit 0.
The retiring Terminal zero-establishment replay currently rejects this shape
with `ZeroedStructuralResultMismatch`; source checking and cleanup planning
must not turn that consumer limitation into permission to skip the hook.
