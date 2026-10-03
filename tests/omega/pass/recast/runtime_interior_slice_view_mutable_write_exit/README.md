# Interior mutable byte and record slices

After ordinary package review, run:

```text
omega run --both tests/omega/pass/recast/runtime_interior_slice_view_mutable_write_exit/main.omg
```

Both engines must return 70. A runtime byte offset forms a five-byte suffix,
which is forwarded through a state parameter and written at its final byte.
A four-byte offset forms two records whose projected field writes must update
the original backing bytes. Each loan ends at an explicit state edge before
the receiver observes its backing. Length checks, backing checks, and failure
exit codes remain independent.
