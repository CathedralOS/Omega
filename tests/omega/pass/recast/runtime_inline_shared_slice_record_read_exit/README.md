# Inline shared slice observations

After ordinary package preparation, run:

```sh
omega run --both tests/omega/pass/recast/runtime_inline_shared_slice_record_read_exit/main.omg
```

Both engines must return 70. Shared byte and record slices are formed in
transition arguments. The byte view retains its offset after the selector
changes; four record fields are read through the other view. Length guards
prove each index before the observations, and backing checks follow the
states that hold the view loans.
