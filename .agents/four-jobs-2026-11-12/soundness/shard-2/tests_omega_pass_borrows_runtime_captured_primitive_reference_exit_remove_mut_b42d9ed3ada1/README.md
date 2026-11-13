After preparing the project dependencies, run from the repository root:

```sh
omega run --both tests/omega/pass/borrows/runtime_captured_primitive_reference_exit/main.omg
```

Both engines must exit 70. The shared reference captures the first byte before
its selector changes. Reading it must observe that captured address, while an
ordinary array observation uses the new selector and sees the adjacent byte.
