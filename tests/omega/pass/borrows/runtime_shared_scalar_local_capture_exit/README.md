Run from the repository root after preparing the project dependencies:

```sh
omega run --both tests/omega/pass/borrows/runtime_shared_scalar_local_capture_exit/main.omg
```

Both engines must exit 70. An effectful helper initializes one immutable scalar.
Two shared borrows must observe that completed value from its original storage;
the counter must remain one, proving address formation did not reevaluate the
initializer.
