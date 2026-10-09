# Cleanup through a narrowed receiver

The original collection's authored requirement establishes the cleanup
prerequisite at the captured old element of a local subslice. The parent leaves
its child's facts intact. The destination is computed once before the
replacement changes its selector; the replacement and all siblings keep their
authored values and qualifiers. The replacement producer explicitly promises
the installed child's facts; the caller does not infer them by replaying its
body. Correct execution returns 0.

```text
omega update --project tests/omega/pass/drops/runtime_narrowed_cleanup_receiver_exit
omega run --both tests/omega/pass/drops/runtime_narrowed_cleanup_receiver_exit/main.omg
```
