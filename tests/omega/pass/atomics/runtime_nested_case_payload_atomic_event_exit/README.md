# Atomic storage nested beneath a selected payload

```sh
omega run --both tests/omega/pass/atomics/runtime_nested_case_payload_atomic_event_exit/main.omg
```

Both engines must exit 0, observing prior 11 and resident 20. Record and case
initializers establish the same exact atomic scalar carrier. The selected
Case/Field path reaches the original nested backing; neither the result
guarantee nor address reconstruction supplies mutation authority by itself.
