# Declared record member writes

```sh
omega run --both tests/omega/pass/structs/runtime_declared_record_member_fill_exit/main.omg
```

Expected native and checked-interpreter exit: **70**. A record declared without
an initializer owns zero-constructible storage; writing its members and then
reading them must use that same storage. Whole-local rebinding still requires
`mut`. This fixture retains the existing hosted receiver entry syntax pending
`STATIC-MACHINE-MIGRATION`.
