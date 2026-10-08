# Stored primitive reference still retains its backing loan

Reading the stored primitive is an observation, not a descriptor transfer or
early loan release. A backing write before that observation must reject.

```sh
omega --check tests/omega/fail/borrows/stored_primitive_reference_backing_write/main.omg
```
