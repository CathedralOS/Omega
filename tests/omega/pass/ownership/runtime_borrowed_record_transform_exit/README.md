# Transform and restore borrowed storage

```sh
omega run --both tests/omega/pass/ownership/runtime_borrowed_record_transform_exit/main.omg
```

Expected native and checked-interpreter exit: **42**. Moving the inventory
out opens a hole in the caller's storage. The consuming call produces a new
owned inventory, and assigning that result repairs the same hole. Neither
the removed value nor the absent destination may be disposed a second time.
The increment explicitly wraps, so the helper is valid for every `i32` input;
unbounded exact addition would require an overflow proof independently of custody.
