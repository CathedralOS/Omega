Boolean equality evaluates a copied record argument on either selected continuation. Both left outcomes execute; each consumer runs once, and the original record remains available. Expected exit: 70.

```sh
omega update --project tests/omega/pass/control_flow/runtime_boolean_equality_copied_record_instances_exit --target macos_arm64 --offline
omega run --both tests/omega/pass/control_flow/runtime_boolean_equality_copied_record_instances_exit/main.omg
```

If update produces a package review, record its decisions and resume the update before running.
