# Stored primitive reference observations

With the compiler on PATH:

```sh
omega update --project tests/omega/pass/borrows/stored_primitive_reference_observations_exit --target macos_arm64 --dangerously-accept-all
omega run --both tests/omega/pass/borrows/stored_primitive_reference_observations_exit/main.omg
```

The authored exit is 42. Local and incoming borrowed storage follow the same
stored descriptor to an i32 leaf. A bool descriptor selects an ordinary
state; a sibling using the same field declaration retains its different backing.
Neither observation moves the enclosing carrier or copies its pointer bits as a scalar.
