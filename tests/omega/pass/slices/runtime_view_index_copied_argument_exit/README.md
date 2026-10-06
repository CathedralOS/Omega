A scalar helper copies a record selected through a borrowed element view directly into a call argument. The checked index bound uses the view’s current length, and the callee reads the nested array from the copied record. Expected exit: 70.

```sh
omega update --project tests/omega/pass/slices/runtime_view_index_copied_argument_exit --target macos_arm64 --offline
omega run --both tests/omega/pass/slices/runtime_view_index_copied_argument_exit/main.omg
```

If update produces a package review, record its decisions and resume the update before running.
