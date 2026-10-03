# Inline interior slice arguments

After ordinary package preparation, run:

```sh
omega run --both tests/omega/pass/recast/runtime_inline_slice_view_mutable_write_exit/main.omg
```

Both engines must return 70. A runtime-offset mutable byte slice is formed
inside the selected transition argument, forwarded to a state and used to
write the original backing after the authored offset variable changes. An inline record slice then writes four fields.
Backing observations occur after the states that hold the mutable loans.
