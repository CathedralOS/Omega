# Projected reference-record argument

An ordinary call consumes an owned record subtree and returns the exclusive
reference stored within it. Writing through that reference changes the caller's
original scalar, which must be observed as 29 after the reference retires.

After ordinary local package review:

```sh
omega run --both tests/omega/pass/borrows/projected_reference_record_argument_exit/main.omg
```

Both engines must exit 29. Whole-owner cleanup must not dispose the transferred
subtree a second time, even when its residual complement is empty.
