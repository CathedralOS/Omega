Run from the repository root:

```
cargo run -p omega -- run --both tests/omega/pass/concurrency/runtime_returned_carrier_suspension_exit/main.omg
```

Both engines must exit 0. The returned generic carrier retains both declared
lifetime candidates, including when the call initializes a nested field.
The caller observes its original referents after the boundary call, including
a reference projected directly from another returned carrier.
The selected provider completes inline; this does not test physical parking,
resumption, or cancellation.
