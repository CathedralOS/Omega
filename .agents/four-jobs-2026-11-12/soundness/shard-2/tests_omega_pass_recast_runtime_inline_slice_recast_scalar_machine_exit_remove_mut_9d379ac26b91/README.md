# Slice recasts formed inside a scalar machine

After ordinary package preparation, run:

```sh
omega run --both tests/omega/pass/recast/runtime_inline_slice_recast_scalar_machine_exit/main.omg
```

Both engines must return 70. This is the scalar-machine variant of
`runtime_inline_slice_recast_call_exit`: a scalar helper, not the Unit entry,
forms the mutable and shared `[u16]` recasts as call operands. Each formed
descriptor is reserved in its own home under its stored view type, which keeps
the view's access ceiling, while the callee's formal presents the canonical
view.
