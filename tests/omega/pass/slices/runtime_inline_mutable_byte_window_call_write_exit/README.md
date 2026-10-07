# Inline mutable byte-window argument

From the repository root:

```text
cargo run -p omega -- run tests/omega/pass/slices/runtime_inline_mutable_byte_window_call_write_exit/main.omg
```

The scalar-returning helper receives the nonzero-offset mutable window, writes
its first byte and returns its length. The caller checks length 2, the write
at the original offset and untouched neighboring bytes, then exits 0.
