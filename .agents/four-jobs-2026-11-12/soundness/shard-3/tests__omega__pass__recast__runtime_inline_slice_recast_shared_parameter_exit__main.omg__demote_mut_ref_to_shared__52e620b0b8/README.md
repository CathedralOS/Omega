# Slice recast of a shared array parameter

After ordinary package preparation, run:

```sh
omega run --both tests/omega/pass/recast/runtime_inline_slice_recast_shared_parameter_exit/main.omg
```

Both engines must return 70. A scalar helper receives a shared six-byte array
and passes an inline `[u16]` recast of its final four bytes to a reader. The
formed descriptor is reserved under its stored view type while the reader's
formal presents the canonical view.
