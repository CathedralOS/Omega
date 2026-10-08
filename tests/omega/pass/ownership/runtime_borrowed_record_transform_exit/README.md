# Transform and restore borrowed storage

```sh
omega run --both tests/omega/pass/ownership/runtime_borrowed_record_transform_exit/main.omg
```

Expected native and Terminal-interpreter exit: **42**. Moving the linear
inventory out opens a hole in the caller's storage. Both the fresh-result
branch and the identity-result branch return an owned inventory to the exact
hole. The caller then mutates its scalar field and finally consumes the
inventory. Neither the removed value nor the absent destination may be
disposed a second time.
The increment explicitly wraps, so the helper is valid for every `i32` input;
unbounded exact addition would require an overflow proof independently of custody.
