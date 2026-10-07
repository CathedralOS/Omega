# Inline mutable element-view argument

From the repository root:

```text
cargo run -p omega -- run tests/omega/pass/slices/runtime_inline_mutable_view_call_write_exit/main.omg
```

The helper receives an evaluated mutable range descriptor, writes its first
element and returns Unit. The caller observes the write in the original
array and exits 0. Failure to preserve backing or call completion traps.
